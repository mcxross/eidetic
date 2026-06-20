use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ErrorData as McpError, Implementation, ServerCapabilities, ServerInfo,
    },
    tool, tool_handler, tool_router,
};

use crate::storage::MemoryStore;
use crate::tools::*;

#[derive(Clone)]
pub struct EideticServer {
    tool_router: ToolRouter<Self>,
    store: MemoryStore,
    mem_save: MemSave,
    mem_update: MemUpdate,
    mem_delete: MemDelete,
    mem_suggest_topic_key: MemSuggestTopicKey,
    mem_search: MemSearch,
    mem_session_summary: MemSessionSummary,
    mem_context: MemContext,
    mem_timeline: MemTimeline,
    mem_get_observation: MemGetObservation,
    mem_save_prompt: MemSavePrompt,
    mem_stats: MemStats,
    mem_session_start: MemSessionStart,
    mem_session_end: MemSessionEnd,
    mem_capture_passive: MemCapturePassive,
    mem_merge_projects: MemMergeProjects,
    mem_current_project: MemCurrentProject,
    mem_doctor: MemDoctor,
    mem_sui_accounts: MemSuiAccounts,
    mem_select_sui_account: MemSelectSuiAccount,
    mem_memwal_config: MemMemwalConfig,
    mem_review: MemReview,
    mem_judge: MemJudge,
    mem_compare: MemCompare,
}

impl EideticServer {
    pub fn new(store: MemoryStore) -> Self {
        Self {
            tool_router: Self::tool_router(),
            mem_save: MemSave::new(store.clone()),
            mem_update: MemUpdate::new(store.clone()),
            mem_delete: MemDelete::new(store.clone()),
            mem_suggest_topic_key: MemSuggestTopicKey::new(store.clone()),
            mem_search: MemSearch::new(store.clone()),
            mem_session_summary: MemSessionSummary::new(store.clone()),
            mem_context: MemContext::new(store.clone()),
            mem_timeline: MemTimeline::new(store.clone()),
            mem_get_observation: MemGetObservation::new(store.clone()),
            mem_save_prompt: MemSavePrompt::new(store.clone()),
            mem_stats: MemStats::new(store.clone()),
            mem_session_start: MemSessionStart::new(store.clone()),
            mem_session_end: MemSessionEnd::new(store.clone()),
            mem_capture_passive: MemCapturePassive::new(store.clone()),
            mem_merge_projects: MemMergeProjects::new(store.clone()),
            mem_current_project: MemCurrentProject::new(store.clone()),
            mem_doctor: MemDoctor::new(store.clone()),
            mem_sui_accounts: MemSuiAccounts::new(store.clone()),
            mem_select_sui_account: MemSelectSuiAccount::new(store.clone()),
            mem_memwal_config: MemMemwalConfig::new(store.clone()),
            mem_review: MemReview::new(store.clone()),
            mem_judge: MemJudge::new(store.clone()),
            mem_compare: MemCompare::new(store.clone()),
            store,
        }
    }
}

