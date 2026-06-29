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
pub struct MemSaveArtifactParams {
    pub filename: String,
    pub content: String,
    #[serde(default)]
    pub encrypt: bool,
    pub topic_key: Option<String>,
}

#[derive(Clone)]
pub struct MemSaveArtifact {
    store: MemoryStore,
}

impl MemSaveArtifact {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    pub async fn mem_save_artifact(
        &self,
        params: Parameters<MemSaveArtifactParams>,
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

        let result = artifact_manager
            .upload_artifact(
                &params.0.filename,
                params.0.content.as_bytes(),
                params.0.encrypt,
            )
            .await
            .map_err(|e| {
                McpError::internal_error(format!("Failed to upload artifact: {}", e), None)
            })?;

        let mut metadata = std::collections::HashMap::new();
        metadata.insert(
            "harbor_file_id".to_string(),
            serde_json::Value::String(result.file_id.clone()),
        );
        metadata.insert(
            "is_encrypted".to_string(),
            serde_json::Value::Bool(result.is_encrypted),
        );
        if let Some(id_bytes) = result.id_bytes {
            metadata.insert(
                "seal_id_bytes".to_string(),
                serde_json::Value::String(hex::encode(id_bytes)),
            );
        }
        metadata.insert(
            "filename".to_string(),
            serde_json::Value::String(params.0.filename.clone()),
        );

        let project_id = store
            .get_current_project()
            .await
            .ok_or_else(|| McpError::internal_error("No active project", None))?;

        let session_id = store.get_current_session().await;

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        let obs = Observation {
            id: id.clone(),
            project_id,
            session_id,
            title: format!("Artifact: {}", params.0.filename),
            memory_type: MemoryType::Artifact,
            scope: Scope::Personal,
            topic_key: params.0.topic_key.clone(),
            tags: vec!["artifact".to_string(), "harbor".to_string()],
            content: format!(
                "Artifact uploaded to Harbor. File ID: {}\nEncrypted: {}",
                result.file_id, result.is_encrypted
            ),
            hash: "".to_string(),
            revision_count: 0,
            duplicate_count: 0,
            last_seen_at: now,
            reviewed_at: None,
            review_after: None,
            deleted_at: None,
            deleted_mode: None,
            related_observations: vec![],
            source_prompt: None,
            capture_prompt: false,
            created_at: now,
            updated_at: now,
            lifecycle: LifecycleState::Active,
            metadata,
        };

        let storage = store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => return Err(McpError::internal_error("mem_save_artifact is not supported on unstructured storage backends like memwal", None)),
        };

        structured.save_observation(&obs).await.map_err(|e| {
            McpError::internal_error(format!("Failed to save observation: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Successfully saved artifact to Harbor.\nFile ID: {}\nObservation ID: {}",
            result.file_id, id
        ))]))
    }
}
