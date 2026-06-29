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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemSessionSummaryParams {
    #[schemars(description = "Session ID (optional, uses current session if not provided)")]
    pub session_id: Option<String>,
    #[schemars(description = "Goal of the session")]
    pub goal: String,
    #[schemars(description = "Key discoveries made")]
    pub discoveries: Vec<String>,
    #[schemars(description = "What was accomplished")]
    pub accomplished: Vec<String>,
    #[schemars(description = "Next steps for future sessions")]
    pub next_steps: Vec<String>,
    #[schemars(description = "Files modified during session")]
    pub files_modified: Vec<String>,
}

#[derive(Clone)]
pub struct MemSessionSummary {
    store: MemoryStore,
}

impl MemSessionSummary {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Save end-of-session summary")]
    pub async fn mem_session_summary(
        &self,
        Parameters(params): Parameters<MemSessionSummaryParams>,
    ) -> Result<CallToolResult, McpError> {
        let storage = self.store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => return Err(McpError::internal_error("Sessions are not supported on unstructured storage backends like memwal", None)),
        };

        if params.goal.trim().is_empty() {
            return Err(McpError::invalid_params("goal must not be empty", None));
        }

        let session_id = if let Some(sid) = params.session_id {
            sid
        } else {
            self.store.get_current_session().await.ok_or_else(|| {
                McpError::invalid_params(
                    "No active session found. Provide session_id or start a session first."
                        .to_string(),
                    None,
                )
            })?
        };

        let mut session = structured
            .get_session(&session_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(format!("Session not found: {}", session_id), None)
            })?;

        let summary = SessionSummary {
            goal: params.goal,
            discoveries: params.discoveries,
            accomplished: params.accomplished,
            next_steps: params.next_steps,
            files_modified: params.files_modified,
            created_at: Utc::now(),
        };

        session.summary = Some(summary);
        session.ended_at = Some(Utc::now());
        structured
            .update_session(&session)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        // Only clear current session if the target session was the active one
        if let Some(current) = self.store.get_current_session().await
            && current == session_id
        {
            self.store.clear_current_session().await;
        }

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Saved session summary for: {}",
            session_id
        ))]))
    }
}
