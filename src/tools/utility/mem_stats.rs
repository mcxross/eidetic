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
        let storage = self.store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => return Err(McpError::internal_error("mem_stats is not supported on unstructured storage backends like memwal", None)),
        };

        let project = if let Some(pid) = &params.project_id {
            structured
                .get_project(pid)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Project not found: {}", pid), None)
                })?
        } else {
            // First try to get existing project for cwd
            let cwd = std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if let Ok(Some(project_id)) = self.store.detect_project(Some(cwd)).await {
                structured
                    .get_project(&project_id)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
                    .ok_or_else(|| McpError::internal_error("Failed to load detected project", None))?
            } else {
                // If no project exists for cwd, get/create default project
                self.store
                    .get_or_create_project(None)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
            }
        };

        let stats = structured
            .get_stats(&project.id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let stats_json = serde_json::to_string_pretty(&stats).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize stats: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(stats_json)]))
    }
}
