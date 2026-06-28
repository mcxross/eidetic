use crate::memory::types::*;
use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
        if let Some(ref metadata) = params.metadata {
            if !metadata.is_null() && !metadata.is_object() {
                return Err(McpError::invalid_params(
                    "metadata must be a JSON object, not an array or primitive",
                    None,
                ));
            }
        }
        if let Some(ref sid) = params.session_id {
            let session_exists = self
                .store
                .storage()
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

        let project = if let Some(pid) = params.project_id {
            self.store
                .storage()
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

        let mut all_obs = self
            .store
            .storage()
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
            self.store
                .storage()
                .update_observation(existing)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Observation deduplicated: {} (ID: {}, Duplicates: {})",
                existing.title, existing.id, existing.duplicate_count
            ))]));
        }

        if let Some(topic) = &params.topic_key
            && let Some(existing) = all_obs
                .iter_mut()
                .find(|o| o.scope == scope && o.topic_key.as_ref() == Some(topic))
        {
            info!("mem_save: topic upsert for key={}", topic);
            existing.content = params.content.clone();
            existing.hash = content_hash.clone();
            existing.title = params.title.clone();
            existing.memory_type = params.memory_type;
            if let Some(tags) = &params.tags {
                existing.tags = tags.clone();
            }
            if let Some(metadata) = &params.metadata
                && let Some(obj) = metadata.as_object()
            {
                existing.metadata = obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            }
            existing.revision_count += 1;
            existing.updated_at = chrono::Utc::now();
            self.store
                .storage()
                .update_observation(existing)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Observation topic upserted: {} (ID: {}, Revisions: {})",
                existing.title, existing.id, existing.revision_count
            ))]));
        }

        let review_after = if let Some(ref ra) = params.review_after {
            Some(
                chrono::DateTime::parse_from_rfc3339(ra)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|_| {
                        McpError::invalid_params(
                            format!(
                                "Invalid review_after datetime format: '{}'. Expected ISO 8601/RFC 3339.",
                                ra
                            ),
                            None,
                        )
                    })?,
            )
        } else {
            None
        };

        let mut obs = Observation::new(
            project_id.clone(),
            scope,
            params.memory_type,
            params.title,
            params.content,
        );

        obs.session_id = params.session_id;
        obs.topic_key = params.topic_key;
        obs.tags = params.tags.unwrap_or_default();
        obs.metadata = params
            .metadata
            .unwrap_or_default()
            .as_object()
            .map_or(HashMap::new(), |m| {
                m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            });
        obs.review_after = review_after;
        obs.related_observations = params.related_observations.unwrap_or_default();
        obs.capture_prompt = params.capture_prompt.unwrap_or(true);

        if obs.capture_prompt {
            let active_session = if let Some(sid) = obs.session_id.clone() {
                Some(sid)
            } else {
                self.store.get_current_session().await
            };
            if let Ok(prompts) = self
                .store
                .storage()
                .get_prompts(&project_id, active_session.as_ref())
                .await
                && let Some(latest) = prompts.first()
            {
                obs.source_prompt = Some(latest.prompt.clone());
            }
        }

        self.store
            .storage()
            .save_observation(&obs)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if let Some(sid) = &obs.session_id
            && let Some(mut session) = self
                .store
                .storage()
                .get_session(sid)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        {
            session.observation_ids.push(obs.id.clone());
            self.store
                .storage()
                .update_session(&session)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        }

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Saved new observation: {} (ID: {})",
            obs.title, obs.id
        ))]))
    }
}