#[tool_router]
impl EideticServer {
    #[tool(
        description = "Save a structured observation (decision, bugfix, pattern, etc.); best-effort captures process-local current prompt context when available unless capture_prompt=false"
    )]
    async fn mem_save(
        &self,
        params: Parameters<crate::tools::core::mem_save::MemSaveParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_save.mem_save(params).await
    }

    #[tool(description = "Update an existing observation by ID")]
    async fn mem_update(
        &self,
        params: Parameters<crate::tools::core::mem_update::MemUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_update.mem_update(params).await
    }

    #[tool(description = "Delete an observation (soft-delete by default, hard-delete optional)")]
    async fn mem_delete(
        &self,
        params: Parameters<crate::tools::core::mem_delete::MemDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_delete.mem_delete(params).await
    }

    #[tool(description = "Suggest a stable topic_key for evolving topics before saving")]
    async fn mem_suggest_topic_key(
        &self,
        params: Parameters<crate::tools::core::mem_suggest_topic_key::MemSuggestTopicKeyParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_suggest_topic_key
            .mem_suggest_topic_key(params)
            .await
    }

    #[tool(description = "Full-text search across all memories")]
    async fn mem_search(
        &self,
        params: Parameters<crate::tools::core::mem_search::MemSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_search.mem_search(params).await
    }

    #[tool(description = "Get full content of a specific memory")]
    async fn mem_get_observation(
        &self,
        params: Parameters<crate::tools::core::mem_get_observation::MemGetObservationParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_get_observation.mem_get_observation(params).await
    }

    #[tool(description = "Save a user prompt for future context")]
    async fn mem_save_prompt(
        &self,
        params: Parameters<crate::tools::core::mem_save_prompt::MemSavePromptParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_save_prompt.mem_save_prompt(params).await
    }

    #[tool(description = "Register a session start")]
    async fn mem_session_start(
        &self,
        params: Parameters<crate::tools::session::mem_session_start::MemSessionStartParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_session_start.mem_session_start(params).await
    }

    #[tool(description = "Mark a session as completed")]
    async fn mem_session_end(
        &self,
        params: Parameters<crate::tools::session::mem_session_end::MemSessionEndParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_session_end.mem_session_end(params).await
    }

    #[tool(description = "Get recent context from previous sessions")]
    async fn mem_context(
        &self,
        params: Parameters<crate::tools::session::mem_context::MemContextParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_context.mem_context(params).await
    }

    #[tool(description = "Save end-of-session summary")]
    async fn mem_session_summary(
        &self,
        params: Parameters<crate::tools::session::mem_session_summary::MemSessionSummaryParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_session_summary.mem_session_summary(params).await
    }

    #[tool(description = "Merge project name variants into canonical name (admin)")]
    async fn mem_merge_projects(
        &self,
        params: Parameters<crate::tools::project::mem_merge_projects::MemMergeProjectsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_merge_projects.mem_merge_projects(params).await
    }

    #[tool(description = "Detect project from cwd — never errors, recommended first call")]
    async fn mem_current_project(
        &self,
        params: Parameters<crate::tools::project::mem_current_project::MemCurrentProjectParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_current_project.mem_current_project(params).await
    }

    #[tool(description = "Get chronological context around a specific observation")]
    async fn mem_timeline(
        &self,
        params: Parameters<crate::tools::advanced::mem_timeline::MemTimelineParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_timeline.mem_timeline(params).await
    }

    #[tool(description = "Extract learnings from text output")]
    async fn mem_capture_passive(
        &self,
        params: Parameters<crate::tools::advanced::mem_capture_passive::MemCapturePassiveParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_capture_passive.mem_capture_passive(params).await
    }

    #[tool(
        description = "List observations whose review_after lifecycle is stale; mark_reviewed resets the local review cycle"
    )]
    async fn mem_review(
        &self,
        params: Parameters<crate::tools::advanced::mem_review::MemReviewParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_review.mem_review(params).await
    }

    #[tool(description = "Record a verdict for a pending memory conflict surfaced by mem_save")]
    async fn mem_judge(
        &self,
        params: Parameters<crate::tools::advanced::mem_judge::MemJudgeParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_judge.mem_judge(params).await
    }

    #[tool(description = "Persist a semantic relation verdict between two existing observations")]
    async fn mem_compare(
        &self,
        params: Parameters<crate::tools::advanced::mem_compare::MemCompareParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_compare.mem_compare(params).await
    }

    #[tool(description = "Memory system statistics")]
    async fn mem_stats(
        &self,
        params: Parameters<crate::tools::utility::mem_stats::MemStatsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_stats.mem_stats(params).await
    }

    #[tool(
        description = "Run read-only operational diagnostics for project detection and store health"
    )]
    async fn mem_doctor(
        &self,
        params: Parameters<crate::tools::utility::mem_doctor::MemDoctorParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_doctor.mem_doctor(params).await
    }

    #[tool(description = "List Sui accounts available from ~/.sui for Memwal operations")]
    async fn mem_sui_accounts(
        &self,
        params: Parameters<crate::tools::utility::mem_sui_accounts::MemSuiAccountsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_sui_accounts.mem_sui_accounts(params).await
    }

    #[tool(description = "Select the Sui account used by Memwal operations")]
    async fn mem_select_sui_account(
        &self,
        params: Parameters<
            crate::tools::utility::mem_select_sui_account::MemSelectSuiAccountParams,
        >,
    ) -> Result<CallToolResult, McpError> {
        self.mem_select_sui_account
            .mem_select_sui_account(params)
            .await
    }

    #[tool(description = "Show redacted Memwal account and backend configuration")]
    async fn mem_memwal_config(
        &self,
        params: Parameters<crate::tools::utility::mem_memwal_config::MemMemwalConfigParams>,
    ) -> Result<CallToolResult, McpError> {
        self.mem_memwal_config.mem_memwal_config(params).await
    }
}

#[tool_handler]
impl ServerHandler for EideticServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "eidetic-mcp-server".into(),
                version: "0.1.0".into(),
                title: None,
                description: Some(
                    "Eidetic MCP Server - Memory management for agentic workflows".into(),
                ),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Eidetic MCP Server handles project memory and observation storage.".into(),
            ),
        }
    }
}
