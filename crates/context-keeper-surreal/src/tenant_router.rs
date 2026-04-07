use std::sync::atomic::{AtomicUsize, Ordering};

use context_keeper_core::error::Result;
use context_keeper_core::ContextKeeperError;
use dashmap::DashMap;
use tokio::sync::Mutex;

use crate::client::{connect, SurrealConfig};
use crate::repository::Repository;
use crate::schema::apply_schema;

pub const DEFAULT_TENANT_ID: &str = "main";

/// Routes each tenant to a dedicated SurrealDB connection bound to its own database.
///
/// Each tenant maps to `(ns=config.namespace, db=tenant_id)`. Connections and
/// schema are lazily initialised on first access and cached for reuse.
/// A configurable cap prevents unbounded connection growth.
pub struct TenantRouter {
    tenants: DashMap<String, Repository>,
    config_template: SurrealConfig,
    max_tenants: usize,
    tenant_count: AtomicUsize,
    creation_lock: Mutex<()>,
}

impl TenantRouter {
    pub fn new(config_template: SurrealConfig, max_tenants: usize) -> Self {
        Self {
            tenants: DashMap::new(),
            config_template,
            max_tenants,
            tenant_count: AtomicUsize::new(0),
            creation_lock: Mutex::new(()),
        }
    }

    /// Get or lazily create a [`Repository`] bound to the given tenant's database.
    ///
    /// On a cache miss the method acquires a creation lock, connects to
    /// `(namespace, tenant_id)`, applies the schema, and caches the result.
    pub async fn get_or_create(&self, tenant_id: &str) -> Result<Repository> {
        if let Some(repo) = self.tenants.get(tenant_id) {
            return Ok(repo.value().clone());
        }

        let _guard = self.creation_lock.lock().await;

        // Double-check after acquiring the lock.
        if let Some(repo) = self.tenants.get(tenant_id) {
            return Ok(repo.value().clone());
        }

        if self.tenant_count.load(Ordering::Relaxed) >= self.max_tenants {
            return Err(ContextKeeperError::ValidationError(format!(
                "tenant limit reached (max {})",
                self.max_tenants
            )));
        }

        let config = SurrealConfig {
            database: tenant_id.to_string(),
            ..self.config_template.clone()
        };

        tracing::info!(
            tenant_id = tenant_id,
            ns = %config.namespace,
            db = %config.database,
            "Provisioning new tenant database"
        );

        let db = connect(&config).await?;
        apply_schema(&db, &config).await?;
        let repo = Repository::new(db);

        self.tenants.insert(tenant_id.to_string(), repo.clone());
        self.tenant_count.fetch_add(1, Ordering::Relaxed);

        Ok(repo)
    }

    pub fn tenant_count(&self) -> usize {
        self.tenant_count.load(Ordering::Relaxed)
    }

    pub fn contains(&self, tenant_id: &str) -> bool {
        self.tenants.contains_key(tenant_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_config(dims: usize) -> SurrealConfig {
        SurrealConfig {
            embedding_dimensions: dims,
            ..SurrealConfig::default()
        }
    }

    #[tokio::test]
    async fn get_or_create_returns_same_repo_on_second_call() {
        let router = TenantRouter::new(mem_config(3), 10);
        let r1 = router.get_or_create("alpha").await.unwrap();
        let r2 = router.get_or_create("alpha").await.unwrap();

        // Both should be the same cloned handle — verify via a write/read.
        use chrono::Utc;
        use context_keeper_core::models::Episode;
        use uuid::Uuid;
        let ep = Episode {
            id: Uuid::new_v4(),
            content: "hello".into(),
            source: "test".into(),
            session_id: None,
            agent: None,
            namespace: None,
            created_at: Utc::now(),
        };
        r1.create_episode(&ep).await.unwrap();
        let found = r2.get_episode(ep.id).await.unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn distinct_tenants_are_isolated() {
        let router = TenantRouter::new(mem_config(3), 10);
        let repo_a = router.get_or_create("tenant_a").await.unwrap();
        let repo_b = router.get_or_create("tenant_b").await.unwrap();

        use chrono::Utc;
        use context_keeper_core::models::Episode;
        use uuid::Uuid;
        let ep = Episode {
            id: Uuid::new_v4(),
            content: "secret".into(),
            source: "test".into(),
            session_id: None,
            agent: None,
            namespace: None,
            created_at: Utc::now(),
        };
        repo_a.create_episode(&ep).await.unwrap();

        let found_b = repo_b.get_episode(ep.id).await.unwrap();
        assert!(found_b.is_none(), "tenant_b must NOT see tenant_a's data");

        assert_eq!(router.tenant_count(), 2);
    }

    #[tokio::test]
    async fn rejects_when_limit_reached() {
        let router = TenantRouter::new(mem_config(3), 1);
        router.get_or_create("first").await.unwrap();
        let err = router.get_or_create("second").await;
        match err {
            Err(e) => assert!(
                e.to_string().contains("tenant limit"),
                "expected tenant limit error",
            ),
            Ok(_) => panic!("expected error when tenant limit reached"),
        }
    }
}
