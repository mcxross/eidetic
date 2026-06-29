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
    pub observation_id: String,
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

        let obs_id = params.0.observation_id.clone();
        let storage = store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => return Err(McpError::internal_error("mem_get_artifact is not supported on unstructured storage backends like memwal", None)),
        };

        let obs = structured
            .get_observation(&obs_id)
            .await
            .map_err(|e| {
                McpError::internal_error(format!("Failed to get observation: {}", e), None)
            })?
            .ok_or_else(|| McpError::internal_error("Observation not found", None))?;

        if obs.memory_type != MemoryType::Artifact {
            return Err(McpError::internal_error(
                "Observation is not an artifact",
                None,
            ));
        }

        let file_id = obs
            .metadata
            .get("harbor_file_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpError::internal_error("harbor_file_id not found in metadata", None)
            })?;

        let is_encrypted = obs
            .metadata
            .get("is_encrypted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let id_bytes = if let Some(hex_str) =
            obs.metadata.get("seal_id_bytes").and_then(|v| v.as_str())
        {
            Some(hex::decode(hex_str).map_err(|e| {
                McpError::internal_error(format!("Failed to decode seal_id_bytes hex: {}", e), None)
            })?)
        } else {
            None
        };

        let filename = obs
            .metadata
            .get("filename")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let downloaded_bytes = artifact_manager
            .download_artifact(file_id, is_encrypted, id_bytes)
            .await
            .map_err(|e| {
                McpError::internal_error(format!("Failed to download artifact: {}", e), None)
            })?;

        let content =
            String::from_utf8(downloaded_bytes).unwrap_or_else(|_| "<binary content>".to_string());

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Artifact: {}\nFile ID: {}\nEncrypted: {}\n\n{}",
            filename, file_id, is_encrypted, content
        ))]))
    }
}
