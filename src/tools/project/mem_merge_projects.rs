use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemMergeProjectsParams {
    #[schemars(description = "Canonical project ID to merge into")]
    pub canonical_project_id: String,
    #[schemars(description = "Project IDs to merge (aliases)")]
    pub alias_project_ids: Vec<String>,
}

#[derive(Clone)]
pub struct MemMergeProjects {
    store: MemoryStore,
}

impl MemMergeProjects {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Merge project name variants into canonical name (admin)")]
    pub async fn mem_merge_projects(
        &self,
        Parameters(params): Parameters<MemMergeProjectsParams>,
    ) -> Result<CallToolResult, McpError> {
        let storage = self.store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => {
                return Err(McpError::internal_error(
                    "mem_merge_projects is not supported on unstructured storage backends like memwal",
                    None,
                ));
            }
        };

        if params
            .alias_project_ids
            .contains(&params.canonical_project_id)
        {
            return Err(McpError::invalid_params(
                "Cannot merge a project into itself: canonical_project_id must not appear in alias_project_ids",
                None,
            ));
        }

        let mut canonical = structured
            .get_project(&params.canonical_project_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!(
                        "Canonical project not found: {}",
                        params.canonical_project_id
                    ),
                    None,
                )
            })?;

        let mut merged_count = 0;
        for alias_id in &params.alias_project_ids {
            if let Some(alias_proj) = structured
                .get_project(alias_id)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
            {
                let observations = structured
                    .list_observations(&alias_proj.id)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                for mut obs in observations {
                    obs.project_id = canonical.id.clone();
                    structured
                        .update_observation(&obs)
                        .await
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                }

                let sessions = structured
                    .list_sessions(&alias_proj.id)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                for mut sess in sessions {
                    sess.project_id = canonical.id.clone();
                    structured
                        .update_session(&sess)
                        .await
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                }

                if !canonical.aliases.contains(&alias_proj.name) {
                    canonical.aliases.push(alias_proj.name.clone());
                }

                let mut alias_proj = alias_proj;
                alias_proj.active = false;
                structured
                    .update_project(&alias_proj)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                merged_count += 1;
            }
        }

        structured
            .update_project(&canonical)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Merged {} projects into {} ({})",
            merged_count, canonical.name, canonical.id
        ))]))
    }
}
