use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemSearchParams {
    #[schemars(description = "Project ID (optional, will auto-detect from cwd if not provided)")]
    pub project_id: Option<String>,
    #[schemars(description = "Search query")]
    pub query: String,
    #[schemars(description = "Maximum number of results (default: 20)")]
    pub limit: Option<usize>,
}

#[derive(Clone)]
pub struct MemSearch {
    store: MemoryStore,
}

impl MemSearch {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Full-text search across all memories")]
    pub async fn mem_search(
        &self,
        Parameters(params): Parameters<MemSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        if params.query.trim().is_empty() {
            return Err(McpError::invalid_params("query must not be empty", None));
        }

        let project = if let Some(pid) = params.project_id {
            self.store
                .storage()
                .get_project(&pid)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Project not found: {}", pid), None)
                })?
        } else {
            self.store
                .get_or_create_project(None)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        let project_id = project.id.clone();
        let limit = params.limit.unwrap_or(20).min(500);

        let results = self
            .store
            .storage()
            .search_observations(&project_id, &params.query, limit)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let output = if results.is_empty() {
            "No results found".to_string()
        } else {
            results
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let ra_str = r
                        .observation
                        .review_after
                        .map(|d| format!(" [Review: {}]", d.to_rfc3339()))
                        .unwrap_or_default();
                    format!(
                        "{}. [{:?} | Scope: {:?} | State: {:?}]{} {} (score: {:.1}) - {}",
                        i + 1,
                        r.observation.memory_type,
                        r.observation.scope,
                        r.observation.lifecycle,
                        ra_str,
                        r.observation.title,
                        r.score,
                        r.observation.content.chars().take(100).collect::<String>()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Found {} results for '{}':\n{}",
            results.len(),
            params.query,
            output
        ))]))
    }
}
