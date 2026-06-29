use async_trait::async_trait;

use crate::auth::AuthManager;
use crate::memory::types::StoreHealth;
use crate::storage::{Storage, StorageCapabilities, UnstructuredStorage};
use memwal_core::types::{RecallParams, RememberBulkItem};
use std::sync::Arc;

pub struct MemwalStorage {
    auth_manager: Arc<AuthManager>,
}

impl MemwalStorage {
    pub fn new(auth_manager: Arc<AuthManager>) -> Self {
        Self { auth_manager }
    }
}

#[async_trait]
impl Storage for MemwalStorage {
    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities::Unstructured
    }

    fn as_unstructured(&self) -> Option<&dyn UnstructuredStorage> {
        Some(self)
    }

    async fn health_check(&self) -> anyhow::Result<StoreHealth> {
        // If we can get a client, it's broadly healthy.
        match self.auth_manager.memwal_client().await {
            Ok(_) => Ok(StoreHealth {
                readable: true,
                writable: true,
                corruption_detected: false,
                orphaned_observations: 0,
                orphaned_sessions: 0,
            }),
            Err(_) => Ok(StoreHealth {
                readable: false,
                writable: false,
                corruption_detected: false,
                orphaned_observations: 0,
                orphaned_sessions: 0,
            }),
        }
    }
}

#[async_trait]
impl UnstructuredStorage for MemwalStorage {
    async fn remember(&self, text: &str, namespace: Option<&str>) -> anyhow::Result<String> {
        let client = self.auth_manager.memwal_client().await?;
        tracing::info!(
            "MemwalStorage::remember -> namespace={:?}, text_len={}",
            namespace,
            text.len()
        );
        if let Some(ns) = namespace {
            let item = RememberBulkItem {
                text: text.to_string(),
                namespace: Some(ns.to_string()),
            };
            // Bulk remember array of 1
            let job_ids = client.remember_bulk(&[item]).await?;
            let job_id = job_ids.job_ids.first().cloned().unwrap_or_default();
            tracing::info!(
                "MemwalStorage::remember bulk completed -> job_id={}",
                job_id
            );
            Ok(job_id)
        } else {
            let job_id = client.remember_async(text).await?;
            tracing::info!(
                "MemwalStorage::remember_async completed -> job_id={}",
                job_id.job_id
            );
            Ok(job_id.job_id)
        }
    }

    async fn recall(&self, query: &str, namespace: Option<&str>) -> anyhow::Result<Vec<String>> {
        let client = self.auth_manager.memwal_client().await?;
        tracing::info!(
            "MemwalStorage::recall -> namespace={:?}, query='{}'",
            namespace,
            query
        );
        let params = RecallParams {
            query: query.to_string(),
            namespace: namespace.map(|s| s.to_string()),
            limit: Some(10),
            max_distance: None,
            top_k: None,
        };

        let results = client.recall(params).await?;
        tracing::info!(
            "MemwalStorage::recall completed -> found {} results",
            results.results.len()
        );
        Ok(results.results.into_iter().map(|item| item.text).collect())
    }
}
