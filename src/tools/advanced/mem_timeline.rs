use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemTimelineParams {
    #[schemars(description = "Observation ID to center the timeline around")]
    pub observation_id: String,
    #[schemars(description = "Limit of items before and after (default: 5)")]
    pub context_limit: Option<usize>,
}

#[derive(Clone)]
pub struct MemTimeline {
    store: MemoryStore,
}

impl MemTimeline {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Get chronological context around a specific observation")]
    pub async fn mem_timeline(
        &self,
        Parameters(params): Parameters<MemTimelineParams>,
    ) -> Result<CallToolResult, McpError> {
        let storage = self.store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => return Err(McpError::internal_error("mem_timeline is not supported on unstructured storage backends like memwal", None)),
        };

        let obs = structured
            .get_observation(&params.observation_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!("Observation not found: {}", params.observation_id),
                    None,
                )
            })?;

        let project_id = obs.project_id.clone();

        let limit = params.context_limit.unwrap_or(5).min(50);
        let mut all_obs = structured
            .list_observations(&project_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        all_obs.sort_by_key(|a| a.created_at);

        let target_idx = all_obs
            .iter()
            .position(|o| o.id == params.observation_id)
            .unwrap_or(0);

        let start_idx = target_idx.saturating_sub(limit);
        let end_idx = std::cmp::min(all_obs.len(), target_idx + limit + 1);

        let timeline = &all_obs[start_idx..end_idx];

        let result_json = serde_json::to_string_pretty(&timeline).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize timeline: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(result_json)]))
    }
}
