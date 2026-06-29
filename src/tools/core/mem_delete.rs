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
pub struct MemDeleteParams {
    #[schemars(description = "Observation ID to delete")]
    pub id: String,
    #[schemars(description = "Delete mode: soft (default) or hard")]
    pub mode: Option<DeleteMode>,
}

#[derive(Clone)]
pub struct MemDelete {
    store: MemoryStore,
}

impl MemDelete {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Delete an observation (soft-delete by default, hard-delete optional)")]
    pub async fn mem_delete(
        &self,
        Parameters(params): Parameters<MemDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        let storage = self.store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => {
                return Err(McpError::internal_error(
                    "mem_delete is not supported on unstructured storage backends like memwal",
                    None,
                ));
            }
        };

        let obs = structured
            .get_observation(&params.id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(format!("Observation not found: {}", params.id), None)
            })?;

        let mode = params.mode.unwrap_or(DeleteMode::Soft);

        // Idempotent: don't re-delete already soft-deleted observations
        if obs.lifecycle == LifecycleState::Deleted && mode == DeleteMode::Soft {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Observation '{}' (ID: {}) is already soft-deleted",
                obs.title, obs.id
            ))]));
        }

        structured
            .delete_observation(&params.id, mode)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "{} deleted observation: {} (ID: {})",
            match mode {
                DeleteMode::Soft => "Soft",
                DeleteMode::Hard => "Hard",
            },
            obs.title,
            obs.id
        ))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::types::{MemoryType, Observation, Scope};
    use crate::storage::MemoryStore;
    use rmcp::handler::server::wrapper::Parameters;

    #[tokio::test]
    async fn test_mem_delete() {
        let (store, _dir) = MemoryStore::setup_test_store().await;
        let tool = MemDelete::new(store.clone());
        let storage = store.storage();
        let structured = storage.as_structured().unwrap();
        let project = store.get_or_create_project(None).await.unwrap();

        let obs = Observation::new(
            project.id.clone(),
            Scope::Project,
            MemoryType::Note,
            "Delete Me".to_string(),
            "To be deleted".to_string(),
        );
        let obs_id = obs.id.clone();
        structured.save_observation(&obs).await.unwrap();

        // Delete observation
        let params = MemDeleteParams {
            id: obs_id.clone(),
            mode: Some(DeleteMode::Soft),
        };

        let result = tool.mem_delete(Parameters(params)).await;
        assert!(result.is_ok());

        // Verify it was soft-deleted
        let deleted_obs = structured.get_observation(&obs_id).await.unwrap().unwrap();
        assert_eq!(deleted_obs.lifecycle, LifecycleState::Deleted);
    }
}
