use crate::memory::types::*;
use async_trait::async_trait;
use std::path::PathBuf;

pub mod file_storage;
pub mod memory_store;
pub mod sqlite_storage;

pub use file_storage::FileStorage;
pub use memory_store::MemoryStore;
pub use sqlite_storage::SqliteStorage;

#[async_trait]
pub trait Storage: Send + Sync {
    async fn save_observation(&self, obs: &Observation) -> anyhow::Result<()>;
    async fn get_observation(&self, id: &ObservationId) -> anyhow::Result<Option<Observation>>;
    async fn update_observation(&self, obs: &Observation) -> anyhow::Result<()>;
    async fn delete_observation(&self, id: &ObservationId, mode: DeleteMode) -> anyhow::Result<()>;
    async fn list_observations(&self, project_id: &ProjectId) -> anyhow::Result<Vec<Observation>>;
    async fn search_observations(
        &self,
        project_id: &ProjectId,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>>;
    async fn get_observations_by_topic(
        &self,
        project_id: &ProjectId,
        topic_key: &TopicKey,
    ) -> anyhow::Result<Vec<Observation>>;
    async fn get_observations_by_session(
        &self,
        session_id: &SessionId,
    ) -> anyhow::Result<Vec<Observation>>;
    async fn get_recent_observations(
        &self,
        project_id: &ProjectId,
        limit: usize,
    ) -> anyhow::Result<Vec<Observation>>;
    async fn get_stale_reviews(&self, project_id: &ProjectId) -> anyhow::Result<Vec<ReviewItem>>;

    async fn save_session(&self, session: &Session) -> anyhow::Result<()>;
    async fn get_session(&self, id: &SessionId) -> anyhow::Result<Option<Session>>;
    async fn update_session(&self, session: &Session) -> anyhow::Result<()>;
    async fn list_sessions(&self, project_id: &ProjectId) -> anyhow::Result<Vec<Session>>;
    async fn get_recent_sessions(
        &self,
        project_id: &ProjectId,
        limit: usize,
    ) -> anyhow::Result<Vec<Session>>;

    async fn save_project(&self, project: &Project) -> anyhow::Result<()>;
    async fn get_project(&self, id: &ProjectId) -> anyhow::Result<Option<Project>>;
    async fn get_project_by_path(&self, path: &str) -> anyhow::Result<Option<Project>>;
    async fn list_projects(&self) -> anyhow::Result<Vec<Project>>;
    async fn update_project(&self, project: &Project) -> anyhow::Result<()>;

    async fn save_prompt(&self, prompt: &SavedPrompt) -> anyhow::Result<()>;
    async fn get_prompts(
        &self,
        project_id: &ProjectId,
        session_id: Option<&SessionId>,
    ) -> anyhow::Result<Vec<SavedPrompt>>;

    async fn save_relation(&self, relation: &SemanticRelation) -> anyhow::Result<()>;
    async fn get_relations(
        &self,
        observation_id: &ObservationId,
    ) -> anyhow::Result<Vec<SemanticRelation>>;

    async fn get_stats(&self, project_id: &ProjectId) -> anyhow::Result<MemoryStats>;

    async fn health_check(&self) -> anyhow::Result<StoreHealth>;
}

pub fn get_storage_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("eidetic-mcp")
        .join("storage")
}
