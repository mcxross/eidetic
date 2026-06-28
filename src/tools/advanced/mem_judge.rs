use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemJudgeParams {
    #[schemars(description = "ID of the semantic relation to judge")]
    pub relation_id: String,
    #[schemars(description = "Judgment verdict (e.g., 'approved', 'rejected')")]
    pub verdict: String,
}

#[derive(Clone)]
pub struct MemJudge {
    store: MemoryStore,
}

const ALLOWED_VERDICTS: &[&str] = &["approved", "rejected", "deferred", "merged"];

impl MemJudge {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(
        description = "Record a verdict for a pending memory conflict surfaced by mem_save (or relation)"
    )]
    pub async fn mem_judge(
        &self,
        Parameters(params): Parameters<MemJudgeParams>,
    ) -> Result<CallToolResult, McpError> {
        if params.relation_id.trim().is_empty() {
            return Err(McpError::invalid_params(
                "relation_id must not be empty",
                None,
            ));
        }

        let verdict = params.verdict.trim().to_lowercase();
        if !ALLOWED_VERDICTS.contains(&verdict.as_str()) {
            return Err(McpError::invalid_params(
                format!(
                    "Invalid verdict: '{}'. Allowed values: {}",
                    params.verdict,
                    ALLOWED_VERDICTS.join(", ")
                ),
                None,
            ));
        }

        // TODO: Persist verdict once Storage trait supports get_relation_by_id / update_relation.
        // Currently, we validate inputs and acknowledge the verdict but cannot persist it.
        warn!("mem_judge: verdict persistence not yet implemented — relation_id={}, verdict={}", params.relation_id, verdict);

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Recorded verdict '{}' for relation {} (note: relation persistence pending trait extension)",
            verdict, params.relation_id
        ))]))
    }
}
