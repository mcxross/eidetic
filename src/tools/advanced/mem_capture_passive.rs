use crate::memory::types::*;
use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemCapturePassiveParams {
    #[schemars(description = "Raw text output to extract learnings from")]
    pub text: String,
    #[schemars(description = "Project ID (optional)")]
    pub project_id: Option<String>,
}

#[derive(Clone)]
pub struct MemCapturePassive {
    store: MemoryStore,
}

impl MemCapturePassive {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Extract learnings from text output")]
    pub async fn mem_capture_passive(
        &self,
        Parameters(params): Parameters<MemCapturePassiveParams>,
    ) -> Result<CallToolResult, McpError> {
        if params.text.trim().is_empty() {
            return Err(McpError::invalid_params("text must not be empty", None));
        }

        let storage = self.store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => return Err(McpError::internal_error("mem_capture_passive is not supported on unstructured storage backends like memwal", None)),
        };

        let project = if let Some(pid) = params.project_id {
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

        let title = {
            let cleaned = params.text.replace('\n', " ");
            let truncated: String = cleaned.chars().take(50).collect();
            if cleaned.chars().count() > 50 {
                format!("Passive: {}...", truncated)
            } else {
                format!("Passive: {}", truncated)
            }
        };
        let obs = Observation::new(
            project.id.clone(),
            Scope::Project,
            MemoryType::Learning,
            title,
            params.text,
        );

        let storage = self.store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => return Err(McpError::internal_error("mem_capture_passive is not supported on unstructured storage backends like memwal", None)),
        };

        structured
            .save_observation(&obs)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Captured passive learning as Observation ID: {}",
            obs.id
        ))]))
    }
}
