use crate::errors::DbError;
use chrono::{DateTime, Utc};
use lovely_tree::{ElementRow, ElementUuid};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ElementDbRow {
    pub id: Uuid,
    pub page_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub prev_sibling: Option<Uuid>,
    pub tag: String,
    pub attrs: serde_json::Value,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ElementDbRow {
    /// True when this DB row matches the supplied canonical tag name.
    pub fn is_tag(&self, name: &str) -> bool {
        self.tag == name
    }

    /// True when this row stores an inline `#text` node.
    pub fn is_text(&self) -> bool {
        lovely_tree::tags::is_text_tag(&self.tag)
    }

    pub fn into_tree_row(self) -> ElementRow {
        // Only #text nodes carry text. Other tags store text inside
        // `#text` child elements — the legacy payload.text on regular
        // elements is ignored.
        let text = if self.is_text() {
            extract_text(&self.payload)
        } else {
            None
        };
        ElementRow {
            id: ElementUuid(self.id),
            parent_id: self.parent_id.map(ElementUuid),
            prev_sibling: self.prev_sibling.map(ElementUuid),
            tag: self.tag,
            attrs_json: self.attrs,
            text,
        }
    }
}

fn extract_text(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
}

pub async fn load_elements_for_page(
    pool: &PgPool,
    page_id: Uuid,
) -> Result<Vec<ElementRow>, DbError> {
    let rows = sqlx::query_as::<_, ElementDbRow>(
        "SELECT id, page_id, parent_id, prev_sibling, tag, attrs, payload, created_at, updated_at \
         FROM elements WHERE page_id = $1",
    )
    .bind(page_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.into_tree_row()).collect())
}

#[derive(Clone, Debug)]
pub struct InsertElement {
    pub page_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub prev_sibling: Option<Uuid>,
    pub tag: String,
    pub attrs: serde_json::Value,
    pub payload: serde_json::Value,
}

pub async fn insert_element(pool: &PgPool, insert: InsertElement) -> Result<ElementDbRow, DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    // If the new element is being inserted in the middle of a sibling chain,
    // patch the existing sibling that previously had this prev_sibling.
    let new_id: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO elements (page_id, parent_id, prev_sibling, tag, attrs, payload)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind(insert.page_id)
    .bind(insert.parent_id)
    .bind(insert.prev_sibling)
    .bind(&insert.tag)
    .bind(&insert.attrs)
    .bind(&insert.payload)
    .fetch_one(&mut *tx)
    .await?;
    // Relink: any element that previously had `insert.prev_sibling` as its
    // prev_sibling AND the same parent should now point at the new id.
    sqlx::query(
        "UPDATE elements SET prev_sibling = $1, updated_at = now() \
         WHERE page_id = $2 AND id != $1 \
           AND parent_id IS NOT DISTINCT FROM $3 \
           AND prev_sibling IS NOT DISTINCT FROM $4",
    )
    .bind(new_id.0)
    .bind(insert.page_id)
    .bind(insert.parent_id)
    .bind(insert.prev_sibling)
    .execute(&mut *tx)
    .await?;
    let row = sqlx::query_as::<_, ElementDbRow>(
        "SELECT id, page_id, parent_id, prev_sibling, tag, attrs, payload, created_at, updated_at \
         FROM elements WHERE id = $1",
    )
    .bind(new_id.0)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn delete_element(pool: &PgPool, id: Uuid) -> Result<u64, DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    // Find the element being deleted to learn its prev_sibling, so we can
    // relink the next sibling to bypass it.
    let target: Option<(Uuid, Option<Uuid>, Option<Uuid>)> =
        sqlx::query_as("SELECT page_id, parent_id, prev_sibling FROM elements WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((page_id, parent_id, prev_sibling)) = target else {
        tx.commit().await?;
        return Ok(0);
    };
    // Bypass.
    sqlx::query(
        "UPDATE elements SET prev_sibling = $1, updated_at = now() \
         WHERE page_id = $2 \
           AND parent_id IS NOT DISTINCT FROM $3 \
           AND prev_sibling = $4",
    )
    .bind(prev_sibling)
    .bind(page_id)
    .bind(parent_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    let n = sqlx::query("DELETE FROM elements WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    tx.commit().await?;
    Ok(n)
}

#[derive(Clone, Debug, Default)]
pub struct ElementPatch {
    pub tag: Option<String>,
    pub attrs: Option<serde_json::Value>,
    pub payload: Option<serde_json::Value>,
}

pub async fn update_element(
    pool: &PgPool,
    id: Uuid,
    patch: ElementPatch,
) -> Result<ElementDbRow, DbError> {
    let row = sqlx::query_as::<_, ElementDbRow>(
        r#"
        UPDATE elements
        SET tag      = COALESCE($2, tag),
            attrs    = COALESCE($3, attrs),
            payload  = COALESCE($4, payload),
            updated_at = now()
        WHERE id = $1
        RETURNING id, page_id, parent_id, prev_sibling, tag, attrs, payload, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(patch.tag.as_deref())
    .bind(patch.attrs)
    .bind(patch.payload)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
