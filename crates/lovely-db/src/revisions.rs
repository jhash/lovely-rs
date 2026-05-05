//! Undo/redo storage. Each mutating endpoint snapshots the affected
//! page's full element rows; the page row holds a `revision_cursor`
//! pointing at the current state.

use crate::errors::DbError;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Captures the current rows for a page, truncates any redo branch
/// after the cursor, appends a new revision, and bumps the cursor.
pub async fn snapshot_page(pool: &PgPool, page_id: Uuid) -> Result<i64, DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    // Snapshot.
    let snapshot: serde_json::Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(row_to_json(e) ORDER BY created_at), '[]'::jsonb) \
         FROM elements e WHERE e.page_id = $1",
    )
    .bind(page_id)
    .fetch_one(&mut *tx)
    .await?;

    // Drop redo branch (any seq > current cursor).
    let cursor: Option<i64> =
        sqlx::query_scalar("SELECT revision_cursor FROM pages WHERE id = $1")
            .bind(page_id)
            .fetch_one(&mut *tx)
            .await?;
    if let Some(c) = cursor {
        sqlx::query("DELETE FROM element_revisions WHERE page_id = $1 AND seq > $2")
            .bind(page_id)
            .bind(c)
            .execute(&mut *tx)
            .await?;
    }

    // Insert new revision and update cursor.
    let seq: i64 = sqlx::query_scalar(
        "INSERT INTO element_revisions (page_id, snapshot_json) VALUES ($1, $2) RETURNING seq",
    )
    .bind(page_id)
    .bind(&snapshot)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("UPDATE pages SET revision_cursor = $2 WHERE id = $1")
        .bind(page_id)
        .bind(seq)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(seq)
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Undo,
    Redo,
}

/// Steps the cursor and replaces the page's elements from the target
/// revision's snapshot. Returns the new cursor seq, or None if there
/// was nothing to do.
pub async fn step(pool: &PgPool, page_id: Uuid, dir: Direction) -> Result<Option<i64>, DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    let cursor: Option<i64> =
        sqlx::query_scalar("SELECT revision_cursor FROM pages WHERE id = $1")
            .bind(page_id)
            .fetch_one(&mut *tx)
            .await?;
    let target_seq: Option<(i64,)> = match (dir, cursor) {
        (Direction::Undo, Some(c)) => sqlx::query_as(
            "SELECT seq FROM element_revisions \
             WHERE page_id = $1 AND seq < $2 \
             ORDER BY seq DESC LIMIT 1",
        )
        .bind(page_id)
        .bind(c)
        .fetch_optional(&mut *tx)
        .await?,
        (Direction::Redo, Some(c)) => sqlx::query_as(
            "SELECT seq FROM element_revisions \
             WHERE page_id = $1 AND seq > $2 \
             ORDER BY seq ASC LIMIT 1",
        )
        .bind(page_id)
        .bind(c)
        .fetch_optional(&mut *tx)
        .await?,
        (_, None) => None,
    };
    let Some((target,)) = target_seq else {
        tx.commit().await?;
        return Ok(None);
    };

    // Restore the snapshot.
    let snapshot: serde_json::Value = sqlx::query_scalar(
        "SELECT snapshot_json FROM element_revisions WHERE page_id = $1 AND seq = $2",
    )
    .bind(page_id)
    .bind(target)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM elements WHERE page_id = $1")
        .bind(page_id)
        .execute(&mut *tx)
        .await?;
    if let serde_json::Value::Array(rows) = snapshot {
        for row in rows {
            let id: Uuid = serde_json::from_value(row.get("id").cloned().unwrap_or_default())
                .map_err(|e| DbError::SnapshotDecode(e.to_string()))?;
            let parent_id: Option<Uuid> =
                serde_json::from_value(row.get("parent_id").cloned().unwrap_or_default())
                    .map_err(|e| DbError::SnapshotDecode(e.to_string()))?;
            let prev_sibling: Option<Uuid> =
                serde_json::from_value(row.get("prev_sibling").cloned().unwrap_or_default())
                    .map_err(|e| DbError::SnapshotDecode(e.to_string()))?;
            let tag: String = row
                .get("tag")
                .and_then(|v| v.as_str())
                .unwrap_or("div")
                .to_string();
            let attrs = row
                .get("attrs")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
            let payload = row
                .get("payload")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
            sqlx::query(
                "INSERT INTO elements (id, page_id, parent_id, prev_sibling, tag, attrs, payload) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(id)
            .bind(page_id)
            .bind(parent_id)
            .bind(prev_sibling)
            .bind(tag)
            .bind(&attrs)
            .bind(&payload)
            .execute(&mut *tx)
            .await?;
        }
    }
    sqlx::query("UPDATE pages SET revision_cursor = $2 WHERE id = $1")
        .bind(page_id)
        .bind(target)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Some(target))
}
