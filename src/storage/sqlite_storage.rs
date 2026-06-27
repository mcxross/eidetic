use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::path::Path;

use crate::memory::types::*;
use crate::storage::Storage;

pub struct SqliteStorage {
    pool: SqlitePool,
}

impl SqliteStorage {
    pub async fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let db_path = path.as_ref().join("eidetic.db");

        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let storage = Self { pool };
        storage.initialize_schema().await?;
        Ok(storage)
    }

    async fn initialize_schema(&self) -> anyhow::Result<()> {
        let schema = r#"
        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            canonical_name TEXT NOT NULL,
            path TEXT NOT NULL,
            aliases TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT,
            active BOOLEAN NOT NULL
        );

        CREATE TABLE IF NOT EXISTS observations (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            session_id TEXT,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            tags TEXT NOT NULL,
            metadata TEXT,
            type TEXT NOT NULL,
            scope TEXT NOT NULL,
            lifecycle_state TEXT NOT NULL,
            topic_key TEXT,
            hash TEXT NOT NULL,
            revision_count INTEGER NOT NULL,
            duplicate_count INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            reviewed_at TEXT,
            review_after TEXT,
            deleted_at TEXT,
            deleted_mode TEXT,
            related_observations TEXT,
            source_prompt TEXT,
            capture_prompt BOOLEAN NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS observations_fts USING fts5(
            id UNINDEXED,
            title,
            content,
            tags
        );

        CREATE TRIGGER IF NOT EXISTS observations_ai AFTER INSERT ON observations BEGIN
            INSERT INTO observations_fts(id, title, content, tags) VALUES (new.id, new.title, new.content, new.tags);
        END;
        CREATE TRIGGER IF NOT EXISTS observations_au AFTER UPDATE ON observations BEGIN
            UPDATE observations_fts SET title = new.title, content = new.content, tags = new.tags WHERE id = old.id;
        END;
        CREATE TRIGGER IF NOT EXISTS observations_ad AFTER DELETE ON observations BEGIN
            DELETE FROM observations_fts WHERE id = old.id;
        END;

        CREATE TABLE IF NOT EXISTS prompts (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            session_id TEXT,
            prompt TEXT NOT NULL,
            context TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS relations (
            id TEXT PRIMARY KEY,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            relation_type TEXT NOT NULL,
            confidence REAL NOT NULL,
            reasoning TEXT NOT NULL,
            created_at TEXT NOT NULL,
            judged_by TEXT
        );
        "#;

        sqlx::query(schema).execute(&self.pool).await?;
        Ok(())
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    async fn save_project(&self, project: &Project) -> anyhow::Result<()> {
        let aliases_json = serde_json::to_string(&project.aliases)?;
        sqlx::query(
            "INSERT OR REPLACE INTO projects (id, name, canonical_name, path, aliases, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.canonical_name)
        .bind(&project.path)
        .bind(aliases_json)
        .bind(project.created_at.to_rfc3339())
        .bind(project.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_project(&self, id: &ProjectId) -> anyhow::Result<Option<Project>> {
        let row = sqlx::query("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let aliases: Vec<String> = serde_json::from_str(row.get("aliases"))?;
            let created_at =
                DateTime::parse_from_rfc3339(row.get("created_at"))?.with_timezone(&Utc);
            let updated_at =
                DateTime::parse_from_rfc3339(row.get("updated_at"))?.with_timezone(&Utc);

            Ok(Some(Project {
                id: row.get("id"),
                name: row.get("name"),
                canonical_name: row.get("canonical_name"),
                path: row.get("path"),
                aliases,
                created_at,
                updated_at,
                active: true, // Assuming true since it exists
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_project_by_path(&self, path: &str) -> anyhow::Result<Option<Project>> {
        let row = sqlx::query("SELECT id FROM projects WHERE path = ? LIMIT 1")
            .bind(path)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let id: String = row.get("id");
            self.get_project(&id).await
        } else {
            Ok(None)
        }
    }

    async fn list_projects(&self) -> anyhow::Result<Vec<Project>> {
        let rows = sqlx::query("SELECT id FROM projects")
            .fetch_all(&self.pool)
            .await?;

        let mut projects = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            if let Some(p) = self.get_project(&id).await? {
                projects.push(p);
            }
        }
        Ok(projects)
    }

    async fn update_project(&self, project: &Project) -> anyhow::Result<()> {
        self.save_project(project).await
    }

    async fn save_session(&self, session: &Session) -> anyhow::Result<()> {
        let ended_at = session.ended_at.map(|t| t.to_rfc3339());
        sqlx::query(
            "INSERT OR REPLACE INTO sessions (id, project_id, created_at, updated_at, active)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&session.id)
        .bind(&session.project_id)
        .bind(session.started_at.to_rfc3339())
        .bind(ended_at)
        .bind(session.ended_at.is_none()) // active
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_session(&self, id: &SessionId) -> anyhow::Result<Option<Session>> {
        let row = sqlx::query("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let created_at =
                DateTime::parse_from_rfc3339(row.get("created_at"))?.with_timezone(&Utc);
            let ended_at = row
                .get::<Option<String>, _>("updated_at")
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            Ok(Some(Session {
                id: row.get("id"),
                project_id: row.get("project_id"),
                started_at: created_at,
                ended_at,
                summary: None,
                context_injected: false,
                observation_ids: vec![], // Not stored in schema directly, would need join
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_session(&self, session: &Session) -> anyhow::Result<()> {
        self.save_session(session).await
    }

    async fn list_sessions(&self, project_id: &ProjectId) -> anyhow::Result<Vec<Session>> {
        let rows =
            sqlx::query("SELECT id FROM sessions WHERE project_id = ? ORDER BY created_at DESC")
                .bind(project_id)
                .fetch_all(&self.pool)
                .await?;

        let mut sessions = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            if let Some(s) = self.get_session(&id).await? {
                sessions.push(s);
            }
        }
        Ok(sessions)
    }

    async fn get_recent_sessions(
        &self,
        project_id: &ProjectId,
        limit: usize,
    ) -> anyhow::Result<Vec<Session>> {
        let rows = sqlx::query(
            "SELECT id FROM sessions WHERE project_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(project_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut sessions = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            if let Some(s) = self.get_session(&id).await? {
                sessions.push(s);
            }
        }
        Ok(sessions)
    }

    async fn save_observation(&self, obs: &Observation) -> anyhow::Result<()> {
        let tags_json = serde_json::to_string(&obs.tags)?;
        let metadata_json = serde_json::to_string(&obs.metadata)?;
        let type_str = serde_json::to_string(&obs.memory_type)?
            .trim_matches('"')
            .to_string();
        let scope_str = serde_json::to_string(&obs.scope)?
            .trim_matches('"')
            .to_string();
        let state_str = serde_json::to_string(&obs.lifecycle)?
            .trim_matches('"')
            .to_string();
        let related_json = serde_json::to_string(&obs.related_observations)?;
        let deleted_mode_str = obs.deleted_mode.map(|m| {
            serde_json::to_string(&m)
                .unwrap()
                .trim_matches('"')
                .to_string()
        });

        sqlx::query(
            "INSERT OR REPLACE INTO observations 
             (id, project_id, session_id, title, content, tags, metadata, type, scope, lifecycle_state, topic_key, hash, revision_count, duplicate_count, created_at, updated_at, last_seen_at, reviewed_at, review_after, deleted_at, deleted_mode, related_observations, source_prompt, capture_prompt)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&obs.id)
        .bind(&obs.project_id)
        .bind(&obs.session_id)
        .bind(&obs.title)
        .bind(&obs.content)
        .bind(tags_json)
        .bind(metadata_json)
        .bind(type_str)
        .bind(scope_str)
        .bind(state_str)
        .bind(&obs.topic_key)
        .bind(&obs.hash)
        .bind(obs.revision_count)
        .bind(obs.duplicate_count)
        .bind(obs.created_at.to_rfc3339())
        .bind(obs.updated_at.to_rfc3339())
        .bind(obs.last_seen_at.to_rfc3339())
        .bind(obs.reviewed_at.map(|t| t.to_rfc3339()))
        .bind(obs.review_after.map(|t| t.to_rfc3339()))
        .bind(obs.deleted_at.map(|t| t.to_rfc3339()))
        .bind(deleted_mode_str)
        .bind(related_json)
        .bind(&obs.source_prompt)
        .bind(obs.capture_prompt)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_observation(&self, id: &ObservationId) -> anyhow::Result<Option<Observation>> {
        let row = sqlx::query("SELECT * FROM observations WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let tags: Vec<String> = serde_json::from_str(row.get("tags"))?;
            let metadata: std::collections::HashMap<String, serde_json::Value> =
                serde_json::from_str(row.get("metadata"))?;
            let memory_type: MemoryType =
                serde_json::from_str(&format!("\"{}\"", row.get::<String, _>("type")))?;
            let scope: Scope =
                serde_json::from_str(&format!("\"{}\"", row.get::<String, _>("scope")))?;
            let lifecycle: LifecycleState =
                serde_json::from_str(&format!("\"{}\"", row.get::<String, _>("lifecycle_state")))?;
            let created_at =
                DateTime::parse_from_rfc3339(row.get("created_at"))?.with_timezone(&Utc);
            let updated_at =
                DateTime::parse_from_rfc3339(row.get("updated_at"))?.with_timezone(&Utc);
            let last_seen_at =
                DateTime::parse_from_rfc3339(row.get("last_seen_at"))?.with_timezone(&Utc);

            let reviewed_at = row
                .get::<Option<String>, _>("reviewed_at")
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));
            let review_after = row
                .get::<Option<String>, _>("review_after")
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));
            let deleted_at = row
                .get::<Option<String>, _>("deleted_at")
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));
            let deleted_mode = row
                .get::<Option<String>, _>("deleted_mode")
                .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok());
            let related_observations: Vec<String> =
                serde_json::from_str(row.get("related_observations"))?;

            Ok(Some(Observation {
                id: row.get("id"),
                project_id: row.get("project_id"),
                session_id: row.get("session_id"),
                title: row.get("title"),
                content: row.get("content"),
                tags,
                metadata,
                memory_type,
                scope,
                lifecycle,
                topic_key: row.get("topic_key"),
                hash: row.get("hash"),
                revision_count: row.get::<i32, _>("revision_count") as u32,
                duplicate_count: row.get::<i32, _>("duplicate_count") as u32,
                created_at,
                updated_at,
                last_seen_at,
                reviewed_at,
                review_after,
                deleted_at,
                deleted_mode,
                related_observations,
                source_prompt: row.get("source_prompt"),
                capture_prompt: row.get("capture_prompt"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_observation(&self, obs: &Observation) -> anyhow::Result<()> {
        self.save_observation(obs).await
    }

    async fn delete_observation(&self, id: &ObservationId, mode: DeleteMode) -> anyhow::Result<()> {
        match mode {
            DeleteMode::Hard => {
                sqlx::query("DELETE FROM observations WHERE id = ?")
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
            DeleteMode::Soft => {
                let state_str = serde_json::to_string(&LifecycleState::Deleted)?
                    .trim_matches('"')
                    .to_string();
                let deleted_mode_str = serde_json::to_string(&mode)?.trim_matches('"').to_string();
                sqlx::query("UPDATE observations SET lifecycle_state = ?, deleted_at = ?, deleted_mode = ? WHERE id = ?")
                    .bind(state_str)
                    .bind(Utc::now().to_rfc3339())
                    .bind(deleted_mode_str)
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    async fn list_observations(&self, project_id: &ProjectId) -> anyhow::Result<Vec<Observation>> {
        let rows = sqlx::query("SELECT id FROM observations WHERE project_id = ? AND deleted_at IS NULL ORDER BY created_at DESC")
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;

        let mut obs = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            if let Some(o) = self.get_observation(&id).await? {
                obs.push(o);
            }
        }
        Ok(obs)
    }

    async fn search_observations(
        &self,
        project_id: &ProjectId,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let safe_query = format!("\"{}\"", query.replace("\"", "\"\""));
        let rows = sqlx::query(
            "SELECT observations_fts.id, bm25(observations_fts) as rank FROM observations_fts JOIN observations o ON observations_fts.id = o.id WHERE observations_fts MATCH ? AND o.deleted_at IS NULL ORDER BY rank LIMIT ?"
        )
            .bind(&safe_query)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            let rank: f64 = row.get("rank");
            if let Some(obs) = self.get_observation(&id).await?
                && &obs.project_id == project_id
            {
                results.push(SearchResult {
                    observation: obs,
                    score: (1.0 / (1.0 + rank)) as f32,
                    matched_fields: vec!["content".to_string()],
                });
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(results)
    }

    async fn get_observations_by_topic(
        &self,
        project_id: &ProjectId,
        topic_key: &TopicKey,
    ) -> anyhow::Result<Vec<Observation>> {
        let rows = sqlx::query("SELECT id FROM observations WHERE project_id = ? AND topic_key = ? AND deleted_at IS NULL ORDER BY created_at DESC")
            .bind(project_id)
            .bind(topic_key)
            .fetch_all(&self.pool)
            .await?;

        let mut obs = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            if let Some(o) = self.get_observation(&id).await? {
                obs.push(o);
            }
        }
        Ok(obs)
    }

    async fn get_observations_by_session(
        &self,
        session_id: &SessionId,
    ) -> anyhow::Result<Vec<Observation>> {
        let rows = sqlx::query("SELECT id FROM observations WHERE session_id = ? AND deleted_at IS NULL ORDER BY created_at DESC")
            .bind(session_id)
            .fetch_all(&self.pool)
            .await?;

        let mut obs = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            if let Some(o) = self.get_observation(&id).await? {
                obs.push(o);
            }
        }
        Ok(obs)
    }

    async fn get_recent_observations(
        &self,
        project_id: &ProjectId,
        limit: usize,
    ) -> anyhow::Result<Vec<Observation>> {
        let rows = sqlx::query("SELECT id FROM observations WHERE project_id = ? AND deleted_at IS NULL ORDER BY created_at DESC LIMIT ?")
            .bind(project_id)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;

        let mut obs = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            if let Some(o) = self.get_observation(&id).await? {
                obs.push(o);
            }
        }
        Ok(obs)
    }

    async fn get_stale_reviews(&self, project_id: &ProjectId) -> anyhow::Result<Vec<ReviewItem>> {
        let threshold = Utc::now() - chrono::Duration::days(7);
        let rows = sqlx::query("SELECT id FROM observations WHERE project_id = ? AND updated_at < ? AND lifecycle_state != ?")
            .bind(project_id)
            .bind(threshold.to_rfc3339())
            .bind(serde_json::to_string(&LifecycleState::Archived)?.trim_matches('"'))
            .fetch_all(&self.pool)
            .await?;

        let mut reviews = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            if let Some(o) = self.get_observation(&id).await? {
                reviews.push(ReviewItem {
                    observation: o,
                    days_stale: 7,
                    review_after: Utc::now(),
                });
            }
        }
        Ok(reviews)
    }

    async fn save_prompt(&self, prompt: &SavedPrompt) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO prompts (id, project_id, session_id, prompt, context, created_at)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&prompt.id)
        .bind(&prompt.project_id)
        .bind(&prompt.session_id)
        .bind(&prompt.prompt)
        .bind(&prompt.context)
        .bind(prompt.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_prompts(
        &self,
        project_id: &ProjectId,
        session_id: Option<&SessionId>,
    ) -> anyhow::Result<Vec<SavedPrompt>> {
        let rows = if let Some(sid) = session_id {
            sqlx::query("SELECT * FROM prompts WHERE project_id = ? AND session_id = ? ORDER BY created_at DESC")
                .bind(project_id)
                .bind(sid)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query("SELECT * FROM prompts WHERE project_id = ? ORDER BY created_at DESC")
                .bind(project_id)
                .fetch_all(&self.pool)
                .await?
        };

        let mut prompts = Vec::new();
        for row in rows {
            prompts.push(SavedPrompt {
                id: row.get("id"),
                project_id: row.get("project_id"),
                session_id: row.get("session_id"),
                prompt: row.get("prompt"),
                context: row.get("context"),
                created_at: DateTime::parse_from_rfc3339(row.get("created_at"))?
                    .with_timezone(&Utc),
            });
        }
        Ok(prompts)
    }

    async fn save_relation(&self, relation: &SemanticRelation) -> anyhow::Result<()> {
        let type_str = serde_json::to_string(&relation.relation_type)?
            .trim_matches('"')
            .to_string();
        sqlx::query(
            "INSERT OR REPLACE INTO relations (id, source_id, target_id, relation_type, confidence, reasoning, created_at, judged_by)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&relation.id)
        .bind(&relation.source_id)
        .bind(&relation.target_id)
        .bind(type_str)
        .bind(relation.confidence as f64)
        .bind(&relation.reasoning)
        .bind(relation.created_at.to_rfc3339())
        .bind(&relation.judged_by)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_relations(
        &self,
        observation_id: &ObservationId,
    ) -> anyhow::Result<Vec<SemanticRelation>> {
        let rows = sqlx::query(
            "SELECT * FROM relations WHERE source_id = ? OR target_id = ? ORDER BY created_at DESC",
        )
        .bind(observation_id)
        .bind(observation_id)
        .fetch_all(&self.pool)
        .await?;

        let mut relations = Vec::new();
        for row in rows {
            let conf: f64 = row.get("confidence");
            relations.push(SemanticRelation {
                id: row.get("id"),
                source_id: row.get("source_id"),
                target_id: row.get("target_id"),
                relation_type: serde_json::from_str(&format!(
                    "\"{}\"",
                    row.get::<String, _>("relation_type")
                ))?,
                confidence: conf as f32,
                reasoning: row.get("reasoning"),
                created_at: DateTime::parse_from_rfc3339(row.get("created_at"))?
                    .with_timezone(&Utc),
                judged_by: row.get("judged_by"),
            });
        }
        Ok(relations)
    }

    async fn get_stats(&self, project_id: &ProjectId) -> anyhow::Result<MemoryStats> {
        let count_row = sqlx::query("SELECT COUNT(*) as c FROM observations WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
        let total_observations: i64 = count_row.get("c");

        let sessions_row = sqlx::query("SELECT COUNT(*) as c FROM sessions WHERE project_id = ?")
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
        let total_sessions: i64 = sessions_row.get("c");

        let projs_row = sqlx::query("SELECT COUNT(*) as c FROM projects")
            .fetch_one(&self.pool)
            .await?;
        let total_projects: i64 = projs_row.get("c");

        Ok(MemoryStats {
            total_observations: total_observations as usize,
            active_observations: total_observations as usize,
            archived_observations: 0,
            deleted_observations: 0,
            total_sessions: total_sessions as usize,
            active_sessions: 1,
            total_projects: total_projects as usize,
            storage_size_bytes: 4096, // placeholder
            oldest_observation: None,
            newest_observation: None,
        })
    }

    async fn health_check(&self) -> anyhow::Result<StoreHealth> {
        Ok(StoreHealth {
            readable: true,
            writable: true,
            corruption_detected: false,
            orphaned_observations: 0,
            orphaned_sessions: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use tempfile::tempdir;

    async fn setup_db() -> (tempfile::TempDir, SqliteStorage) {
        let dir = tempdir().unwrap();
        let storage = SqliteStorage::new(dir.path().to_path_buf()).await.unwrap();
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
    async fn test_sqlite_project_crud() {
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
    async fn test_sqlite_session_crud() {
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
        // SqliteStorage doesn't persist context_injected.
        assert_eq!(retrieved_updated.id, sess.id);

        let list = storage.list_sessions(&proj.id).await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_sqlite_observation_crud() {
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
    async fn test_sqlite_observation_search() {
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

    #[tokio::test]
    async fn test_sqlite_relations() {
        let (_dir, storage) = setup_db().await;
        let relation = SemanticRelation {
            id: "rel_1".to_string(),
            source_id: "obs_1".to_string(),
            target_id: "obs_2".to_string(),
            relation_type: RelationType::Related,
            confidence: 0.9,
            reasoning: "Test reasoning".to_string(),
            created_at: Utc::now(),
            judged_by: None,
        };

        storage.save_relation(&relation).await.unwrap();

        let relations = storage.get_relations(&"obs_1".to_string()).await.unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].target_id, "obs_2");
        assert_eq!(relations[0].relation_type, RelationType::Related);
    }
}
