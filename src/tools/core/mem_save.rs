use crate::memory::types::*;
use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemSaveParams {
    #[schemars(description = "Project ID (optional, will auto-detect from cwd if not provided)")]
    pub project_id: Option<String>,
    #[schemars(description = "Scope of the observation (project, personal, global)")]
    pub scope: Option<Scope>,
    #[schemars(description = "Type of memory")]
    pub memory_type: MemoryType,
    #[schemars(description = "Short title for the observation")]
    pub title: String,
    #[schemars(description = "Full content of the observation")]
    pub content: String,
    #[schemars(description = "Optional topic key for grouping related observations")]
    pub topic_key: Option<String>,
    #[schemars(description = "Tags for categorization")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Additional metadata")]
    pub metadata: Option<serde_json::Value>,
    #[schemars(description = "Whether to capture current prompt context (default: true)")]
    pub capture_prompt: Option<bool>,
    #[schemars(description = "Session ID to associate with (optional)")]
    pub session_id: Option<String>,
    #[schemars(description = "When to schedule review (ISO 8601 datetime, optional)")]
    pub review_after: Option<String>,
    #[schemars(description = "Related observation IDs")]
    pub related_observations: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct MemSave {
    store: MemoryStore,
}

impl MemSave {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(
        description = "Save a structured observation (decision, bugfix, pattern, etc.); best-effort captures process-local current prompt context when available unless capture_prompt=false"
    )]
    pub async fn mem_save(
        &self,
        Parameters(params): Parameters<MemSaveParams>,
    ) -> Result<CallToolResult, McpError> {
        // Input validation
        if params.title.trim().is_empty() {
            return Err(McpError::invalid_params("title must not be empty", None));
        }
        if params.content.trim().is_empty() {
            return Err(McpError::invalid_params("content must not be empty", None));
        }
        if let Some(ref metadata) = params.metadata
            && !metadata.is_null()
            && !metadata.is_object()
        {
            return Err(McpError::invalid_params(
                "metadata must be a JSON object, not an array or primitive",
                None,
            ));
        }

        let storage = self.store.storage();

        if let Some(structured) = storage.as_structured() {
            if let Some(ref sid) = params.session_id {
                let session_exists = structured
                    .get_session(sid)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
                    .is_some();
                if !session_exists {
                    return Err(McpError::invalid_params(
                        format!("Session not found: {}", sid),
                        None,
                    ));
                }
            }

            let project = if let Some(pid) = &params.project_id {
                structured
                    .get_project(&pid)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
                    .ok_or_else(|| {
                        McpError::invalid_params(format!("Project not found: {}", pid), None)
                    })?
            } else {
                self.store
                    .get_or_create_project(None)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
            };

            info!("mem_save: saving observation for project={}", project.id);

            let project_id = project.id.clone();
            let scope = params.scope.unwrap_or(Scope::Project);
            let content_hash = Observation::compute_hash(
                &project_id,
                &scope,
                &params.memory_type,
                &params.title,
                &params.content,
            );

            let mut all_obs = structured
                .list_observations(&project_id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            if let Some(existing) = all_obs.iter_mut().find(|o| {
                o.hash == content_hash
                    && o.scope == scope
                    && o.memory_type == params.memory_type
                    && o.title == params.title
            }) {
                info!("mem_save: dedup hit for hash={}", content_hash);
                existing.duplicate_count += 1;
                existing.last_seen_at = chrono::Utc::now();
                existing.updated_at = chrono::Utc::now();
                structured
                    .update_observation(existing)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Observation deduplicated. Existing ID: {} ({} duplicates)",
                    existing.id, existing.duplicate_count
                ))]));
            }

            let meta = match params.metadata.clone() {
                Some(serde_json::Value::Object(map)) => Some(map.into_iter().collect()),
                _ => None,
            };

            let mut obs = Observation {
                id: format!("obs_{}", uuid::Uuid::new_v4().simple()),
                project_id: project_id.clone(),
                session_id: params.session_id.clone(),
                title: params.title.clone(),
                content: params.content.clone(),
                tags: params.tags.clone().unwrap_or_default(),
                metadata: meta.unwrap_or_default(),
                memory_type: params.memory_type.clone(),
                scope,
                lifecycle: LifecycleState::Active,
                topic_key: params.topic_key.clone(),
                hash: content_hash,
                revision_count: 1,
                duplicate_count: 0,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                last_seen_at: chrono::Utc::now(),
                reviewed_at: None,
                review_after: params
                    .review_after
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc)),
                deleted_at: None,
                deleted_mode: None,
                related_observations: params.related_observations.unwrap_or_default(),
                source_prompt: None,
                capture_prompt: params.capture_prompt.unwrap_or(true),
            };

            if obs.capture_prompt {
                if let Ok(recent) = structured.get_recent_observations(&project_id, 5).await {
                    for r in recent.iter() {
                        if r.capture_prompt && r.source_prompt.is_some() {
                            obs.source_prompt = r.source_prompt.clone();
                            break;
                        }
                    }
                }
            }

            structured
                .save_observation(&obs)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            Ok(CallToolResult::success(vec![Content::text(format!(
                "Successfully saved observation `{}` with ID `{}`",
                obs.title, obs.id
            ))]))
        } else if let Some(unstructured) = storage.as_unstructured() {
            let project = self
                .store
                .get_or_create_project(None)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            
            let namespace = project.id.clone();
            
            let text_to_remember = format!(
                "Title: {}\nType: {:?}\nTags: {:?}\n\n{}",
                params.title, params.memory_type, params.tags.unwrap_or_default(), params.content
            );

            let job_id = unstructured
                .remember(&text_to_remember, Some(&namespace))
                .await
                .map_err(|e| McpError::internal_error(format!("Memwal remember failed: {}", e), None))?;

            Ok(CallToolResult::success(vec![Content::text(format!(
                "Successfully sent observation to Memwal network. Job ID: `{}`",
                job_id
            ))]))
        } else {
            Err(McpError::internal_error("Storage backend has no known capabilities", None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::types::{MemoryType, Scope};
    use crate::storage::MemoryStore;
    use rmcp::handler::server::wrapper::Parameters;

    #[tokio::test]
    async fn test_mem_save_validation() {
        let (store, _dir) = MemoryStore::setup_test_store().await;
        let tool = MemSave::new(store);

        // Empty title
        let params = MemSaveParams {
            project_id: None,
            scope: None,
            memory_type: MemoryType::Note,
            title: "".to_string(),
            content: "Some content".to_string(),
            topic_key: None,
            tags: None,
            metadata: None,
            capture_prompt: None,
            session_id: None,
            review_after: None,
            related_observations: None,
        };

        let result = tool.mem_save(Parameters(params)).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .message
                .contains("title must not be empty")
        );
    }

    #[tokio::test]
    async fn test_mem_save_success() {
        let (store, _dir) = MemoryStore::setup_test_store().await;
        let tool = MemSave::new(store.clone());

        let params = MemSaveParams {
            project_id: None,
            scope: Some(Scope::Global),
            memory_type: MemoryType::Note,
            title: "Test Observation".to_string(),
            content: "This is a test observation.".to_string(),
            topic_key: Some("test_topic".to_string()),
            tags: Some(vec!["test".to_string()]),
            metadata: None,
            capture_prompt: Some(false),
            session_id: None,
            review_after: None,
            related_observations: None,
        };

        let result = tool.mem_save(Parameters(params)).await;
        assert!(result.is_ok());

        // Verify it was actually saved
        let project = store.get_or_create_project(None).await.unwrap();
        let storage = store.storage();
        let structured = storage.as_structured().unwrap();
        let saved_obs = structured
            .search_observations(&project.id, "Test Observation", 10)
            .await
            .unwrap();

        assert_eq!(saved_obs.len(), 1);
        assert_eq!(saved_obs[0].observation.title, "Test Observation");
        assert_eq!(
            saved_obs[0].observation.content,
            "This is a test observation."
        );
        assert_eq!(
            saved_obs[0].observation.topic_key.as_deref(),
            Some("test_topic")
        );
    }
}
