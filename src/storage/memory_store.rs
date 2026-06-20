use crate::auth::{AuthManager, MemwalAuthConfig};
use crate::memory::types::*;
use crate::storage::{FileStorage, MemwalStorage, Storage, get_storage_path};
use std::sync::Arc;

pub struct MemoryStore {
    storage: Arc<dyn Storage>,
    auth_manager: Option<Arc<AuthManager>>,
    current_project: Arc<tokio::sync::RwLock<Option<ProjectId>>>,
    current_session: Arc<tokio::sync::RwLock<Option<SessionId>>>,
}

impl MemoryStore {
    pub async fn new(
        backend: String,
        custom_path: Option<String>,
        auth_config: MemwalAuthConfig,
    ) -> anyhow::Result<Self> {
        let path = custom_path
            .map(|p| std::path::PathBuf::from(p))
            .unwrap_or_else(|| get_storage_path());

        let mut auth_manager = None;
        let storage: Arc<dyn Storage> = match backend.as_str() {
            "file" => {
                let fs = FileStorage::new(path);
                fs.initialize().await?;
                Arc::new(fs)
            }
            "memwal" => {
                let auth = Arc::new(AuthManager::new(auth_config).await?);
                let storage = MemwalStorage::new(path, auth.clone()).await?;
                auth_manager = Some(auth);
                Arc::new(storage)
            }
            "sqlite" => {
                let sq = crate::storage::SqliteStorage::new(path).await?;
                Arc::new(sq)
            }
            other => anyhow::bail!(
                "Unsupported storage backend `{other}`. Expected file, sqlite, or memwal."
            ),
        };

        Ok(Self {
            storage,
            auth_manager,
            current_project: Arc::new(tokio::sync::RwLock::new(None)),
            current_session: Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    pub fn storage(&self) -> Arc<dyn Storage> {
        self.storage.clone()
    }

    pub fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }

    pub async fn set_current_project(&self, project_id: ProjectId) {
        *self.current_project.write().await = Some(project_id);
    }

    pub async fn get_current_project(&self) -> Option<ProjectId> {
        self.current_project.read().await.clone()
    }

    pub async fn set_current_session(&self, session_id: SessionId) {
        *self.current_session.write().await = Some(session_id);
    }

    pub async fn get_current_session(&self) -> Option<SessionId> {
        self.current_session.read().await.clone()
    }

    pub async fn clear_current_session(&self) {
        *self.current_session.write().await = None;
    }

    pub async fn detect_project(&self, cwd: Option<String>) -> anyhow::Result<Option<ProjectId>> {
        let cwd = cwd.unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
        let path = std::path::Path::new(&cwd);

        let mut current = Some(path);
        while let Some(dir) = current {
            if let Ok(Some(project)) = self
                .storage
                .get_project_by_path(&dir.to_string_lossy())
                .await
            {
                return Ok(Some(project.id));
            }
            current = dir.parent();
        }

        let dir_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let canonical = Project::canonicalize(&dir_name);

        for proj in self.storage.list_projects().await? {
            if proj.canonical_name == canonical || proj.aliases.iter().any(|a| a == &dir_name) {
                return Ok(Some(proj.id));
            }
        }

        Ok(None)
    }

    pub async fn get_or_create_project(&self, cwd: Option<String>) -> anyhow::Result<Project> {
        let cwd = cwd.unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

        if let Some(project_id) = self.detect_project(Some(cwd.clone())).await? {
            if let Some(project) = self.storage.get_project(&project_id).await? {
                return Ok(project);
            }
        }

        let project = Project::new(
            std::path::Path::new(&cwd)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            cwd.clone(),
        );

        self.storage.save_project(&project).await?;
        self.set_current_project(project.id.clone()).await;

        Ok(project)
    }
}

impl Clone for MemoryStore {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            auth_manager: self.auth_manager.clone(),
            current_project: self.current_project.clone(),
            current_session: self.current_session.clone(),
        }
    }
}
