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

        let title = "Passive Capture Learning".to_string();
        let obs = Observation::new(
            project.id.clone(),
            Scope::Project,
            MemoryType::Learning,
            title,
            params.text,
        );

        self.store
            .storage()
            .save_observation(&obs)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Captured passive learning as Observation ID: {}",
            obs.id
        ))]))
    }
}
