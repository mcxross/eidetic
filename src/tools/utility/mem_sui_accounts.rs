use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

use crate::storage::MemoryStore;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemSuiAccountsParams {}

#[derive(Clone)]
pub struct MemSuiAccounts {
    store: MemoryStore,
}

impl MemSuiAccounts {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "List Sui accounts available from ~/.sui for Memwal operations")]
    pub async fn mem_sui_accounts(
        &self,
        Parameters(_params): Parameters<MemSuiAccountsParams>,
    ) -> Result<CallToolResult, McpError> {
        let auth = self.store.auth_manager().ok_or_else(|| {
            McpError::invalid_params(
                "Memwal auth is only available with --storage-backend memwal",
                None,
            )
        })?;
        let accounts = auth
            .list_sui_accounts()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let result_json = serde_json::to_string_pretty(&accounts).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize accounts: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(result_json)]))
    }
}
