use crate::auth::AuthManager;
use crate::config::HarborConfig;
use crate::harbor::HarborCredentials;
use anyhow::{Context, Result};
use fastcrypto::ed25519::Ed25519KeyPair;
use fastcrypto::traits::ToFromBytes;
use harbor_core::client::{HarborClient, HarborClientOptions};
use harbor_core::seal::HarborSealService;
use harbor_core::utils::{SimpleSigner, fetch_initial_shared_version};
use seal_sdk_rs::session_key::SessionKey;
use std::str::FromStr;
use std::sync::Arc;

pub struct ArtifactUploadResult {
    pub file_id: String,
    pub is_encrypted: bool,
    pub id_bytes: Option<Vec<u8>>,
}

pub struct ArtifactManager {
    harbor: HarborClient,
    config: HarborConfig,
    auth: Arc<AuthManager>,
}

impl ArtifactManager {
    pub fn new(
        credentials: &HarborCredentials,
        mut config: HarborConfig,
        auth: Arc<AuthManager>,
    ) -> Self {
        if config.seal_package_id.is_none() {
            config.seal_package_id = Some("0x8b2429358e9b0f005b69fe8ad3cbd1268ad87f35047a21612e082c64824faf8d".to_string());
        }
        if config.seal_key_server_ids.is_none() {
            config.seal_key_server_ids = Some(vec![
                "0x6068c0acb197dddbacd4746a9de7f025b2ed5a5b6c1b1ab44dade4426d141da2".to_string(),
                "0x164ac3d2b3b8694b8181c13f671950004765c23f270321a45fdd04d40cccf0f2".to_string(),
                "0x9c949e53c36ab7a9c484ed9e8b43267a77d4b8d70e79aa6b39042e3d4c434105".to_string(),
            ]);
        }

        let harbor = HarborClient::new(HarborClientOptions {
            api_key: credentials.api_key.clone(),
            ..Default::default()
        });
        Self {
            harbor,
            config,
            auth,
        }
    }

    async fn get_sui_keypair(&self) -> Result<Ed25519KeyPair> {
        let privkey_b64 = self.auth.get_sui_private_key().await?;
        if privkey_b64.starts_with("suiprivkey") {
            use bech32::FromBase32;
            let (_, data, _) = bech32::decode(&privkey_b64)?;
            let decoded = Vec::<u8>::from_base32(&data)?;
            let secret_key = decoded[1..].to_vec();
            Ed25519KeyPair::from_bytes(&secret_key)
                .map_err(|e| anyhow::anyhow!("Invalid ed25519 key: {}", e))
        } else {
            use base64::Engine;
            let privkey_bytes = base64::engine::general_purpose::STANDARD.decode(&privkey_b64)?;
            Ed25519KeyPair::from_bytes(&privkey_bytes)
                .map_err(|e| anyhow::anyhow!("Invalid ed25519 key: {}", e))
        }
    }

    pub async fn upload_artifact(
        &self,
        filename: &str,
        content: &[u8],
        encrypt: bool,
    ) -> Result<ArtifactUploadResult> {
        let bucket_id = self
            .config
            .bucket_id
            .as_ref()
            .context("Harbor bucket ID not configured")?;

        let mut upload_payload = content.to_vec();
        let mut id_bytes = None;

        if encrypt {
            let seal_policy_id = self
                .config
                .seal_policy_id
                .as_ref()
                .context("seal_policy_id not configured")?;
            let seal_package_id = self
                .config
                .seal_package_id
                .as_ref()
                .context("seal_package_id not configured")?;
            let seal_key_servers = self
                .config
                .seal_key_server_ids
                .as_ref()
                .context("seal_key_server_ids not configured")?;

            let sui_client = sui_rpc::Client::new("https://fullnode.testnet.sui.io:443")?;
            let seal = HarborSealService::new(
                sui_client,
                seal_key_servers.iter().map(|s| s.as_str()).collect(),
            );

            let (enc_id_bytes, encrypted) = seal
                .encrypt(seal_package_id, seal_policy_id, content)
                .await?;
            upload_payload = encrypted;
            id_bytes = Some(enc_id_bytes);
        }

        let upload = self
            .harbor
            .upload_file(bucket_id, filename, upload_payload, |_, _| {})
            .await?;

        self.harbor
            .poll_until_completed(bucket_id, &upload.data.id, |_, _| {})
            .await?;

        Ok(ArtifactUploadResult {
            file_id: upload.data.id,
            is_encrypted: encrypt,
            id_bytes,
        })
    }

    pub async fn download_artifact(
        &self,
        file_id: &str,
        is_encrypted: bool,
        id_bytes: Option<Vec<u8>>,
    ) -> Result<Vec<u8>> {
        let bucket_id = self
            .config
            .bucket_id
            .as_ref()
            .context("Harbor bucket ID not configured")?;
        let downloaded = self.harbor.download_file(bucket_id, file_id).await?;

        if is_encrypted {
            let seal_policy_id = self
                .config
                .seal_policy_id
                .as_ref()
                .context("seal_policy_id not configured")?;
            let seal_package_id = self
                .config
                .seal_package_id
                .as_ref()
                .context("seal_package_id not configured")?;
            let seal_key_servers = self
                .config
                .seal_key_server_ids
                .as_ref()
                .context("seal_key_server_ids not configured")?;

            let mut sui_client = sui_rpc::Client::new("https://fullnode.testnet.sui.io:443")?;
            let seal = HarborSealService::new(
                sui_client.clone(),
                seal_key_servers.iter().map(|s| s.as_str()).collect(),
            );

            let policy_initial_shared_version =
                fetch_initial_shared_version(&mut sui_client, seal_policy_id).await?;

            let keypair = self.get_sui_keypair().await?;
            let mut signer = SimpleSigner(keypair);

            let pkg_addr = sui_sdk_types::Address::from_str(seal_package_id)
                .map_err(|e| anyhow::anyhow!("Invalid package ID: {}", e))?
                .into_inner();

            let session_key = SessionKey::new(pkg_addr, 10, &mut signer).await?;

            let id_bytes = id_bytes.context("id_bytes is required for decrypting")?;

            let decrypted = seal
                .decrypt(
                    seal_package_id,
                    seal_policy_id,
                    policy_initial_shared_version,
                    id_bytes,
                    &downloaded,
                    &session_key,
                )
                .await?;

            return Ok(decrypted);
        }

        Ok(downloaded)
    }
}
