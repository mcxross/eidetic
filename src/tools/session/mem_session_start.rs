use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    tool,
    schemars::JsonSchema,
};
use serde::{Deserialize, Serialize};
use crate::memory::types::*;
use crate::storage::MemoryStore;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemSessionStartParams {
    #[schemars(description = "Project ID (optional, will auto-detect from cwd if not provided)")]
    pub project_id: Option<String>,
    #[schemars(description = "Optional session name/description")]
    pub name: Option<String>,
}

#[derive(Clone)]
pub struct MemSessionStart {
    store: MemoryStore,
}

impl MemSessionStart {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Register a session start")]
    pub async fn mem_session_start(
        &self,
        Parameters(params): Parameters<MemSessionStartParams>,
    ) -> Result<CallToolResult, McpError> {
        let project = if let Some(pid) = params.project_id {
            self.store.storage().get_project(&pid).await.map_err(|e| McpError::internal_error(e.to_string(), None))?
                .ok_or_else(|| McpError::invalid_params(format!("Project not found: {}", pid), None))?
        } else {
            self.store.get_or_create_project(None).await.map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        let project_id = project.id.clone();

        let existing_sessions = self.store.storage().list_sessions(&project_id).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;
        if let Some(active) = existing_sessions.iter().find(|s| s.ended_at.is_none()) {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Session already active: {} (ID: {})",
                params.name.unwrap_or_else(|| "unnamed".to_string()),
                active.id
            ))]));
        }

        let session = Session::new(project_id.clone());
        self.store.storage().save_session(&session).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;

        self.store.set_current_session(session.id.clone()).await;

        let session_limit = 5;
        let observation_limit = 20;

        let recent_sessions = self.store.storage().get_recent_sessions(&project_id, session_limit).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let recent_observations = self.store.storage().get_recent_observations(&project_id, observation_limit).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;
        
        let all_obs = self.store.storage().list_observations(&project_id).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let mut topic_counts: std::collections::HashMap<TopicKey, usize> = std::collections::HashMap::new();
        for obs in &all_obs {
            if let Some(topic) = &obs.topic_key {
                *topic_counts.entry(topic.clone()).or_insert(0) += 1;
            }
        }
        let mut active_topics: Vec<_> = topic_counts.into_iter().collect();
        active_topics.sort_by(|a, b| b.1.cmp(&a.1));
        let active_topics: Vec<TopicKey> = active_topics.into_iter().take(10).map(|(t, _)| t).collect();

        let context_output = format!(
            "Project: {} ({})\n\nRecent Sessions ({}):\n{}\n\nRecent Observations ({}):\n{}\n\nActive Topics ({}):\n{}",
            project.name,
            project_id,
            recent_sessions.len(),
            recent_sessions.iter().map(|s| format!(
                "  - {} ({}) {}",
                s.id,
                s.started_at.to_rfc3339(),
                s.summary.as_ref().map(|sum| format!("[Summary: {}]", sum.goal)).unwrap_or_default()
            )).collect::<Vec<_>>().join("\n"),
            recent_observations.len(),
            recent_observations.iter().map(|o| format!(
                "  - [{:?}] {} - {}",
                o.memory_type, o.title, o.content.chars().take(80).collect::<String>()
            )).collect::<Vec<_>>().join("\n"),
            active_topics.len(),
            if active_topics.is_empty() { "none".to_string() } else { active_topics.join(", ") }
        );

        let final_output = format!(
            "Started new session: {} (ID: {})\n\n--- PREVIOUS SESSION CONTEXT ---\n{}",
            params.name.unwrap_or_else(|| "unnamed".to_string()),
            session.id,
            context_output
        );

        Ok(CallToolResult::success(vec![Content::text(final_output)]))
    }
}
