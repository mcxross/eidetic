use crate::memory::types::*;
use crate::storage::Storage;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Projects,
    Observations,
    Sessions,
    Search,
}

impl Tab {
    pub fn next(self) -> Self {
        match self {
            Tab::Projects => Tab::Observations,
            Tab::Observations => Tab::Sessions,
            Tab::Sessions => Tab::Search,
            Tab::Search => Tab::Projects,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Projects => "Projects",
            Tab::Observations => "Observations",
            Tab::Sessions => "Sessions",
            Tab::Search => "Search",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailView {
    None,
    ObservationDetail(ObservationId),
    SessionDetail(SessionId),
}

pub struct App {
    pub storage: Arc<dyn Storage>,
    pub running: bool,
    pub input_mode: bool,

    pub tab: Tab,
    pub detail: DetailView,

    pub projects: Vec<Project>,
    pub observations: Vec<Observation>,
    pub sessions: Vec<Session>,
    pub search_results: Vec<SearchResult>,

    pub active_project: Option<Project>,

    pub project_cursor: usize,
    pub observation_cursor: usize,
    pub session_cursor: usize,
    pub search_cursor: usize,

    pub page_size: usize,
    pub observation_page: usize,
    pub session_page: usize,

    pub input_buffer: String,

    pub detail_observation: Option<Observation>,
    pub detail_session: Option<Session>,
    pub detail_session_observations: Vec<Observation>,

    pub status: String,

    pub confirm_action: Option<ConfirmAction>,
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    HardDelete(ObservationId),
}

impl App {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self {
            storage,
            running: true,
            input_mode: false,
            tab: Tab::Projects,
            detail: DetailView::None,
            projects: Vec::new(),
            observations: Vec::new(),
            sessions: Vec::new(),
            search_results: Vec::new(),
            active_project: None,
            project_cursor: 0,
            observation_cursor: 0,
            session_cursor: 0,
            search_cursor: 0,
            page_size: 50,
            observation_page: 0,
            session_page: 0,
            input_buffer: String::new(),
            detail_observation: None,
            detail_session: None,
            detail_session_observations: Vec::new(),
            status: String::new(),
            confirm_action: None,
        }
    }

    pub async fn load_projects(&mut self) -> anyhow::Result<()> {
        self.projects = self.storage.list_projects().await?;
        self.projects.sort_by(|a, b| a.name.cmp(&b.name));
        self.project_cursor = 0;
        Ok(())
    }

    pub async fn load_observations(&mut self) -> anyhow::Result<()> {
        if let Some(proj) = &self.active_project {
            let mut all = self.storage.list_observations(&proj.id).await?;
            all.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
            let start = self.observation_page * self.page_size;
            if start < all.len() {
                self.observations =
                    all[start..std::cmp::min(start + self.page_size, all.len())].to_vec();
            } else {
                self.observations.clear();
            }
        } else {
            self.observations.clear();
        }
        self.observation_cursor = 0;
        Ok(())
    }

    pub async fn load_sessions(&mut self) -> anyhow::Result<()> {
        if let Some(proj) = &self.active_project {
            let mut all = self.storage.list_sessions(&proj.id).await?;
            all.sort_by_key(|b| std::cmp::Reverse(b.started_at));
            let start = self.session_page * self.page_size;
            if start < all.len() {
                self.sessions =
                    all[start..std::cmp::min(start + self.page_size, all.len())].to_vec();
            } else {
                self.sessions.clear();
            }
        } else {
            self.sessions.clear();
        }
        self.session_cursor = 0;
        Ok(())
    }

    pub async fn do_search(&mut self) -> anyhow::Result<()> {
        if let Some(proj) = &self.active_project {
            if !self.input_buffer.is_empty() {
                self.search_results = self
                    .storage
                    .search_observations(&proj.id, &self.input_buffer, self.page_size)
                    .await?;
            } else {
                self.search_results.clear();
            }
        }
        self.search_cursor = 0;
        Ok(())
    }

    pub async fn load_observation_detail(&mut self, id: &ObservationId) -> anyhow::Result<()> {
        self.detail_observation = self.storage.get_observation(id).await?;
        Ok(())
    }

    pub async fn load_session_detail(&mut self, id: &SessionId) -> anyhow::Result<()> {
        self.detail_session = self.storage.get_session(id).await?;
        self.detail_session_observations = self.storage.get_observations_by_session(id).await?;
        Ok(())
    }

    pub async fn soft_delete_selected(&mut self) -> anyhow::Result<()> {
        if let Some(obs) = self.selected_observation() {
            let id = obs.id.clone();
            let title = obs.title.clone();
            self.storage
                .delete_observation(&id, DeleteMode::Soft)
                .await?;
            self.status = format!("Soft-deleted: {}", title);
            self.reload_current_list().await?;
        }
        Ok(())
    }

    pub async fn hard_delete_confirmed(&mut self, id: &ObservationId) -> anyhow::Result<()> {
        let title = self
            .storage
            .get_observation(id)
            .await?
            .map(|o| o.title.clone())
            .unwrap_or_else(|| id.clone());
        self.storage
            .delete_observation(id, DeleteMode::Hard)
            .await?;
        self.status = format!("Hard-deleted: {}", title);
        self.confirm_action = None;
        self.reload_current_list().await?;
        Ok(())
    }

    pub async fn mark_reviewed_selected(&mut self) -> anyhow::Result<()> {
        if let Some(obs) = self.selected_observation() {
            let mut obs = obs.clone();
            obs.reviewed_at = Some(chrono::Utc::now());
            obs.review_after = Some(chrono::Utc::now() + chrono::Duration::days(7));
            self.storage.update_observation(&obs).await?;
            self.status = format!("Marked reviewed: {}", obs.title);
            self.reload_current_list().await?;
        }
        Ok(())
    }

    pub fn selected_observation(&self) -> Option<&Observation> {
        match self.tab {
            Tab::Observations => self.observations.get(self.observation_cursor),
            Tab::Search => self
                .search_results
                .get(self.search_cursor)
                .map(|r| &r.observation),
            _ => None,
        }
    }

    pub fn active_cursor(&self) -> usize {
        match self.tab {
            Tab::Projects => self.project_cursor,
            Tab::Observations => self.observation_cursor,
            Tab::Sessions => self.session_cursor,
            Tab::Search => self.search_cursor,
        }
    }

    pub fn active_list_len(&self) -> usize {
        match self.tab {
            Tab::Projects => self.projects.len(),
            Tab::Observations => self.observations.len(),
            Tab::Sessions => self.sessions.len(),
            Tab::Search => self.search_results.len(),
        }
    }

    pub fn cursor_up(&mut self) {
        match self.tab {
            Tab::Projects => self.project_cursor = self.project_cursor.saturating_sub(1),
            Tab::Observations => {
                self.observation_cursor = self.observation_cursor.saturating_sub(1)
            }
            Tab::Sessions => self.session_cursor = self.session_cursor.saturating_sub(1),
            Tab::Search => self.search_cursor = self.search_cursor.saturating_sub(1),
        }
    }

    pub fn cursor_down(&mut self) {
        let max = self.active_list_len().saturating_sub(1);
        match self.tab {
            Tab::Projects => self.project_cursor = (self.project_cursor + 1).min(max),
            Tab::Observations => self.observation_cursor = (self.observation_cursor + 1).min(max),
            Tab::Sessions => self.session_cursor = (self.session_cursor + 1).min(max),
            Tab::Search => self.search_cursor = (self.search_cursor + 1).min(max),
        }
    }

    pub async fn reload_current_list(&mut self) -> anyhow::Result<()> {
        match self.tab {
            Tab::Projects => self.load_projects().await?,
            Tab::Observations => self.load_observations().await?,
            Tab::Sessions => self.load_sessions().await?,
            Tab::Search => self.do_search().await?,
        }
        Ok(())
    }

    pub async fn enter_project(&mut self) -> anyhow::Result<()> {
        if let Some(proj) = self.projects.get(self.project_cursor).cloned() {
            let name = proj.name.clone();
            self.active_project = Some(proj);
            self.tab = Tab::Observations;
            self.observation_page = 0;
            self.load_observations().await?;
            self.status = format!("Project: {}", name);
        }
        Ok(())
    }

    pub async fn enter_observation_detail(&mut self) -> anyhow::Result<()> {
        if let Some(obs) = self.selected_observation() {
            let id = obs.id.clone();
            self.load_observation_detail(&id).await?;
            self.detail = DetailView::ObservationDetail(id);
        }
        Ok(())
    }

    pub async fn enter_session_detail(&mut self) -> anyhow::Result<()> {
        if let Some(sess) = self.sessions.get(self.session_cursor) {
            let id = sess.id.clone();
            self.load_session_detail(&id).await?;
            self.detail = DetailView::SessionDetail(id);
        }
        Ok(())
    }
}
