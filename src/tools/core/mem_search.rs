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

        let storage = self.store.storage();

        if let Some(structured) = storage.as_structured() {
            let project = if let Some(pid) = &params.project_id {
                structured
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

            let results = structured
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
        } else if let Some(unstructured) = storage.as_unstructured() {
            let namespace = if let Some(pid) = &params.project_id {
                crate::memory::types::Project::canonicalize(pid)
            } else {
                let project = self
                    .store
                    .get_or_create_project(None)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                project.id
            };

            let limit = params.limit.unwrap_or(20).min(500);
            
            let results = unstructured
                .recall(&params.query, Some(&namespace))
                .await
                .map_err(|e| McpError::internal_error(format!("Memwal recall failed: {}", e), None))?;

            let output = if results.is_empty() {
                "No results found".to_string()
            } else {
                results
                    .iter()
                    .enumerate()
                    .map(|(i, text)| {
                        format!("{}. {}", i + 1, text.chars().take(200).collect::<String>())
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            };

            Ok(CallToolResult::success(vec![Content::text(format!(
                "Found {} results for '{}' from Memwal network:\n{}",
                results.len(),
                params.query,
                output
            ))]))
        } else {
            Err(McpError::internal_error("Storage backend has no known capabilities", None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::types::{MemoryType, Observation, Scope};
    use crate::storage::MemoryStore;

    #[tokio::test]
    async fn test_mem_search() {
        let (store, _dir) = MemoryStore::setup_test_store().await;
        let storage = store.storage();
        let structured = storage.as_structured().unwrap();
        let project = store.get_or_create_project(None).await.unwrap();

        let obs = Observation::new(
            project.id.clone(),
            Scope::Project,
            MemoryType::Note,
            "Rust async".to_string(),
            "Rust async uses Future trait.".to_string(),
        );
        structured.save_observation(&obs).await.unwrap();

        let tool = MemSearch::new(store.clone());

        // Test search
        let params = MemSearchParams {
            project_id: None,
            query: "async".to_string(),
            limit: Some(10),
        };

        let result = tool.mem_search(Parameters(params)).await;
        assert!(result.is_ok());
        let res = result.unwrap();
        let content_str = format!("{:?}", res.content[0]);
        assert!(content_str.contains("Rust async"));
    }
}
