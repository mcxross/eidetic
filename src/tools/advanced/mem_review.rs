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
pub struct MemReviewParams {
    #[schemars(description = "Action to perform: 'list' or 'mark_reviewed'")]
    pub action: String,
    #[schemars(description = "Project ID (optional)")]
    pub project_id: Option<String>,
    #[schemars(
        description = "Observation ID to mark as reviewed (required if action = 'mark_reviewed')"
    )]
    pub observation_id: Option<String>,
}

#[derive(Clone)]
pub struct MemReview {
    store: MemoryStore,
}

impl MemReview {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(
        description = "List observations whose review_after lifecycle is stale; mark_reviewed resets the local review cycle"
    )]
    pub async fn mem_review(
        &self,
        Parameters(params): Parameters<MemReviewParams>,
    ) -> Result<CallToolResult, McpError> {
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

        let action = params.action.trim().to_lowercase();
        match action.as_str() {
            "list" => {
                let stale_reviews = self
                    .store
                    .storage()
                    .get_stale_reviews(&project.id)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                let result_json = serde_json::to_string_pretty(&stale_reviews).map_err(|e| {
                    McpError::internal_error(format!("Failed to serialize reviews: {}", e), None)
                })?;
                Ok(CallToolResult::success(vec![Content::text(result_json)]))
            }
            "mark_reviewed" => {
                let obs_id = params.observation_id.ok_or_else(|| {
                    McpError::invalid_params(
                        "observation_id is required for action 'mark_reviewed'",
                        None,
                    )
                })?;

                if let Some(mut obs) = self
                    .store
                    .storage()
                    .get_observation(&obs_id)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
                {
                    obs.reviewed_at = Some(Utc::now());
                    obs.review_after = Some(Utc::now() + chrono::Duration::days(7));
                    self.store
                        .storage()
                        .update_observation(&obs)
                        .await
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Observation {} marked as reviewed. Next review scheduled for {}",
                        obs_id,
                        obs.review_after.unwrap_or_else(Utc::now).to_rfc3339()
                    ))]))
                } else {
                    Err(McpError::invalid_params(
                        format!("Observation not found: {}", obs_id),
                        None,
                    ))
                }
            }
            _ => Err(McpError::invalid_params(
                format!("Invalid action: {}", params.action),
                None,
            )),
        }
    }
}
