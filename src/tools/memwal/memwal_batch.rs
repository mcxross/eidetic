use crate::storage::MemoryStore;
use memwal_core::types::RememberBulkItem;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ErrorData as McpError};
use schemars::JsonSchema;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MemwalBatchParams {
    /// List of text items to store in the Memwal backend
    pub items: Vec<String>,
    /// The namespace to store the memories in (defaults to 'default')
    pub namespace: Option<String>,
    /// Wait for the bulk memory storage to complete (default: true). If false, returns a list of job_ids immediately.
    pub wait_for_completion: Option<bool>,
}

#[derive(Clone)]
pub struct MemwalBatch {
    store: MemoryStore,
}

impl MemwalBatch {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    pub async fn memwal_batch(
        &self,
        Parameters(params): Parameters<MemwalBatchParams>,
    ) -> Result<CallToolResult, McpError> {
        let items = params.items.clone();
        let wait = params.wait_for_completion.unwrap_or(true);

        if items.is_empty() {
            return Err(McpError::invalid_params(
                "Items array cannot be empty",
                None,
            ));
        }

        let auth_manager = self
            .store
            .auth_manager()
            .ok_or_else(|| McpError::internal_error("AuthManager not available", None))?;

        let memwal_client = auth_manager.memwal_client().await.map_err(|e| {
            McpError::internal_error(format!("Failed to get Memwal client: {}", e), None)
        })?;

        let bulk_items: Vec<RememberBulkItem> = items
            .into_iter()
            .map(|text| RememberBulkItem {
                text,
                namespace: params.namespace.clone(),
            })
            .collect();

        // `memwal-rs` allows bulk remember with vector of items
        if wait {
            let res = memwal_client
                .remember_bulk_and_wait(
                    &bulk_items,
                    Duration::from_secs(1),
                    Duration::from_secs(120),
                )
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("Bulk remember operation failed: {}", e), None)
                })?;

            Ok(CallToolResult::success(vec![Content::text(format!(
                "Successfully stored {} memories in Memwal. Bulk State: {:?}",
                bulk_items.len(),
                res
            ))]))
        } else {
            let res = memwal_client
                .remember_bulk(&bulk_items)
                .await
                .map_err(|e| {
                    McpError::internal_error(
                        format!("Bulk remember async operation failed: {}", e),
                        None,
                    )
                })?;

            Ok(CallToolResult::success(vec![Content::text(format!(
                "Successfully initiated bulk memory storage in Memwal. Job IDs: {:?}",
                res
            ))]))
        }
    }
}
