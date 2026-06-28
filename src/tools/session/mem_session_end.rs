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
pub struct MemSessionEndParams {
    #[schemars(description = "Session ID to end (optional, uses current session if not provided)")]
    pub session_id: Option<String>,
}

#[derive(Clone)]
pub struct MemSessionEnd {
    store: MemoryStore,
}

impl MemSessionEnd {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Mark a session as completed")]
    pub async fn mem_session_end(
        &self,
        Parameters(params): Parameters<MemSessionEndParams>,
    ) -> Result<CallToolResult, McpError> {
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

        let mut session = self
            .store
            .storage()
            .get_session(&session_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(format!("Session not found: {}", session_id), None)
            })?;

        if session.ended_at.is_some() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Session already ended: {}",
                session_id
            ))]));
        }

        session.ended_at = Some(Utc::now());
        self.store
            .storage()
            .update_session(&session)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        // Only clear current session if the ended session was the active one
        if let Some(current) = self.store.get_current_session().await {
            if current == session_id {
                self.store.clear_current_session().await;
            }
        }

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Ended session: {} (duration: {} minutes)",
            session_id,
            (session.ended_at.unwrap_or_else(Utc::now) - session.started_at).num_minutes()
        ))]))
    }
}
