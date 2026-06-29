use crate::storage::MemoryStore;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ErrorData as McpError};
use schemars::JsonSchema;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MemwalRememberParams {
    /// The text to store in the Memwal backend
    pub text: String,
    /// Wait for the memory storage to complete (default: true). If false, returns a job_id immediately.
    pub wait_for_completion: Option<bool>,
}

#[derive(Clone)]
pub struct MemwalRemember {
    store: MemoryStore,
}

impl MemwalRemember {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    pub async fn memwal_remember(
        &self,
        Parameters(params): Parameters<MemwalRememberParams>,
    ) -> Result<CallToolResult, McpError> {
        let text = params.text.clone();
        let wait = params.wait_for_completion.unwrap_or(true);

        if text.trim().is_empty() {
            return Err(McpError::invalid_params("Text cannot be empty", None));
        }

        let auth_manager = self
            .store
            .auth_manager()
            .ok_or_else(|| McpError::internal_error("AuthManager not available", None))?;

        let memwal_client = auth_manager.memwal_client().await.map_err(|e| {
            McpError::internal_error(format!("Failed to get Memwal client: {}", e), None)
        })?;

        if wait {
            let res = memwal_client
                .remember(&text, Duration::from_secs(1), Duration::from_secs(60))
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("Remember operation failed: {}", e), None)
                })?;

            Ok(CallToolResult::success(vec![Content::text(format!(
                "Successfully stored memory in Memwal. Job ID: {}",
                res.job_id
            ))]))
        } else {
            let res = memwal_client.remember_async(&text).await.map_err(|e| {
                McpError::internal_error(format!("Remember async operation failed: {}", e), None)
            })?;

            Ok(CallToolResult::success(vec![Content::text(format!(
                "Successfully initiated memory storage in Memwal. Job ID: {}",
                res.job_id
            ))]))
        }
    }
}
