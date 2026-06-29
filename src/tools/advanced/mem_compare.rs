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
pub struct MemCompareParams {
    #[schemars(description = "Source observation ID")]
    pub source_id: String,
    #[schemars(description = "Target observation ID")]
    pub target_id: String,
    #[schemars(
        description = "Type of relation (duplicate, contradicts, supersedes, extends, references, related)"
    )]
    pub relation_type: RelationType,
    #[schemars(description = "Confidence score (0.0 to 1.0)")]
    pub confidence: f32,
    #[schemars(description = "Reasoning for the relation")]
    pub reasoning: String,
}

#[derive(Clone)]
pub struct MemCompare {
    store: MemoryStore,
}

impl MemCompare {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Persist a semantic relation verdict between two existing observations")]
    pub async fn mem_compare(
        &self,
        Parameters(params): Parameters<MemCompareParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate inputs
        if params.source_id == params.target_id {
            return Err(McpError::invalid_params(
                "Cannot create a relation between an observation and itself",
                None,
            ));
        }
        if params.reasoning.trim().is_empty() {
            return Err(McpError::invalid_params(
                "reasoning must not be empty",
                None,
            ));
        }
        if params.confidence.is_nan() {
            return Err(McpError::invalid_params(
                "confidence must be a valid number, not NaN",
                None,
            ));
        }
        let confidence = params.confidence.clamp(0.0, 1.0);

        // Verify both observations exist
        let storage = self.store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => return Err(McpError::internal_error("mem_compare is not supported on unstructured storage backends like memwal", None)),
        };

        if structured
            .get_observation(&params.source_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .is_none()
        {
            return Err(McpError::invalid_params(
                format!("Source observation not found: {}", params.source_id),
                None,
            ));
        }
        if structured
            .get_observation(&params.target_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .is_none()
        {
            return Err(McpError::invalid_params(
                format!("Target observation not found: {}", params.target_id),
                None,
            ));
        }

        let relation = SemanticRelation {
            id: Uuid::new_v4().to_string(),
            source_id: params.source_id.clone(),
            target_id: params.target_id.clone(),
            relation_type: params.relation_type,
            confidence,
            reasoning: params.reasoning,
            created_at: Utc::now(),
            judged_by: None,
        };

        structured
            .save_relation(&relation)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Persisted semantic relation (ID: {}) between {} and {}",
            relation.id, relation.source_id, relation.target_id
        ))]))
    }
}
