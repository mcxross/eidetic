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

fn default_true() -> bool {
    true
}

#[derive(Deserialize, JsonSchema)]
pub struct MemSaveArtifactParams {
    pub filename: String,
    pub content: Option<String>,
    pub file_path: Option<String>,
    #[serde(default = "default_true")]
    pub encrypt: bool,
    pub topic_key: Option<String>,
    pub project_id: Option<String>,
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

        let content = if let Some(path) = &params.0.file_path {
            tokio::fs::read(path).await.map_err(|e| {
                McpError::internal_error(format!("Failed to read file at {}: {}", path, e), None)
            })?
        } else if let Some(content) = &params.0.content {
            content.as_bytes().to_vec()
        } else {
            return Err(McpError::invalid_params(
                "Either content or file_path must be provided".to_string(),
                None,
            ));
        };

        let result = artifact_manager
            .upload_artifact(
                &params.0.filename,
                &content,
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
        if let Some(ref id_bytes) = result.id_bytes {
            metadata.insert(
                "seal_id_bytes".to_string(),
                serde_json::Value::String(hex::encode(id_bytes)),
            );
        }
        metadata.insert(
            "filename".to_string(),
            serde_json::Value::String(params.0.filename.clone()),
        );

        let project_id = if let Some(pid) = params.0.project_id {
            pid
        } else {
            store
                .get_current_project()
                .await
                .ok_or_else(|| McpError::internal_error("No active project", None))?
        };

        let session_id = store.get_current_session().await;

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        let obs = Observation {
            id: id.clone(),
            project_id: project_id.clone(),
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
        if let Some(structured) = storage.as_structured() {
            structured.save_observation(&obs).await.map_err(|e| {
                McpError::internal_error(format!("Failed to save observation: {}", e), None)
            })?;
        } else if let Some(unstructured) = storage.as_unstructured() {
            let mut seal_id_str = "".to_string();
            if let Some(id_bytes) = &result.id_bytes {
                seal_id_str = format!("\nSeal ID: {}", hex::encode(id_bytes));
            }

            let text = format!(
                "[ARTIFACT REFERENCE]\nFilename: {}\nHarbor File ID: {}\nEncrypted: {}{}",
                params.0.filename, result.file_id, result.is_encrypted, seal_id_str
            );

            let actual_ns = project_id; // we already resolved this above
            unstructured
                .remember(&text, Some(&actual_ns))
                .await
                .map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to remember artifact to Memwal: {}", e),
                        None,
                    )
                })?;
        } else {
            return Err(McpError::internal_error(
                "Storage backend has no known capabilities",
                None,
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Successfully saved artifact to Harbor.\nFile ID: {}",
            result.file_id
        ))]))
    }
}
