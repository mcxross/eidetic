use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemGetObservationParams {
    #[schemars(description = "Observation ID to retrieve")]
    pub id: String,
}

#[derive(Clone)]
pub struct MemGetObservation {
    store: MemoryStore,
}

impl MemGetObservation {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Get full content of a specific memory")]
    pub async fn mem_get_observation(
        &self,
        Parameters(params): Parameters<MemGetObservationParams>,
    ) -> Result<CallToolResult, McpError> {
        let obs = self
            .store
            .storage()
            .get_observation(&params.id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(format!("Observation not found: {}", params.id), None)
            })?;

        let output = format!(
            "ID: {}\nProject: {}\nType: {:?}\nTitle: {}\nTopic: {}\nTags: {}\nLifecycle: {:?}\nCreated: {}\nUpdated: {}\nReview After: {}\nRelated: {}\n\nContent:\n{}",
            obs.id,
            obs.project_id,
            obs.memory_type,
            obs.title,
            obs.topic_key.unwrap_or_else(|| "none".to_string()),
            obs.tags.join(", "),
            obs.lifecycle,
            obs.created_at.to_rfc3339(),
            obs.updated_at.to_rfc3339(),
            obs.review_after
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "none".to_string()),
            obs.related_observations.join(", "),
            obs.content
        );

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}
