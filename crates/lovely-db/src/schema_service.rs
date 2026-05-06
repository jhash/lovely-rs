//! Records and applies per-app schema changes.
//!
//! Postgres is the source of truth: every accepted [`Intent`] gets a
//! row in `app_schema_migrations`. SQLite databases are derived state —
//! deleting one and reopening just replays the log.
//!
//! Concurrency:
//! - One in-process `tokio::Mutex` per app guards `ensure_migrated`, so
//!   two workers in the same process can't race to apply the same
//!   migration twice.
//! - Cross-process safety relies on `BEGIN IMMEDIATE` plus a version
//!   pointer in the SQLite database (`_lovely_schema_version`). A
//!   second process that reaches the same migration sees the bumped
//!   pointer inside the transaction and skips.
//! - `record` uses Postgres `SELECT ... FOR UPDATE` on the parent
//!   `apps` row to serialize version assignment.

use std::sync::Arc;

use dashmap::DashMap;
use sqlx::{PgPool, Row, SqlitePool};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::errors::DbError;
use crate::intent::Intent;

pub type AppId = Uuid;
pub type UserId = Uuid;
pub type MigrationVersion = i64;

/// Services schema migrations for many apps. Keep one per process.
pub struct SchemaService {
    pg: PgPool,
    locks: DashMap<AppId, Arc<Mutex<()>>>,
}

impl SchemaService {
    pub fn new(pg: PgPool) -> Self {
        Self {
            pg,
            locks: DashMap::new(),
        }
    }

    fn lock_for(&self, app_id: AppId) -> Arc<Mutex<()>> {
        self.locks
            .entry(app_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Validate the intent, render its SQL, and insert a row into
    /// `app_schema_migrations`. Returns the assigned version.
    ///
    /// Does NOT touch SQLite — call [`SchemaService::ensure_migrated`]
    /// afterwards (or let the next get_pool() call do it lazily).
    pub async fn record(
        &self,
        app_id: AppId,
        user: UserId,
        intent: Intent,
    ) -> Result<MigrationVersion, DbError> {
        let ddl = intent.render_sqlite()?;
        let intent_json = serde_json::to_value(&intent).map_err(|e| {
            DbError::SchemaConflict(format!("intent serialize: {e}"))
        })?;

        let mut tx = self.pg.begin().await?;

        // Lock the parent apps row so concurrent record() calls for the
        // same app serialize on version assignment. Verifies the app
        // exists in the same hop.
        let exists: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM apps WHERE id = $1 FOR UPDATE")
                .bind(app_id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(DbError::AppNotFound(app_id));
        }

        let next_version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version), 0) + 1
               FROM app_schema_migrations WHERE app_id = $1",
        )
        .bind(app_id)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO app_schema_migrations
                 (app_id, version, intent, forward_sql, reverse_sql, created_by)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(app_id)
        .bind(next_version)
        .bind(intent_json)
        .bind(&ddl.forward)
        .bind(ddl.reverse.as_deref())
        .bind(user)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(next_version)
    }

    /// Apply any pending migrations for `app_id` to the given SQLite
    /// pool. Idempotent: calling repeatedly is cheap (one SELECT to
    /// the version pointer + an empty pending list).
    pub async fn ensure_migrated(
        &self,
        app_id: AppId,
        sqlite: &SqlitePool,
    ) -> Result<(), DbError> {
        let lock = self.lock_for(app_id);
        let _guard = lock.lock().await;

        ensure_version_table(sqlite).await?;
        let applied = read_applied_version(sqlite).await?;

        let pending: Vec<(i64, String)> = sqlx::query_as(
            "SELECT version, forward_sql FROM app_schema_migrations
              WHERE app_id = $1 AND version > $2
              ORDER BY version",
        )
        .bind(app_id)
        .bind(applied)
        .fetch_all(&self.pg)
        .await?;

        for (version, forward_sql) in pending {
            apply_one(sqlite, version, &forward_sql).await?;
        }
        Ok(())
    }

    /// Test/ops helper: list applied migrations for an app.
    pub async fn list_for_app(
        &self,
        app_id: AppId,
    ) -> Result<Vec<(MigrationVersion, Intent)>, DbError> {
        let rows = sqlx::query(
            "SELECT version, intent FROM app_schema_migrations
              WHERE app_id = $1 ORDER BY version",
        )
        .bind(app_id)
        .fetch_all(&self.pg)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let v: i64 = row.try_get("version")?;
            let intent: serde_json::Value = row.try_get("intent")?;
            let intent: Intent = serde_json::from_value(intent)
                .map_err(|e| DbError::SnapshotDecode(e.to_string()))?;
            out.push((v, intent));
        }
        Ok(out)
    }
}

async fn ensure_version_table(sqlite: &SqlitePool) -> Result<(), DbError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _lovely_schema_version (
             rowid INTEGER PRIMARY KEY CHECK (rowid = 1),
             applied_version INTEGER NOT NULL DEFAULT 0
         )",
    )
    .execute(sqlite)
    .await?;
    sqlx::query(
        "INSERT OR IGNORE INTO _lovely_schema_version (rowid, applied_version) VALUES (1, 0)",
    )
    .execute(sqlite)
    .await?;
    Ok(())
}

async fn read_applied_version(sqlite: &SqlitePool) -> Result<i64, DbError> {
    let v: i64 = sqlx::query_scalar(
        "SELECT applied_version FROM _lovely_schema_version WHERE rowid = 1",
    )
    .fetch_one(sqlite)
    .await?;
    Ok(v)
}

async fn apply_one(
    sqlite: &SqlitePool,
    version: i64,
    forward_sql: &str,
) -> Result<(), DbError> {
    let mut tx = sqlite.begin().await?;
    // Re-read the version pointer inside the txn — guards against a
    // racing process that already applied this migration.
    let current: i64 = sqlx::query_scalar(
        "SELECT applied_version FROM _lovely_schema_version WHERE rowid = 1",
    )
    .fetch_one(&mut *tx)
    .await?;
    if current >= version {
        // Another process beat us to it. No-op.
        return Ok(());
    }
    sqlx::query(forward_sql).execute(&mut *tx).await?;
    sqlx::query("UPDATE _lovely_schema_version SET applied_version = ? WHERE rowid = 1")
        .bind(version)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
