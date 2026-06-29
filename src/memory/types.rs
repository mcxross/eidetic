use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub type ObservationId = String;

pub type SessionId = String;

pub type ProjectId = String;

pub type TopicKey = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Project,
    Personal,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    Decision,
    Bugfix,
    Pattern,
    Discovery,
    Reference,
    Task,
    Note,
    CodeSnippet,
    Error,
    Learning,
    Artifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Active,
    Review,
    Archived,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeleteMode {
    Soft,
    Hard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub project_id: ProjectId,
    pub session_id: Option<SessionId>,
    pub topic_key: Option<TopicKey>,
    pub memory_type: MemoryType,
    pub scope: Scope,
    pub title: String,
    pub content: String,
    pub hash: String,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub lifecycle: LifecycleState,
    pub revision_count: u32,
    pub duplicate_count: u32,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_after: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_mode: Option<DeleteMode>,
    pub related_observations: Vec<ObservationId>,
    pub source_prompt: Option<String>,
    pub capture_prompt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub project_id: ProjectId,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub summary: Option<SessionSummary>,
    pub context_injected: bool,
    pub observation_ids: Vec<ObservationId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub goal: String,
    pub discoveries: Vec<String>,
    pub accomplished: Vec<String>,
    pub next_steps: Vec<String>,
    pub files_modified: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: String,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPrompt {
    pub id: String,
    pub project_id: ProjectId,
    pub session_id: Option<SessionId>,
    pub prompt: String,
    pub context: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub observation: Observation,
    pub score: f32,
    pub matched_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_observations: usize,
    pub active_observations: usize,
    pub archived_observations: usize,
    pub deleted_observations: usize,
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub total_projects: usize,
    pub storage_size_bytes: u64,
    pub oldest_observation: Option<DateTime<Utc>>,
    pub newest_observation: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicSuggestion {
    pub suggested_key: TopicKey,
    pub confidence: f32,
    pub existing_similar: Vec<TopicKey>,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub observation: Observation,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub project_id: ProjectId,
    pub recent_sessions: Vec<Session>,
    pub recent_observations: Vec<Observation>,
    pub active_topics: Vec<TopicKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorResult {
    pub project_detection: ProjectDetectionResult,
    pub store_health: StoreHealth,
    pub issues: Vec<DiagnosticIssue>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetectionResult {
    pub detected_project: Option<ProjectId>,
    pub confidence: f32,
    pub candidates: Vec<ProjectCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCandidate {
    pub project_id: ProjectId,
    pub name: String,
    pub path: String,
    pub match_reason: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreHealth {
    pub readable: bool,
    pub writable: bool,
    pub corruption_detected: bool,
    pub orphaned_observations: usize,
    pub orphaned_sessions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticIssue {
    pub severity: IssueSeverity,
    pub category: String,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingConflict {
    pub id: String,
    pub observation_id: ObservationId,
    pub conflict_type: ConflictType,
    pub description: String,
    pub suggested_resolution: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    Duplicate,
    Contradiction,
    Superseded,
    Related,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticRelation {
    pub id: String,
    pub source_id: ObservationId,
    pub target_id: ObservationId,
    pub relation_type: RelationType,
    pub confidence: f32,
    pub reasoning: String,
    pub created_at: DateTime<Utc>,
    pub judged_by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    Duplicate,
    Contradicts,
    Supersedes,
    Extends,
    References,
    Related,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    pub observation: Observation,
    pub days_stale: i64,
    pub review_after: DateTime<Utc>,
}

impl Observation {
    pub fn compute_hash(
        project_id: &str,
        scope: &Scope,
        memory_type: &MemoryType,
        title: &str,
        content: &str,
    ) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(project_id.as_bytes());
        hasher.update(format!("{:?}", scope).as_bytes());
        hasher.update(format!("{:?}", memory_type).as_bytes());
        hasher.update(title.as_bytes());
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn new(
        project_id: ProjectId,
        scope: Scope,
        memory_type: MemoryType,
        title: String,
        content: String,
    ) -> Self {
        let now = Utc::now();
        let hash = Self::compute_hash(&project_id, &scope, &memory_type, &title, &content);

        Self {
            id: Uuid::new_v4().to_string(),
            project_id,
            session_id: None,
            topic_key: None,
            memory_type,
            scope,
            title,
            content,
            hash,
            tags: Vec::new(),
            metadata: HashMap::new(),
            lifecycle: LifecycleState::Active,
            revision_count: 0,
            duplicate_count: 0,
            last_seen_at: now,
            created_at: now,
            updated_at: now,
            reviewed_at: None,
            review_after: None,
            deleted_at: None,
            deleted_mode: None,
            related_observations: Vec::new(),
            source_prompt: None,
            capture_prompt: true,
        }
    }
}

impl Session {
    pub fn new(project_id: ProjectId) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            project_id,
            started_at: now,
            ended_at: None,
            summary: None,
            context_injected: false,
            observation_ids: Vec::new(),
        }
    }
}

impl Project {
    pub fn new(name: String, path: String) -> Self {
        let now = Utc::now();
        let canonical = Self::canonicalize(&name);
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.clone(),
            path,
            canonical_name: canonical,
            aliases: vec![name],
            created_at: now,
            updated_at: now,
            active: true,
        }
    }

    pub fn canonicalize(name: &str) -> String {
        name.to_lowercase()
            .replace([' ', '-', '_'], "")
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    }
}
