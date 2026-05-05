use crate::errors::DbError;
use chrono::{DateTime, Utc};
use lovely_tree::{ElementTag, ElementUuid};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Page {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub root_element: Option<Uuid>,
    pub author_id: Uuid,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewPage {
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub author_id: Uuid,
    pub root_tag: ElementTag,
}

/// Creates a page row plus a root element row in one transaction. The
/// root_element FK is set on the page after the element row exists.
pub async fn create_page(pool: &PgPool, new: NewPage) -> Result<(Page, Uuid), DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    let page = sqlx::query_as::<_, Page>(
        r#"
        INSERT INTO pages (slug, title, description, author_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id, slug, title, description, root_element, author_id, published_at, created_at, updated_at
        "#,
    )
    .bind(&new.slug)
    .bind(&new.title)
    .bind(&new.description)
    .bind(new.author_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(crate::users::map_unique_violation)?;

    let element_id: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO elements (page_id, parent_id, prev_sibling, tag, attrs, payload)
        VALUES ($1, NULL, NULL, $2, '{}'::jsonb, '{}'::jsonb)
        RETURNING id
        "#,
    )
    .bind(page.id)
    .bind(new.root_tag.name())
    .fetch_one(&mut *tx)
    .await?;

    let updated = sqlx::query_as::<_, Page>(
        "UPDATE pages SET root_element = $2, updated_at = now() WHERE id = $1 \
         RETURNING id, slug, title, description, root_element, author_id, published_at, created_at, updated_at",
    )
    .bind(page.id)
    .bind(element_id.0)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((updated, element_id.0))
}

pub async fn find_page_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Page>, DbError> {
    let row = sqlx::query_as::<_, Page>(
        "SELECT id, slug, title, description, root_element, author_id, published_at, created_at, updated_at \
         FROM pages WHERE slug = $1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_page_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Page>, DbError> {
    let row = sqlx::query_as::<_, Page>(
        "SELECT id, slug, title, description, root_element, author_id, published_at, created_at, updated_at \
         FROM pages WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_pages_by_author(pool: &PgPool, author_id: Uuid) -> Result<Vec<Page>, DbError> {
    let rows = sqlx::query_as::<_, Page>(
        "SELECT id, slug, title, description, root_element, author_id, published_at, created_at, updated_at \
         FROM pages WHERE author_id = $1 ORDER BY updated_at DESC",
    )
    .bind(author_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_published_pages(pool: &PgPool) -> Result<Vec<Page>, DbError> {
    let rows = sqlx::query_as::<_, Page>(
        "SELECT id, slug, title, description, root_element, author_id, published_at, created_at, updated_at \
         FROM pages WHERE published_at IS NOT NULL ORDER BY published_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Clone, Debug, Default)]
pub struct PagePatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub publish: Option<bool>,
}

pub async fn update_page(pool: &PgPool, id: Uuid, patch: PagePatch) -> Result<Page, DbError> {
    let row = sqlx::query_as::<_, Page>(
        r#"
        UPDATE pages
        SET title        = COALESCE($2, title),
            description  = CASE WHEN $3::boolean THEN $4 ELSE description END,
            published_at = CASE WHEN $5::boolean THEN
                                  CASE WHEN $6 THEN now() ELSE NULL END
                              ELSE published_at END,
            updated_at   = now()
        WHERE id = $1
        RETURNING id, slug, title, description, root_element, author_id, published_at, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(patch.title.as_deref())
    .bind(patch.description.is_some())
    .bind(patch.description.flatten())
    .bind(patch.publish.is_some())
    .bind(patch.publish.unwrap_or(false))
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete_page(pool: &PgPool, id: Uuid) -> Result<u64, DbError> {
    // pages.root_element FK is ON DELETE SET NULL, but elements have
    // page_id ON DELETE CASCADE — so deleting the page wipes the elements.
    let n = sqlx::query("DELETE FROM pages WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(n)
}

/// Marker re-export so [`ElementUuid`] is importable from this module.
pub use lovely_tree::ElementUuid as PageElementUuid;

#[allow(dead_code)]
fn _ensure_uuid_in_scope(_e: ElementUuid) {}
