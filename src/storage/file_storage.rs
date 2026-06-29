use crate::memory::types::*;
use crate::storage::{Storage, StorageCapabilities, StructuredStorage};
use async_trait::async_trait;
use dashmap::DashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::warn;
use walkdir::WalkDir;

pub struct FileStorage {
    base_path: PathBuf,
    observations: Arc<DashMap<ObservationId, Observation>>,
    sessions: Arc<DashMap<SessionId, Session>>,
    projects: Arc<DashMap<ProjectId, Project>>,
    prompts: Arc<DashMap<String, SavedPrompt>>,
    relations: Arc<DashMap<String, SemanticRelation>>,
    initialized: Arc<RwLock<bool>>,
}

impl FileStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            observations: Arc::new(DashMap::new()),
            sessions: Arc::new(DashMap::new()),
            projects: Arc::new(DashMap::new()),
            prompts: Arc::new(DashMap::new()),
            relations: Arc::new(DashMap::new()),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn initialize(&self) -> anyhow::Result<()> {
        let mut init = self.initialized.write().await;
        if *init {
            return Ok(());
        }

        fs::create_dir_all(&self.base_path).await?;
        fs::create_dir_all(self.base_path.join("observations")).await?;
        fs::create_dir_all(self.base_path.join("sessions")).await?;
        fs::create_dir_all(self.base_path.join("projects")).await?;
        fs::create_dir_all(self.base_path.join("prompts")).await?;
        fs::create_dir_all(self.base_path.join("relations")).await?;

        self.load_all().await?;
        *init = true;
        Ok(())
    }

    async fn load_all(&self) -> anyhow::Result<()> {
        let obs_dir = self.base_path.join("observations");
        if obs_dir.exists() {
            for entry in WalkDir::new(&obs_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.path().extension().is_some_and(|ext| ext == "json")
                    && let Ok(content) = fs::read_to_string(entry.path()).await
                    && let Ok(obs) = serde_json::from_str::<Observation>(&content)
                {
                    self.observations.insert(obs.id.clone(), obs);
                }
            }
        }

        let sess_dir = self.base_path.join("sessions");
        if sess_dir.exists() {
            for entry in WalkDir::new(&sess_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.path().extension().is_some_and(|ext| ext == "json")
                    && let Ok(content) = fs::read_to_string(entry.path()).await
                    && let Ok(sess) = serde_json::from_str::<Session>(&content)
                {
                    self.sessions.insert(sess.id.clone(), sess);
                }
            }
        }

        let proj_dir = self.base_path.join("projects");
        if proj_dir.exists() {
            for entry in WalkDir::new(&proj_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.path().extension().is_some_and(|ext| ext == "json")
                    && let Ok(content) = fs::read_to_string(entry.path()).await
                    && let Ok(proj) = serde_json::from_str::<Project>(&content)
                {
                    self.projects.insert(proj.id.clone(), proj);
                }
            }
        }

        let prompt_dir = self.base_path.join("prompts");
        if prompt_dir.exists() {
            for entry in WalkDir::new(&prompt_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.path().extension().is_some_and(|ext| ext == "json")
                    && let Ok(content) = fs::read_to_string(entry.path()).await
                    && let Ok(prompt) = serde_json::from_str::<SavedPrompt>(&content)
                {
                    self.prompts.insert(prompt.id.clone(), prompt);
                }
            }
        }

        let rel_dir = self.base_path.join("relations");
        if rel_dir.exists() {
            for entry in WalkDir::new(&rel_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.path().extension().is_some_and(|ext| ext == "json") {
                    match fs::read_to_string(entry.path()).await {
                        Ok(content) => match serde_json::from_str::<SemanticRelation>(&content) {
                            Ok(rel) => {
                                self.relations.insert(rel.id.clone(), rel);
                            }
                            Err(e) => {
                                warn!("Failed to parse relation file {:?}: {}", entry.path(), e)
                            }
                        },
                        Err(e) => warn!("Failed to read relation file {:?}: {}", entry.path(), e),
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate that an ID is safe for use in filesystem paths.
    /// Rejects path traversal attempts (e.g., "../", absolute paths, null bytes).
    fn sanitize_id(id: &str) -> anyhow::Result<&str> {
        if id.is_empty() {
            anyhow::bail!("ID must not be empty");
        }
        if id.contains('/') || id.contains('\\') || id.contains('\0') || id.contains("..") {
            anyhow::bail!("ID contains unsafe path characters: {}", id);
        }
        Ok(id)
    }

    fn obs_path(&self, id: &ObservationId) -> PathBuf {
        self.base_path
            .join("observations")
            .join(format!("{}.json", id))
    }

    fn sess_path(&self, id: &SessionId) -> PathBuf {
        self.base_path.join("sessions").join(format!("{}.json", id))
    }

    fn proj_path(&self, id: &ProjectId) -> PathBuf {
        self.base_path.join("projects").join(format!("{}.json", id))
    }

    fn prompt_path(&self, id: &str) -> PathBuf {
        self.base_path.join("prompts").join(format!("{}.json", id))
    }

    fn rel_path(&self, id: &str) -> PathBuf {
        self.base_path
            .join("relations")
            .join(format!("{}.json", id))
    }

    /// Atomic write: serialize to a temp file, then rename into place.
    /// Prevents partial/corrupt files on crash.
    async fn write_json<T: Serialize>(&self, path: &Path, value: &T) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(value)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &content).await?;
        fs::rename(&tmp_path, path).await?;
        Ok(())
    }
}

#[async_trait]
impl Storage for FileStorage {
    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities::Structured
    }

    fn as_structured(&self) -> Option<&dyn StructuredStorage> {
        Some(self)
    }

    async fn health_check(&self) -> anyhow::Result<StoreHealth> {
        let readable = tokio::fs::read_dir(&self.base_path).await.is_ok();
        let test_file = self.base_path.join(".health_probe");
        let writable = tokio::fs::write(&test_file, b"test").await.is_ok();
        if writable {
            let _ = tokio::fs::remove_file(test_file).await;
        }

        Ok(StoreHealth {
            readable,
            writable,
            corruption_detected: false,
            orphaned_observations: 0,
            orphaned_sessions: 0,
        })
    }
}

#[async_trait]
impl StructuredStorage for FileStorage {
    async fn save_observation(&self, obs: &Observation) -> anyhow::Result<()> {
        Self::sanitize_id(&obs.id)?;
        self.observations.insert(obs.id.clone(), obs.clone());
        self.write_json(&self.obs_path(&obs.id), obs).await
    }

    async fn get_observation(&self, id: &ObservationId) -> anyhow::Result<Option<Observation>> {
        Ok(self.observations.get(id).map(|v| v.clone()))
    }

    async fn update_observation(&self, obs: &Observation) -> anyhow::Result<()> {
        self.observations.insert(obs.id.clone(), obs.clone());
        self.write_json(&self.obs_path(&obs.id), obs).await
    }

    async fn delete_observation(&self, id: &ObservationId, mode: DeleteMode) -> anyhow::Result<()> {
        if mode == DeleteMode::Hard {
            self.observations.remove(id);
            let path = self.obs_path(id);
            if path.exists() {
                fs::remove_file(path).await?;
            }
        } else {
            if let Some(mut obs) = self.observations.get_mut(id) {
                obs.lifecycle = LifecycleState::Deleted;
                obs.deleted_at = Some(Utc::now());
                obs.deleted_mode = Some(mode);
                let obs_clone = obs.clone();
                drop(obs);
                self.write_json(&self.obs_path(id), &obs_clone).await?;
            }
        }
        Ok(())
    }

    async fn list_observations(&self, project_id: &ProjectId) -> anyhow::Result<Vec<Observation>> {
        Ok(self
            .observations
            .iter()
            .filter(|o| o.project_id == *project_id && o.lifecycle != LifecycleState::Deleted)
            .map(|o| o.clone())
            .collect())
    }

    async fn search_observations(
        &self,
        project_id: &ProjectId,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for obs in self.observations.iter() {
            if obs.project_id != *project_id || obs.lifecycle == LifecycleState::Deleted {
                continue;
            }

            let mut score = 0.0;
            let mut matched = Vec::new();

            if obs.title.to_lowercase().contains(&query_lower) {
                score += 10.0;
                matched.push("title".to_string());
            }

            if obs.content.to_lowercase().contains(&query_lower) {
                score += 5.0;
                matched.push("content".to_string());
            }

            for tag in &obs.tags {
                if tag.to_lowercase().contains(&query_lower) {
                    score += 3.0;
                    matched.push("tags".to_string());
                    break;
                }
            }

            if let Some(topic) = &obs.topic_key
                && topic.to_lowercase().contains(&query_lower)
            {
                score += 7.0;
                matched.push("topic_key".to_string());
            }

            if score > 0.0 {
                results.push(SearchResult {
                    observation: obs.clone(),
                    score,
                    matched_fields: matched,
                });
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    async fn get_observations_by_topic(
        &self,
        project_id: &ProjectId,
        topic_key: &TopicKey,
    ) -> anyhow::Result<Vec<Observation>> {
        Ok(self
            .observations
            .iter()
            .filter(|o| {
                o.project_id == *project_id
                    && o.topic_key.as_ref() == Some(topic_key)
                    && o.lifecycle != LifecycleState::Deleted
            })
            .map(|o| o.clone())
            .collect())
    }

    async fn get_observations_by_session(
        &self,
        session_id: &SessionId,
    ) -> anyhow::Result<Vec<Observation>> {
        Ok(self
            .observations
            .iter()
            .filter(|o| {
                o.session_id.as_ref() == Some(session_id) && o.lifecycle != LifecycleState::Deleted
            })
            .map(|o| o.clone())
            .collect())
    }

    async fn get_recent_observations(
        &self,
        project_id: &ProjectId,
        limit: usize,
    ) -> anyhow::Result<Vec<Observation>> {
        let mut obs: Vec<_> = self
            .observations
            .iter()
            .filter(|o| o.project_id == *project_id && o.lifecycle != LifecycleState::Deleted)
            .map(|o| o.clone())
            .collect();

        obs.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        obs.truncate(limit);
        Ok(obs)
    }

    async fn get_stale_reviews(&self, project_id: &ProjectId) -> anyhow::Result<Vec<ReviewItem>> {
        let now = Utc::now();
        let mut items = Vec::new();

        for obs in self.observations.iter() {
            if obs.project_id != *project_id || obs.lifecycle != LifecycleState::Review {
                continue;
            }

            if let Some(review_after) = obs.review_after
                && review_after <= now
            {
                let days_stale = (now - review_after).num_days();
                items.push(ReviewItem {
                    observation: obs.clone(),
                    days_stale,
                    review_after,
                });
            }
        }

        items.sort_by_key(|b| std::cmp::Reverse(b.days_stale));
        Ok(items)
    }

    async fn save_session(&self, session: &Session) -> anyhow::Result<()> {
        Self::sanitize_id(&session.id)?;
        self.sessions.insert(session.id.clone(), session.clone());
        self.write_json(&self.sess_path(&session.id), session).await
    }

    async fn get_session(&self, id: &SessionId) -> anyhow::Result<Option<Session>> {
        Ok(self.sessions.get(id).map(|v| v.clone()))
    }

    async fn update_session(&self, session: &Session) -> anyhow::Result<()> {
        self.sessions.insert(session.id.clone(), session.clone());
        self.write_json(&self.sess_path(&session.id), session).await
    }

    async fn list_sessions(&self, project_id: &ProjectId) -> anyhow::Result<Vec<Session>> {
        Ok(self
            .sessions
            .iter()
            .filter(|s| s.project_id == *project_id)
            .map(|s| s.clone())
            .collect())
    }

    async fn get_recent_sessions(
        &self,
        project_id: &ProjectId,
        limit: usize,
    ) -> anyhow::Result<Vec<Session>> {
        let mut sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|s| s.project_id == *project_id)
            .map(|s| s.clone())
            .collect();

        sessions.sort_by_key(|b| std::cmp::Reverse(b.started_at));
        sessions.truncate(limit);
        Ok(sessions)
    }

    async fn save_project(&self, project: &Project) -> anyhow::Result<()> {
        Self::sanitize_id(&project.id)?;
        self.projects.insert(project.id.clone(), project.clone());
        self.write_json(&self.proj_path(&project.id), project).await
    }

    async fn get_project(&self, id: &ProjectId) -> anyhow::Result<Option<Project>> {
        Ok(self.projects.get(id).map(|v| v.clone()))
    }

    async fn get_project_by_path(&self, path: &str) -> anyhow::Result<Option<Project>> {
        let canonical = Project::canonicalize(path);
        Ok(self
            .projects
            .iter()
            .find(|p| {
                p.path == path
                    || p.canonical_name == canonical
                    || p.aliases.iter().any(|a| a == path)
            })
            .map(|v| v.clone()))
    }

    async fn list_projects(&self) -> anyhow::Result<Vec<Project>> {
        Ok(self.projects.iter().map(|p| p.clone()).collect())
    }

    async fn update_project(&self, project: &Project) -> anyhow::Result<()> {
        self.projects.insert(project.id.clone(), project.clone());
        self.write_json(&self.proj_path(&project.id), project).await
    }

    async fn save_prompt(&self, prompt: &SavedPrompt) -> anyhow::Result<()> {
        Self::sanitize_id(&prompt.id)?;
        self.prompts.insert(prompt.id.clone(), prompt.clone());
        self.write_json(&self.prompt_path(&prompt.id), prompt).await
    }

    async fn get_prompts(
        &self,
        project_id: &ProjectId,
        session_id: Option<&SessionId>,
    ) -> anyhow::Result<Vec<SavedPrompt>> {
        Ok(self
            .prompts
            .iter()
            .filter(|p| {
                p.project_id == *project_id
                    && session_id.is_none_or(|sid| p.session_id.as_ref() == Some(sid))
            })
            .map(|p| p.clone())
            .collect())
    }

    async fn save_relation(&self, relation: &SemanticRelation) -> anyhow::Result<()> {
        Self::sanitize_id(&relation.id)?;
        self.relations.insert(relation.id.clone(), relation.clone());
        self.write_json(&self.rel_path(&relation.id), relation)
            .await
    }

    async fn get_relations(
        &self,
        observation_id: &ObservationId,
    ) -> anyhow::Result<Vec<SemanticRelation>> {
        Ok(self
            .relations
            .iter()
            .filter(|r| r.source_id == *observation_id || r.target_id == *observation_id)
            .map(|r| r.clone())
            .collect())
    }

    async fn get_stats(&self, project_id: &ProjectId) -> anyhow::Result<MemoryStats> {
        let observations: Vec<_> = self
            .observations
            .iter()
            .filter(|o| o.project_id == *project_id)
            .map(|o| o.clone())
            .collect();

        let total = observations.len();
        let active = observations
            .iter()
            .filter(|o| o.lifecycle == LifecycleState::Active)
            .count();
        let archived = observations
            .iter()
            .filter(|o| o.lifecycle == LifecycleState::Archived)
            .count();
        let deleted = observations
            .iter()
            .filter(|o| o.lifecycle == LifecycleState::Deleted)
            .count();

        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|s| s.project_id == *project_id)
            .map(|s| s.clone())
            .collect();

        let total_sessions = sessions.len();
        let active_sessions = sessions.iter().filter(|s| s.ended_at.is_none()).count();

        let oldest = observations
            .iter()
            .min_by_key(|o| o.created_at)
            .map(|o| o.created_at);
        let newest = observations
            .iter()
            .max_by_key(|o| o.created_at)
            .map(|o| o.created_at);

        let mut storage_size = 0u64;
        for entry in WalkDir::new(&self.base_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if let Ok(meta) = entry.metadata() {
                storage_size += meta.len();
            }
        }

        Ok(MemoryStats {
            total_observations: total,
            active_observations: active,
            archived_observations: archived,
            deleted_observations: deleted,
            total_sessions,
            active_sessions,
            total_projects: self.projects.len(),
            storage_size_bytes: storage_size,
            oldest_observation: oldest,
            newest_observation: newest,
        })
    }
}

use chrono::Utc;
use serde::Serialize;

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    async fn setup_db() -> (tempfile::TempDir, FileStorage) {
        let dir = tempdir().unwrap();
        let storage = FileStorage::new(dir.path().to_path_buf());
        (dir, storage)
    }

    fn create_test_project() -> Project {
        Project {
            id: "proj_1".to_string(),
            path: "/path/to/proj".to_string(),
            name: "Test Project".to_string(),
            canonical_name: "test_project".to_string(),
            aliases: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            active: true,
        }
    }

    fn create_test_session() -> Session {
        Session {
            id: "sess_1".to_string(),
            project_id: "proj_1".to_string(),
            started_at: Utc::now(),
            ended_at: None,
            summary: None,
            context_injected: false,
            observation_ids: vec![],
        }
    }

    fn create_test_observation(id: &str) -> Observation {
        Observation {
            id: id.to_string(),
            project_id: "proj_1".to_string(),
            session_id: Some("sess_1".to_string()),
            topic_key: Some("test_topic".to_string()),
            memory_type: MemoryType::Note,
            scope: Scope::Project,
            title: "Test Note".to_string(),
            content: "This is a test observation content with some keywords like banana."
                .to_string(),
            hash: "testhash".to_string(),
            tags: vec!["test".to_string()],
            metadata: std::collections::HashMap::new(),
            lifecycle: LifecycleState::Active,
            revision_count: 0,
            duplicate_count: 0,
            last_seen_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            reviewed_at: None,
            review_after: None,
            deleted_at: None,
            deleted_mode: None,
            related_observations: vec![],
            source_prompt: None,
            capture_prompt: false,
        }
    }

    #[tokio::test]
    async fn test_file_project_crud() {
        let (_dir, storage) = setup_db().await;
        let proj = create_test_project();

        storage.save_project(&proj).await.unwrap();

        let retrieved = storage.get_project(&proj.id).await.unwrap().unwrap();
        assert_eq!(retrieved.name, proj.name);
        assert_eq!(retrieved.path, proj.path);

        let retrieved_by_path = storage
            .get_project_by_path(&proj.path)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved_by_path.id, proj.id);

        let list = storage.list_projects().await.unwrap();
        assert_eq!(list.len(), 1);

        let mut updated_proj = proj.clone();
        updated_proj.name = "Updated Name".to_string();
        storage.update_project(&updated_proj).await.unwrap();

        let retrieved_updated = storage.get_project(&proj.id).await.unwrap().unwrap();
        assert_eq!(retrieved_updated.name, "Updated Name");
    }

    #[tokio::test]
    async fn test_file_session_crud() {
        let (_dir, storage) = setup_db().await;
        let proj = create_test_project();
        storage.save_project(&proj).await.unwrap();

        let sess = create_test_session();
        storage.save_session(&sess).await.unwrap();

        let retrieved = storage.get_session(&sess.id).await.unwrap().unwrap();
        assert_eq!(retrieved.project_id, sess.project_id);

        let mut updated_sess = sess.clone();
        updated_sess.context_injected = true;
        storage.update_session(&updated_sess).await.unwrap();

        let retrieved_updated = storage.get_session(&sess.id).await.unwrap().unwrap();
        assert!(retrieved_updated.context_injected);

        let list = storage.list_sessions(&proj.id).await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_file_observation_crud() {
        let (_dir, storage) = setup_db().await;
        let proj = create_test_project();
        storage.save_project(&proj).await.unwrap();

        let obs = create_test_observation("obs_1");
        storage.save_observation(&obs).await.unwrap();

        let retrieved = storage.get_observation(&obs.id).await.unwrap().unwrap();
        assert_eq!(retrieved.title, obs.title);

        let mut updated_obs = obs.clone();
        updated_obs.title = "Updated Title".to_string();
        storage.update_observation(&updated_obs).await.unwrap();

        let retrieved_updated = storage.get_observation(&obs.id).await.unwrap().unwrap();
        assert_eq!(retrieved_updated.title, "Updated Title");

        let list = storage.list_observations(&proj.id).await.unwrap();
        assert_eq!(list.len(), 1);

        // Soft delete
        storage
            .delete_observation(&obs.id, DeleteMode::Soft)
            .await
            .unwrap();
        let soft_deleted = storage.get_observation(&obs.id).await.unwrap().unwrap();
        assert_eq!(soft_deleted.lifecycle, LifecycleState::Deleted);
        assert!(soft_deleted.deleted_at.is_some());
        assert_eq!(soft_deleted.deleted_mode, Some(DeleteMode::Soft));

        // Hard delete
        storage
            .delete_observation(&obs.id, DeleteMode::Hard)
            .await
            .unwrap();
        let hard_deleted = storage.get_observation(&obs.id).await.unwrap();
        assert!(hard_deleted.is_none());
    }

    #[tokio::test]
    async fn test_file_observation_search() {
        let (_dir, storage) = setup_db().await;
        let proj = create_test_project();
        storage.save_project(&proj).await.unwrap();

        let obs1 = create_test_observation("obs_1");
        storage.save_observation(&obs1).await.unwrap();

        let mut obs2 = create_test_observation("obs_2");
        obs2.title = "Different Note".to_string();
        obs2.content = "Contains the keyword apple instead.".to_string();
        storage.save_observation(&obs2).await.unwrap();

        let search_results = storage
            .search_observations(&proj.id, "banana", 10)
            .await
            .unwrap();
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].observation.id, "obs_1");

        let search_results2 = storage
            .search_observations(&proj.id, "apple", 10)
            .await
            .unwrap();
        assert_eq!(search_results2.len(), 1);
        assert_eq!(search_results2[0].observation.id, "obs_2");
    }
}
