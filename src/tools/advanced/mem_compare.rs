use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    tool,
    schemars::JsonSchema,
};
use serde::{Deserialize, Serialize};
use crate::storage::MemoryStore;
use crate::memory::types::*;
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemCompareParams {
    #[schemars(description = "Source observation ID")]
    pub source_id: String,
    #[schemars(description = "Target observation ID")]
    pub target_id: String,
    #[schemars(description = "Type of relation (duplicate, contradicts, supersedes, extends, references, related)")]
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
        let relation = SemanticRelation {
            id: Uuid::new_v4().to_string(),
            source_id: params.source_id.clone(),
            target_id: params.target_id.clone(),
            relation_type: params.relation_type,
            confidence: params.confidence,
            reasoning: params.reasoning,
            created_at: Utc::now(),
            judged_by: None,
        };

        self.store.storage().save_relation(&relation).await.map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Persisted semantic relation (ID: {}) between {} and {}",
            relation.id, relation.source_id, relation.target_id
        ))]))
    }
}
