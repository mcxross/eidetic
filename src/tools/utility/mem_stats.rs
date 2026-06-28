use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemStatsParams {
    #[schemars(description = "Project ID (optional, will auto-detect from cwd if not provided)")]
    pub project_id: Option<String>,
}

#[derive(Clone)]
pub struct MemStats {
    store: MemoryStore,
}

impl MemStats {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Memory system statistics")]
    pub async fn mem_stats(
        &self,
        Parameters(params): Parameters<MemStatsParams>,
    ) -> Result<CallToolResult, McpError> {
        let project = if let Some(ref pid) = params.project_id {
            self.store
                .storage()
                .get_project(pid)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Project not found: {}", pid), None)
                })?
        } else {
            let project_id = self
                .store
                .detect_project(None)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
                .ok_or_else(|| {
                    McpError::invalid_params(
                        "No project detected. Provide project_id or run from a project directory.",
                        None,
                    )
                })?;
            self.store
                .storage()
                .get_project(&project_id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
                .ok_or_else(|| {
                    McpError::invalid_params("Detected project not found in storage", None)
                })?
        };

        let stats = self
            .store
            .storage()
            .get_stats(&project.id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let stats_json = serde_json::to_string_pretty(&stats).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize stats: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(stats_json)]))
    }
}
