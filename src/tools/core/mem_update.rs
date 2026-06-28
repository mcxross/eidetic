use crate::memory::types::*;
use crate::storage::MemoryStore;
use chrono::Utc;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemUpdateParams {
    #[schemars(description = "Observation ID to update")]
    pub id: String,
    #[schemars(description = "New title (optional)")]
    pub title: Option<String>,
    #[schemars(description = "New content (optional)")]
    pub content: Option<String>,
    #[schemars(description = "New topic key (optional)")]
    pub topic_key: Option<String>,
    #[schemars(description = "New tags (optional)")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "New metadata (optional)")]
    pub metadata: Option<serde_json::Value>,
    #[schemars(description = "New lifecycle state (optional)")]
    pub lifecycle: Option<LifecycleState>,
    #[schemars(description = "When to schedule review (ISO 8601 datetime, optional)")]
    pub review_after: Option<String>,
    #[schemars(description = "Related observation IDs to add (optional)")]
    pub related_observations: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct MemUpdate {
    store: MemoryStore,
}

impl MemUpdate {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Update an existing observation by ID")]
    pub async fn mem_update(
        &self,
        Parameters(params): Parameters<MemUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut obs = self
            .store
            .storage()
            .get_observation(&params.id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(format!("Observation not found: {}", params.id), None)
            })?;

        if let Some(ref metadata) = params.metadata
            && !metadata.is_null()
            && !metadata.is_object()
        {
            return Err(McpError::invalid_params(
                "metadata must be a JSON object, not an array or primitive",
                None,
            ));
        }

        if let Some(title) = params.title {
            obs.title = title;
        }
        if let Some(content) = params.content {
            obs.content = content;
        }
        if let Some(topic_key) = params.topic_key {
            obs.topic_key = Some(topic_key);
        }
        if let Some(tags) = params.tags {
            obs.tags = tags;
        }
        if let Some(metadata) = params.metadata {
            obs.metadata = metadata.as_object().map_or(HashMap::new(), |m| {
                m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            });
        }
        if let Some(lifecycle) = params.lifecycle {
            obs.lifecycle = lifecycle;
        }
        if let Some(ref review_after) = params.review_after {
            obs.review_after = Some(
                chrono::DateTime::parse_from_rfc3339(review_after)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|_| {
                        McpError::invalid_params(
                            format!(
                                "Invalid review_after datetime format: '{}'. Expected ISO 8601/RFC 3339.",
                                review_after
                            ),
                            None,
                        )
                    })?,
            );
        }
        if let Some(related) = params.related_observations {
            obs.related_observations.extend(related);
            obs.related_observations.sort();
            obs.related_observations.dedup();
        }

        // Recompute hash if title or content changed
        obs.hash = Observation::compute_hash(
            &obs.project_id,
            &obs.scope,
            &obs.memory_type,
            &obs.title,
            &obs.content,
        );
        obs.revision_count += 1;
        obs.updated_at = Utc::now();

        self.store
            .storage()
            .update_observation(&obs)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Updated observation: {} (ID: {})",
            obs.title, obs.id
        ))]))
    }
}

use std::collections::HashMap;
