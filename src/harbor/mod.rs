pub mod artifacts;

use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const KEYRING_SERVICE: &str = "eidetic-harbor";
const KEYRING_API_KEY: &str = "api-key";
const KEYRING_SERVICE_KEY: &str = "service-private-key";

const BACKUP_HEADER_MARKER: &str = "EIDETIC_BACKUP_V1";

pub struct HarborCredentials {
    pub api_key: String,
    pub service_private_key: String,
}

impl HarborCredentials {
    pub fn store(api_key: &str, service_private_key: &str) -> Result<()> {
        let api_entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_API_KEY)
            .context("Failed to create keyring entry for API key")?;
        api_entry
            .set_password(api_key)
            .context("Failed to store API key in keyring")?;

        let key_entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_SERVICE_KEY)
            .context("Failed to create keyring entry for service key")?;
        key_entry
            .set_password(service_private_key)
            .context("Failed to store service key in keyring")?;

        Ok(())
    }

    pub fn load() -> Result<Self> {
        let api_entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_API_KEY)
            .context("Failed to create keyring entry for API key")?;
        let api_key = api_entry
            .get_password()
            .context("Harbor API key not found in keyring. Run 'eidetic setup harbor' first.")?;

        let key_entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_SERVICE_KEY)
            .context("Failed to create keyring entry for service key")?;
        let service_private_key = key_entry.get_password().context(
            "Harbor service private key not found in keyring. Run 'eidetic setup harbor' first.",
        )?;

        Ok(Self {
            api_key,
            service_private_key,
        })
    }

    pub fn is_configured() -> bool {
        Self::load().is_ok()
    }
}

pub struct HarborBackupManager {
    harbor: harbor_core::client::HarborClient,
    bucket_id: String,
}

impl HarborBackupManager {
    pub fn new(credentials: &HarborCredentials, bucket_id: String) -> Self {
        let harbor =
            harbor_core::client::HarborClient::new(harbor_core::client::HarborClientOptions {
                api_key: credentials.api_key.clone(),
                ..Default::default()
            });
        Self { harbor, bucket_id }
    }

    fn today_backup_name() -> String {
        let day = Utc::now().format("%A").to_string().to_lowercase();
        format!("backup_{}.enc", day)
    }

