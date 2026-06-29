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
pub struct MemDoctorParams {
    #[schemars(description = "Working directory path to detect project")]
    pub cwd: Option<String>,
}

#[derive(Clone)]
pub struct MemDoctor {
    store: MemoryStore,
}

impl MemDoctor {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(
        description = "Run read-only operational diagnostics for project detection and store health"
    )]
    pub async fn mem_doctor(
        &self,
        Parameters(params): Parameters<MemDoctorParams>,
    ) -> Result<CallToolResult, McpError> {
        let cwd = params.cwd.unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

        let health = self
            .store
            .storage()
            .health_check()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let mut issues = Vec::new();

        let mut detection = ProjectDetectionResult {
            detected_project: None,
            confidence: 0.0,
            candidates: Vec::new(),
        };

        if let Ok(Some(pid)) = self.store.detect_project(Some(cwd.clone())).await {
            if let Some(structured) = self.store.storage().as_structured()
                && let Ok(Some(project)) = structured.get_project(&pid).await
            {
                detection.detected_project = Some(pid.clone());
                detection.confidence = 1.0;
                detection.candidates.push(ProjectCandidate {
                    project_id: pid,
                    name: project.name,
                    path: project.path,
                    match_reason: "Exact or path match".to_string(),
                    score: 1.0,
                });
            }
        } else {
            issues.push(DiagnosticIssue {
                severity: IssueSeverity::Info,
                category: "project_detection".to_string(),
                message: format!("No project found for path: {}", cwd),
                suggestion: Some(
                    "mem_save or mem_session_start will create a new project automatically"
                        .to_string(),
                ),
            });
        }

        if health.orphaned_observations > 0 {
            issues.push(DiagnosticIssue {
                severity: IssueSeverity::Warning,
                category: "health".to_string(),
                message: format!(
                    "Found {} orphaned observations",
                    health.orphaned_observations
                ),
                suggestion: None,
            });
        }

        let mut recommendations = Vec::new();
        if health.orphaned_observations > 0 || health.orphaned_sessions > 0 {
            recommendations.push("Review orphaned data".to_string());
        }

        let doctor_result = DoctorResult {
            project_detection: detection,
            store_health: health,
            issues,
            recommendations,
        };

        let result_json = serde_json::to_string_pretty(&doctor_result).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize result: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(result_json)]))
    }
}
