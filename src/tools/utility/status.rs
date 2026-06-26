use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusParams {}

#[derive(Clone)]
pub struct EideticStatus {
    store: MemoryStore,
}

impl EideticStatus {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(
        description = "Check the current configuration and funding status of the Eidetic memory server. Call this tool first if you are about to use Eidetic for the first time, or if you encounter any authentication or gas errors."
    )]
    pub async fn eidetic_status(
        &self,
        Parameters(_params): Parameters<StatusParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut status = serde_json::json!({
            "storage_backend": "unknown",
            "memwal_status": "not_applicable",
        });

        if let Some(auth_mgr) = self.store.auth_manager() {
            let snap = auth_mgr.config_snapshot().await.map_err(|e| {
                McpError::internal_error(format!("Failed to get config snapshot: {}", e), None)
            })?;

            status["storage_backend"] = serde_json::json!(snap.backend);
            status["selected_address"] = serde_json::json!(snap.selected_address);

            if snap.backend == "memwal" {
                match auth_mgr.memwal_client().await {
                    Ok(_) => {
                        status["memwal_status"] = serde_json::json!("provisioned_and_ready");
                        status["memwal_account_id"] = serde_json::json!(snap.memwal_account_id);
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        if msg.contains("Insufficient gas")
                            || msg.contains("GasBalanceTooLow")
                            || msg.contains("faucet.sui.io")
                        {
                            status["memwal_status"] = serde_json::json!("needs_funding");
                            status["funding_instructions"] = serde_json::json!(msg);
                            status["llm_instruction"] = serde_json::json!(
                                "The user's Memwal account is not provisioned due to insufficient gas. Please ask the user to fund the address shown above, wait for their confirmation, and then retry your tool call."
                            );
                        } else {
                            status["memwal_status"] = serde_json::json!("error");
                            status["error"] = serde_json::json!(msg);
                        }
                    }
                }
            }
        } else {
            status["storage_backend"] = serde_json::json!("local_database (sqlite/file)");
            status["memwal_status"] =
                serde_json::json!("Not using Memwal. No setup or funding required.");
        }

        let result_json = serde_json::to_string_pretty(&status).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize result: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(result_json)]))
    }
}
