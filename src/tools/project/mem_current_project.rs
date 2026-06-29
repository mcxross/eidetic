use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemCurrentProjectParams {
    #[schemars(
        description = "Working directory to detect project from (optional, defaults to cwd)"
    )]
    pub cwd: Option<String>,
}

#[derive(Clone)]
pub struct MemCurrentProject {
    store: MemoryStore,
}

impl MemCurrentProject {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Detect project from cwd — never errors, recommended first call")]
    pub async fn mem_current_project(
        &self,
        Parameters(params): Parameters<MemCurrentProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        let cwd = params.cwd.unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

        let detected = self
            .store
            .detect_project(Some(cwd.clone()))
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let storage = self.store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => return Err(McpError::internal_error("mem_current_project is not supported on unstructured storage backends like memwal", None)),
        };

        let project = if let Some(project_id) = detected {
            structured
                .get_project(&project_id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            Some(
                self.store
                    .get_or_create_project(Some(cwd.clone()))
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?,
            )
        };

        if let Some(proj) = project {
            let output = format!(
                "Project: {}\nID: {}\nPath: {}\nCanonical: {}\nAliases: {}\nActive: {}",
                proj.name,
                proj.id,
                proj.path,
                proj.canonical_name,
                proj.aliases.join(", "),
                proj.active
            );
            Ok(CallToolResult::success(vec![Content::text(output)]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(
                "No project detected. Run mem_current_project with a cwd to create one.",
            )]))
        }
    }
}
