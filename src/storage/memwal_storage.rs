use std::{collections::HashSet, path::Path, sync::Arc, time::Duration};

use async_trait::async_trait;
use memwal_core::RecallParams;

use crate::{
    auth::AuthManager,
    memory::types::*,
    storage::{SqliteStorage, Storage},
};

const PAYLOAD_MARKER: &str = "EIDETIC_MEMWAL_OBSERVATION_V1";

pub struct MemwalStorage {
    auth: Arc<AuthManager>,
    index: SqliteStorage,
}

impl MemwalStorage {
    pub async fn new<P: AsRef<Path>>(path: P, auth: Arc<AuthManager>) -> anyhow::Result<Self> {
        Ok(Self {
            auth,
            index: SqliteStorage::new(path).await?,
        })
    }

    async fn remember_observation(&self, obs: &Observation) -> anyhow::Result<()> {
        let client = self.auth.memwal_client().await?;
        let payload = observation_payload(obs)?;
        client
            .remember(
                &payload,
                Duration::from_millis(1500),
                Duration::from_secs(60),
            )
            .await
            .map_err(|error| anyhow::anyhow!("Memwal remember failed: {error}"))?;
        Ok(())
    }
}

fn observation_payload(obs: &Observation) -> anyhow::Result<String> {
    Ok(format!(
        "{PAYLOAD_MARKER}\n\
         id: {}\n\
         project_id: {}\n\
         title: {}\n\
         type: {:?}\n\
         scope: {:?}\n\
         topic_key: {}\n\
         tags: {}\n\
         content:\n{}\n\
         json:\n{}",
        obs.id,
        obs.project_id,
        obs.title,
        obs.memory_type,
        obs.scope,
        obs.topic_key.clone().unwrap_or_default(),
        obs.tags.join(", "),
        obs.content,
        serde_json::to_string(obs)?
    ))
}

fn parse_observation_payload(text: &str) -> Option<Observation> {
    if !text.starts_with(PAYLOAD_MARKER) {
        return None;
    }
    let json = text.split_once("\njson:\n")?.1.trim();
    serde_json::from_str(json).ok()
}

#[async_trait]
impl Storage for MemwalStorage {
    async fn save_observation(&self, obs: &Observation) -> anyhow::Result<()> {
        self.remember_observation(obs).await?;
        self.index.save_observation(obs).await
    }

    async fn get_observation(&self, id: &ObservationId) -> anyhow::Result<Option<Observation>> {
        self.index.get_observation(id).await
    }

    async fn update_observation(&self, obs: &Observation) -> anyhow::Result<()> {
        self.remember_observation(obs).await?;
        self.index.update_observation(obs).await
    }

    async fn delete_observation(&self, id: &ObservationId, mode: DeleteMode) -> anyhow::Result<()> {
        self.index.delete_observation(id, mode).await
    }

    async fn list_observations(&self, project_id: &ProjectId) -> anyhow::Result<Vec<Observation>> {
        self.index.list_observations(project_id).await
    }

    async fn search_observations(
        &self,
        project_id: &ProjectId,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let mut results = Vec::new();
        let mut seen = HashSet::new();

        if let Ok(client) = self.auth.memwal_client().await {
            if let Ok(recall) = client
                .recall(RecallParams {
                    query: query.to_string(),
                    limit: Some(limit),
                    namespace: None,
                    top_k: Some(limit),
                    max_distance: None,
                })
                .await
            {
                for memory in recall.results {
                    if let Some(obs) = parse_observation_payload(&memory.text) {
                        if &obs.project_id == project_id
                            && obs.lifecycle != LifecycleState::Deleted
                            && seen.insert(obs.id.clone())
                        {
                            results.push(SearchResult {
                                observation: obs,
                                score: (1.0 / (1.0 + memory.distance)) as f32,
                                matched_fields: vec!["memwal".to_string()],
                            });
                        }
                    }
                }
            }
        }

        let fallback = self
            .index
            .search_observations(project_id, query, limit)
            .await?;
        for result in fallback {
            if seen.insert(result.observation.id.clone()) {
                results.push(result);
            }
            if results.len() >= limit {
                break;
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
        self.index
            .get_observations_by_topic(project_id, topic_key)
            .await
    }

    async fn get_observations_by_session(
        &self,
        session_id: &SessionId,
    ) -> anyhow::Result<Vec<Observation>> {
        self.index.get_observations_by_session(session_id).await
    }

    async fn get_recent_observations(
        &self,
        project_id: &ProjectId,
        limit: usize,
    ) -> anyhow::Result<Vec<Observation>> {
        self.index.get_recent_observations(project_id, limit).await
    }

    async fn get_stale_reviews(&self, project_id: &ProjectId) -> anyhow::Result<Vec<ReviewItem>> {
        self.index.get_stale_reviews(project_id).await
    }

    async fn save_session(&self, session: &Session) -> anyhow::Result<()> {
        self.index.save_session(session).await
    }

    async fn get_session(&self, id: &SessionId) -> anyhow::Result<Option<Session>> {
        self.index.get_session(id).await
    }

    async fn update_session(&self, session: &Session) -> anyhow::Result<()> {
        self.index.update_session(session).await
    }

    async fn list_sessions(&self, project_id: &ProjectId) -> anyhow::Result<Vec<Session>> {
        self.index.list_sessions(project_id).await
    }

    async fn get_recent_sessions(
        &self,
        project_id: &ProjectId,
        limit: usize,
    ) -> anyhow::Result<Vec<Session>> {
        self.index.get_recent_sessions(project_id, limit).await
    }

    async fn save_project(&self, project: &Project) -> anyhow::Result<()> {
        self.index.save_project(project).await
    }

    async fn get_project(&self, id: &ProjectId) -> anyhow::Result<Option<Project>> {
        self.index.get_project(id).await
    }

    async fn get_project_by_path(&self, path: &str) -> anyhow::Result<Option<Project>> {
        self.index.get_project_by_path(path).await
    }

    async fn list_projects(&self) -> anyhow::Result<Vec<Project>> {
        self.index.list_projects().await
    }

    async fn update_project(&self, project: &Project) -> anyhow::Result<()> {
        self.index.update_project(project).await
    }

    async fn save_prompt(&self, prompt: &SavedPrompt) -> anyhow::Result<()> {
        self.index.save_prompt(prompt).await
    }

    async fn get_prompts(
        &self,
        project_id: &ProjectId,
        session_id: Option<&SessionId>,
    ) -> anyhow::Result<Vec<SavedPrompt>> {
        self.index.get_prompts(project_id, session_id).await
    }

    async fn save_relation(&self, relation: &SemanticRelation) -> anyhow::Result<()> {
        self.index.save_relation(relation).await
    }

    async fn get_relations(
        &self,
        observation_id: &ObservationId,
    ) -> anyhow::Result<Vec<SemanticRelation>> {
        self.index.get_relations(observation_id).await
    }

    async fn get_stats(&self, project_id: &ProjectId) -> anyhow::Result<MemoryStats> {
        self.index.get_stats(project_id).await
    }

    async fn health_check(&self) -> anyhow::Result<StoreHealth> {
        self.index.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_payload_round_trips() {
        let obs = Observation::new(
            "project".to_string(),
            Scope::Project,
            MemoryType::Decision,
            "Title".to_string(),
            "Content".to_string(),
        );
        let payload = observation_payload(&obs).unwrap();
        let parsed = parse_observation_payload(&payload).unwrap();
        assert_eq!(parsed.id, obs.id);
        assert_eq!(parsed.title, obs.title);
        assert_eq!(parsed.content, obs.content);
    }
}
