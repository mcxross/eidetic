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
pub struct MemContextParams {
    #[schemars(description = "Project ID (optional, will auto-detect from cwd if not provided)")]
    pub project_id: Option<String>,
    #[schemars(description = "Number of recent sessions to include (default: 5)")]
    pub session_limit: Option<usize>,
    #[schemars(description = "Number of recent observations to include (default: 20)")]
    pub observation_limit: Option<usize>,
}

#[derive(Clone)]
pub struct MemContext {
    store: MemoryStore,
}

impl MemContext {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Get recent context from previous sessions")]
    pub async fn mem_context(
        &self,
        Parameters(params): Parameters<MemContextParams>,
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

        let project_id = project.id.clone();
        let session_limit = params.session_limit.unwrap_or(5).min(100);
        let observation_limit = params.observation_limit.unwrap_or(20).min(500);

        let recent_sessions = self
            .store
            .storage()
            .get_recent_sessions(&project_id, session_limit)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let recent_observations = self
            .store
            .storage()
            .get_recent_observations(&project_id, observation_limit)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let all_obs = self
            .store
            .storage()
            .list_observations(&project_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let mut topic_counts: std::collections::HashMap<TopicKey, usize> =
            std::collections::HashMap::new();
        for obs in &all_obs {
            if let Some(topic) = &obs.topic_key {
                *topic_counts.entry(topic.clone()).or_insert(0) += 1;
            }
        }
        let mut active_topics: Vec<_> = topic_counts.into_iter().collect();
        active_topics.sort_by_key(|b| std::cmp::Reverse(b.1));
        let active_topics: Vec<TopicKey> =
            active_topics.into_iter().take(10).map(|(t, _)| t).collect();

        let context = SessionContext {
            project_id: project_id.clone(),
            recent_sessions,
            recent_observations,
            active_topics,
        };

        let output = format!(
            "Project: {} ({})\n\nRecent Sessions ({}):\n{}\n\nRecent Observations ({}):\n{}\n\nActive Topics ({}):\n{}",
            project.name,
            project_id,
            context.recent_sessions.len(),
            context
                .recent_sessions
                .iter()
                .map(|s| format!(
                    "  - {} ({}) {}",
                    s.id,
                    s.started_at.to_rfc3339(),
                    s.summary
                        .as_ref()
                        .map(|sum| format!("[Summary: {}]", sum.goal))
                        .unwrap_or_default()
                ))
                .collect::<Vec<_>>()
                .join("\n"),
            context.recent_observations.len(),
            context
                .recent_observations
                .iter()
                .map(|o| format!(
                    "  - [{:?}] {} - {}",
                    o.memory_type,
                    o.title,
                    o.content.chars().take(80).collect::<String>()
                ))
                .collect::<Vec<_>>()
                .join("\n"),
            context.active_topics.len(),
            if context.active_topics.is_empty() {
                "none".to_string()
            } else {
                context.active_topics.join(", ")
            }
        );

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}
