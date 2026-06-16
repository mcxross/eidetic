use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    tool,
    schemars::JsonSchema,
};
use serde::{Deserialize, Serialize};
use crate::memory::types::*;
use crate::storage::MemoryStore;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemDeleteParams {
    #[schemars(description = "Observation ID to delete")]
    pub id: String,
    #[schemars(description = "Delete mode: soft (default) or hard")]
    pub mode: Option<DeleteMode>,
}

#[derive(Clone)]
pub struct MemDelete {
    store: MemoryStore,
}

impl MemDelete {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Delete an observation (soft-delete by default, hard-delete optional)")]
    pub async fn mem_delete(
        &self,
        Parameters(params): Parameters<MemDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        let obs = self.store.storage().get_observation(&params.id).await.map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params(format!("Observation not found: {}", params.id), None))?;

        let mode = params.mode.unwrap_or(DeleteMode::Soft);
        self.store.storage().delete_observation(&params.id, mode).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "{} deleted observation: {} (ID: {})",
            match mode {
                DeleteMode::Soft => "Soft",
                DeleteMode::Hard => "Hard",
            },
            obs.title, obs.id
        ))]))
    }
}
