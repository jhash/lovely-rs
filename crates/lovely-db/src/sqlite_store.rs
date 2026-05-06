//! Pluggable store for per-app SQLite databases.
//!
//! `SqliteAppStore` is the boundary the rest of `lovely-web` calls
//! through. The local impl writes one `.sqlite` file per app under a
//! configured root directory and lazily migrates each pool on first
//! use. A future remote impl will speak to a separate `lovely-data`
//! binary over RPC.

use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::errors::DbError;
use crate::schema_service::SchemaService;

pub type AppId = Uuid;

#[async_trait::async_trait]
pub trait SqliteAppStore: Send + Sync + 'static {
    async fn get_pool(&self, app_id: AppId) -> Result<SqlitePool, DbError>;
    async fn ensure_migrated(&self, app_id: AppId) -> Result<(), DbError>;
    async fn close_pool(&self, app_id: AppId) -> Result<(), DbError>;
    async fn delete_app(&self, app_id: AppId) -> Result<(), DbError>;
}

pub struct StubSqliteAppStore;

#[async_trait::async_trait]
impl SqliteAppStore for StubSqliteAppStore {
    async fn get_pool(&self, app_id: AppId) -> Result<SqlitePool, DbError> {
        Err(DbError::AppNotFound(app_id))
    }
    async fn ensure_migrated(&self, _: AppId) -> Result<(), DbError> {
        Ok(())
    }
    async fn close_pool(&self, _: AppId) -> Result<(), DbError> {
        Ok(())
    }
    async fn delete_app(&self, _: AppId) -> Result<(), DbError> {
        Ok(())
    }
}

/// Disk-backed SQLite store.
///
/// Pools are created on demand and cached forever (until `close_pool`).
/// Each `get_pool` runs `ensure_migrated` so the caller never needs to
/// remember to do it themselves — the cost is one cheap version-pointer
/// read per call after the first migration.
pub struct LocalSqliteAppStore {
    root: PathBuf,
    schema: Arc<SchemaService>,
    pools: DashMap<AppId, SqlitePool>,
}

impl LocalSqliteAppStore {
    pub fn new(root: impl Into<PathBuf>, schema: Arc<SchemaService>) -> Result<Self, DbError> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            schema,
            pools: DashMap::new(),
        })
    }

    fn path_for(&self, app_id: AppId) -> PathBuf {
        self.root.join(format!("{app_id}.sqlite"))
    }

    async fn open_pool(&self, app_id: AppId) -> Result<SqlitePool, DbError> {
        let path = self.path_for(app_id);
        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await?;
        Ok(pool)
    }
}

#[async_trait::async_trait]
impl SqliteAppStore for LocalSqliteAppStore {
    async fn get_pool(&self, app_id: AppId) -> Result<SqlitePool, DbError> {
        if let Some(p) = self.pools.get(&app_id) {
            // Even on cache hit, replay any newly-recorded migrations.
            // ensure_migrated is cheap when there's nothing pending.
            self.schema.ensure_migrated(app_id, p.value()).await?;
            return Ok(p.value().clone());
        }
        let pool = self.open_pool(app_id).await?;
        self.schema.ensure_migrated(app_id, &pool).await?;
        self.pools.insert(app_id, pool.clone());
        Ok(pool)
    }

    async fn ensure_migrated(&self, app_id: AppId) -> Result<(), DbError> {
        let pool = self.get_pool(app_id).await?;
        self.schema.ensure_migrated(app_id, &pool).await
    }

    async fn close_pool(&self, app_id: AppId) -> Result<(), DbError> {
        if let Some((_, pool)) = self.pools.remove(&app_id) {
            pool.close().await;
        }
        Ok(())
    }

    async fn delete_app(&self, app_id: AppId) -> Result<(), DbError> {
        self.close_pool(app_id).await?;
        let path = self.path_for(app_id);
        // Best-effort: missing is fine, anything else propagates.
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
