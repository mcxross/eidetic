use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    tool,
    schemars::JsonSchema,
};
use serde::{Deserialize, Serialize};
use crate::storage::MemoryStore;

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
        let obs = self.store.storage().get_observation(&params.observation_id).await.map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(format!("Observation not found: {}", params.observation_id), None))?;
            
        let project_id = obs.project_id.clone();
        let _target_time = obs.created_at;

        let limit = params.context_limit.unwrap_or(5);
        let mut all_obs = self.store.storage().list_observations(&project_id).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;
        
        all_obs.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        let target_idx = all_obs.iter().position(|o| o.id == params.observation_id).unwrap_or(0);

        let start_idx = target_idx.saturating_sub(limit);
        let end_idx = std::cmp::min(all_obs.len(), target_idx + limit + 1);

        let timeline = &all_obs[start_idx..end_idx];

        let result_json = serde_json::to_string_pretty(&timeline)
            .map_err(|e| McpError::internal_error(format!("Failed to serialize timeline: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(result_json)]))
    }
}
