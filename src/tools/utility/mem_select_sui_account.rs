use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

use crate::storage::MemoryStore;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemSelectSuiAccountParams {
    #[schemars(description = "Sui account alias or address from ~/.sui")]
    pub selector: String,
}

#[derive(Clone)]
pub struct MemSelectSuiAccount {
    store: MemoryStore,
}

impl MemSelectSuiAccount {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Select the Sui account used by Memwal operations")]
    pub async fn mem_select_sui_account(
        &self,
        Parameters(params): Parameters<MemSelectSuiAccountParams>,
    ) -> Result<CallToolResult, McpError> {
        let auth = self.store.auth_manager().ok_or_else(|| {
            McpError::invalid_params(
                "Memwal auth is only available with --storage-backend memwal",
                None,
            )
        })?;
        let snapshot = auth
            .select_account(&params.selector)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let result_json = serde_json::to_string_pretty(&snapshot).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize Memwal config: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(result_json)]))
    }
}
