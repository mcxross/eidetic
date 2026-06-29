use crate::config::EideticConfig;
use crate::harbor::HarborCredentials;
use crate::harbor::artifacts::ArtifactManager;
use crate::memory::types::*;
use crate::storage::MemoryStore;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData as McpError;
use rmcp::model::{CallToolResult, Content};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize, JsonSchema)]
pub struct MemGetArtifactParams {
    pub harbor_file_id: String,
    pub is_encrypted: Option<bool>,
    pub seal_id_bytes: Option<String>,
}

#[derive(Clone)]
pub struct MemGetArtifact {
    store: MemoryStore,
}

impl MemGetArtifact {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    pub async fn mem_get_artifact(
        &self,
        params: Parameters<MemGetArtifactParams>,
    ) -> Result<CallToolResult, McpError> {
        let store = Arc::new(self.store.clone());
        let config = EideticConfig::load()
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to load config: {}", e), None))?;

        let auth = store
            .auth_manager()
            .ok_or_else(|| McpError::internal_error("AuthManager is not initialized", None))?;

        let harbor_config = config.harbor.clone().ok_or_else(|| {
            McpError::internal_error("Harbor is not configured in this project", None)
        })?;

        let credentials = HarborCredentials::load().map_err(|e| {
            McpError::internal_error(format!("Failed to load Harbor credentials: {}", e), None)
        })?;

        let artifact_manager = ArtifactManager::new(&credentials, harbor_config, auth);

        let file_id = &params.0.harbor_file_id;
        let is_encrypted = params.0.is_encrypted.unwrap_or(false);
        
        let id_bytes = if let Some(hex_str) = params.0.seal_id_bytes {
            Some(hex::decode(&hex_str).map_err(|e| {
                McpError::internal_error(format!("Failed to decode seal_id_bytes hex: {}", e), None)
            })?)
        } else {
            None
        };

        let downloaded_bytes = artifact_manager
            .download_artifact(file_id, is_encrypted, id_bytes)
            .await
            .map_err(|e| {
                McpError::internal_error(format!("Failed to download artifact: {}", e), None)
            })?;

        let content =
            String::from_utf8(downloaded_bytes).unwrap_or_else(|_| "<binary content>".to_string());

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Artifact retrieved from Harbor\nFile ID: {}\nEncrypted: {}\n\n{}",
            file_id, is_encrypted, content
        ))]))
    }
}
