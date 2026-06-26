use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct EideticConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memwal_account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memwal_registry_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memwal_server_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memwal_relayer_config_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memwal_namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memwal_delegate_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sui_config_dir: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
}

impl EideticConfig {
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::home_dir()
            .context("No home directory found")?
            .join(".eidetic");
        Ok(config_dir.join("config.json"))
    }

    pub async fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .await
            .context("Failed to read config file")?;

        if content.trim().is_empty() {
            return Ok(Self::default());
        }

        let config: EideticConfig =
            serde_json::from_str(&content).context("Failed to parse config file")?;
        Ok(config)
    }

    pub async fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create config directory")?;
            }
        }

        let content = serde_json::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&path, content)
            .await
            .context("Failed to write config file")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_default_config() {
        let config = EideticConfig::default();
        assert!(config.storage_backend.is_none());
        assert!(config.memwal_account_id.is_none());
    }

    #[test]
    fn test_serialization_ignores_none() {
        let config = EideticConfig::default();
        let serialized = serde_json::to_string(&config).unwrap();
        // Since all fields are None and we use skip_serializing_if, the default config should serialize to an empty object.
        assert_eq!(serialized, "{}");
    }

    #[test]
    fn test_deserialization() {
        let json_str = r#"{
            "storage_backend": "memwal",
            "memwal_account_id": "acc_123",
            "sui_config_dir": "/custom/path"
        }"#;

        let config: EideticConfig = serde_json::from_str(json_str).unwrap();

        assert_eq!(config.storage_backend.as_deref(), Some("memwal"));
        assert_eq!(config.memwal_account_id.as_deref(), Some("acc_123"));
        assert_eq!(
            config.sui_config_dir.unwrap().to_str(),
            Some("/custom/path")
        );
        assert!(config.private_key.is_none());
    }

    #[test]
    fn test_partial_update_serialization() {
        let mut config = EideticConfig::default();
        config.private_key = Some("suiprivkey123".to_string());

        let serialized = serde_json::to_string(&config).unwrap();
        let json_val: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(json_val["private_key"], "suiprivkey123");
        assert!(json_val.get("storage_backend").is_none());
    }
}
