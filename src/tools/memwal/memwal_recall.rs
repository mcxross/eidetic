use crate::storage::MemoryStore;
use memwal_core::RecallParams;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ErrorData as McpError};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MemwalRecallParams {
    /// The text query to search for
    pub query: String,
    /// Maximum number of results to return
    pub limit: Option<u64>,
    /// Maximum distance (lower means more similar)
    pub max_distance: Option<f64>,
}

#[derive(Clone)]
pub struct MemwalRecall {
    store: MemoryStore,
}

impl MemwalRecall {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    pub async fn memwal_recall(
        &self,
        Parameters(params): Parameters<MemwalRecallParams>,
    ) -> Result<CallToolResult, McpError> {
        let query = params.query.clone();

        if query.trim().is_empty() {
            return Err(McpError::invalid_params("Query cannot be empty", None));
        }

        let auth_manager = self
            .store
            .auth_manager()
            .ok_or_else(|| McpError::internal_error("AuthManager not available", None))?;

        let memwal_client = auth_manager.memwal_client().await.map_err(|e| {
            McpError::internal_error(format!("Failed to get Memwal client: {}", e), None)
        })?;

        let recall_params = RecallParams {
            query,
            limit: params.limit.map(|l| l as usize),
            max_distance: params.max_distance,
            ..Default::default()
        };

        let results = memwal_client.recall(recall_params).await.map_err(|e| {
            McpError::internal_error(format!("Recall operation failed: {}", e), None)
        })?;

        if results.results.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No matching memories found.",
            )]));
        }

        let mut output = String::from("Memwal Search Results:\n\n");
        for (i, res) in results.results.iter().enumerate() {
            output.push_str(&format!(
                "{}. [Distance: {:.4}] (Blob: {})\n{}\n\n",
                i + 1,
                res.distance,
                res.blob_id,
                res.text
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}
