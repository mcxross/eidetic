use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

use crate::storage::MemoryStore;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemMemwalConfigParams {}

#[derive(Clone)]
pub struct MemMemwalConfig {
    store: MemoryStore,
}

impl MemMemwalConfig {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Show redacted Memwal account and backend configuration")]
    pub async fn mem_memwal_config(
        &self,
        Parameters(_params): Parameters<MemMemwalConfigParams>,
    ) -> Result<CallToolResult, McpError> {
        let auth = self.store.auth_manager().ok_or_else(|| {
            McpError::invalid_params(
                "Memwal auth is only available with --storage-backend memwal",
                None,
            )
        })?;
        let snapshot = auth
            .config_snapshot()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let result_json = serde_json::to_string_pretty(&snapshot).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize Memwal config: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(result_json)]))
    }
}
