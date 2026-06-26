use crate::memory::types::*;
use crate::storage::MemoryStore;
use chrono::Utc;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemSavePromptParams {
    #[schemars(description = "The prompt to save")]
    pub prompt: String,
    #[schemars(description = "Optional context surrounding the prompt")]
    pub context: Option<String>,
    #[schemars(description = "Project ID (optional, will auto-detect from cwd if not provided)")]
    pub project_id: Option<String>,
    #[schemars(description = "Session ID to associate with (optional)")]
    pub session_id: Option<String>,
}

#[derive(Clone)]
pub struct MemSavePrompt {
    store: MemoryStore,
}

impl MemSavePrompt {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Save a user prompt for future context")]
    pub async fn mem_save_prompt(
        &self,
        Parameters(params): Parameters<MemSavePromptParams>,
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

        let saved_prompt = SavedPrompt {
            id: Uuid::new_v4().to_string(),
            project_id: project.id.clone(),
            session_id: params.session_id,
            prompt: params.prompt,
            context: params.context,
            created_at: Utc::now(),
        };

        self.store
            .storage()
            .save_prompt(&saved_prompt)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Saved prompt with ID: {}",
            saved_prompt.id
        ))]))
    }
}
