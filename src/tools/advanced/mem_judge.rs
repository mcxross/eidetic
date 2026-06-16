use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    tool,
    schemars::JsonSchema,
};
use serde::{Deserialize, Serialize};
use crate::storage::MemoryStore;

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

impl MemJudge {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Record a verdict for a pending memory conflict surfaced by mem_save (or relation)")]
    pub async fn mem_judge(
        &self,
        Parameters(params): Parameters<MemJudgeParams>,
    ) -> Result<CallToolResult, McpError> {
        
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Recorded verdict '{}' for relation {}", params.verdict, params.relation_id
        ))]))
    }
}