    pub async fn create_snapshot(db_path: &Path, backend: &str) -> Result<PathBuf> {
        let snapshot_path = db_path.with_extension(if backend == "file" {
            "backup.tar.gz"
        } else {
            "backup.tmp"
        });

        if backend == "file" {
            let output = std::process::Command::new("tar")
                .arg("-czf")
                .arg(&snapshot_path)
                .arg("-C")
                .arg(db_path.parent().unwrap())
                .arg(db_path.file_name().unwrap())
                .output()
                .context("Failed to execute tar command")?;

            if !output.status.success() {
                anyhow::bail!("Tar command failed: {:?}", output);
            }
        } else {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    sqlx::sqlite::SqliteConnectOptions::new()
                        .filename(db_path)
                        .read_only(true),
                )
                .await
                .context("Failed to open database for snapshot")?;

            let query = Box::leak(
                format!("VACUUM INTO '{}'", snapshot_path.to_string_lossy()).into_boxed_str(),
            );
            use sqlx::Executor;
            pool.execute(&*query)
                .await
                .context("Failed to create database snapshot via VACUUM INTO")?;

            pool.close().await;
        }
        Ok(snapshot_path)
    }

    pub fn build_payload(snapshot_bytes: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(snapshot_bytes);
        let hash = hex::encode(hasher.finalize());
        let timestamp = Utc::now().to_rfc3339();
        let header = format!(
            "{}\nsha256:{}\ntimestamp:{}\nsize:{}\n---\n",
            BACKUP_HEADER_MARKER,
            hash,
            timestamp,
            snapshot_bytes.len()
        );

        let mut payload = header.into_bytes();
        payload.extend_from_slice(snapshot_bytes);
        payload
    }

    pub fn parse_and_verify_payload(payload: &[u8]) -> Result<Vec<u8>> {
        let separator = b"\n---\n";
        let pos = payload
            .windows(separator.len())
            .position(|w| w == separator)
            .context("Invalid backup payload: missing separator")?;

        let header_str = std::str::from_utf8(&payload[..pos])
            .context("Invalid backup payload: header is not valid UTF-8")?;
        let data = &payload[pos + separator.len()..];

        if !header_str.starts_with(BACKUP_HEADER_MARKER) {
            anyhow::bail!("Invalid backup payload: wrong marker");
        }

        let expected_hash = header_str
            .lines()
            .find(|l| l.starts_with("sha256:"))
            .and_then(|l| l.strip_prefix("sha256:"))
            .context("Invalid backup payload: missing sha256 hash")?;

        let mut hasher = Sha256::new();
        hasher.update(data);
        let actual_hash = hex::encode(hasher.finalize());

        if actual_hash != expected_hash {
            anyhow::bail!(
                "Backup integrity check failed!\n  Expected: {}\n  Got: {}",
                expected_hash,
                actual_hash
            );
        }

        Ok(data.to_vec())
    }

    pub async fn backup(&self, db_path: &Path, backend: &str) -> Result<String> {
        // 1. Create snapshot
        let snapshot_path = Self::create_snapshot(db_path, backend).await?;
        let snapshot_bytes = tokio::fs::read(&snapshot_path)
            .await
            .context("Failed to read snapshot file")?;

        let payload = Self::build_payload(&snapshot_bytes);

        let file_name = Self::today_backup_name();

        if let Ok(files) = self.harbor.list_files(&self.bucket_id).await {
            for file in files {
                if file.name.as_deref() == Some(&file_name) {
                    let _ = self.harbor.delete_file(&self.bucket_id, &file.id).await;
                }
            }
        }

        let upload = self
            .harbor
            .upload_file(&self.bucket_id, &file_name, payload, |attempt, body| {
                tracing::warn!("Upload retry {}: {}", attempt, body);
            })
            .await
            .context("Failed to upload backup to Harbor")?;

        self.harbor
            .poll_until_completed(&self.bucket_id, &upload.data.id, |attempt, state| {
                tracing::info!("Poll {}: state={}", attempt, state);
            })
            .await
            .context("Backup upload did not complete in time")?;

        let _ = tokio::fs::remove_file(&snapshot_path).await;

        Ok(file_name)
    }

    pub async fn list_backups(&self) -> Result<Vec<harbor_core::types::FileSummary>> {
        let files = self
            .harbor
            .list_files(&self.bucket_id)
            .await
            .context("Failed to list files in Harbor bucket")?;
        Ok(files
            .into_iter()
            .filter(|f| {
                f.name
                    .as_deref()
                    .is_some_and(|n| n.starts_with("backup_") && n.ends_with(".enc"))
            })
            .collect())
    }

    pub async fn download_backup(&self, file_id: &str) -> Result<Vec<u8>> {
        let ciphertext = self
            .harbor
            .download_file(&self.bucket_id, file_id)
            .await
            .context("Failed to download backup from Harbor")?;

        let db_bytes = Self::parse_and_verify_payload(&ciphertext)?;
        Ok(db_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_build_and_verify() {
        let dummy_data = b"hello world sqlite snapshot";
        let payload = HarborBackupManager::build_payload(dummy_data);

        // Verify the payload contains the header marker
        let payload_str = String::from_utf8_lossy(&payload);
        assert!(payload_str.starts_with(BACKUP_HEADER_MARKER));
        assert!(payload_str.contains("sha256:"));

        // Parse and verify should recover the original data
        let recovered = HarborBackupManager::parse_and_verify_payload(&payload).unwrap();
        assert_eq!(recovered, dummy_data);
    }

    #[test]
    fn test_payload_tampering() {
        let dummy_data = b"hello world sqlite snapshot";
        let mut payload = HarborBackupManager::build_payload(dummy_data);

        // Tamper with the data payload (after the header)
        let len = payload.len();
        payload[len - 1] = b'x';

        let result = HarborBackupManager::parse_and_verify_payload(&payload);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("integrity check failed")
        );
    }
}
