use crate::errors::DbError;
use uuid::Uuid;

pub type AppId = Uuid;

/// Pluggable store for per-app SQLite databases. The local impl lives next
/// to the web service; a future remote impl will speak to a separate
/// `lovely-data` binary over RPC. v1 ships a stub that errors on any call.
#[async_trait::async_trait]
pub trait SqliteAppStore: Send + Sync + 'static {
    async fn get_pool(&self, app_id: AppId) -> Result<sqlx::SqlitePool, DbError>;
    async fn ensure_migrated(&self, app_id: AppId) -> Result<(), DbError>;
    async fn close_pool(&self, app_id: AppId) -> Result<(), DbError>;
    async fn delete_app(&self, app_id: AppId) -> Result<(), DbError>;
}

pub struct StubSqliteAppStore;

#[async_trait::async_trait]
impl SqliteAppStore for StubSqliteAppStore {
    async fn get_pool(&self, app_id: AppId) -> Result<sqlx::SqlitePool, DbError> {
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
