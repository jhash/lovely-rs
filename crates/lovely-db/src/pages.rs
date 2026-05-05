use crate::errors::DbError;
use chrono::{DateTime, Utc};
use lovely_tree::ElementTag;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Page {
    pub id: Uuid,
    pub app_id: Uuid,
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
    pub app_id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub author_id: Uuid,
    pub root_tag: ElementTag,
}

const PAGE_COLUMNS: &str = "id, app_id, slug, title, description, root_element, author_id, \
     published_at, created_at, updated_at";

/// Creates a page row plus a root element row in one transaction.
pub async fn create_page(pool: &PgPool, new: NewPage) -> Result<(Page, Uuid), DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    let page = sqlx::query_as::<_, Page>(&format!(
        "INSERT INTO pages (app_id, slug, title, description, author_id) \
         VALUES ($1, $2, $3, $4, $5) RETURNING {PAGE_COLUMNS}"
    ))
    .bind(new.app_id)
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

    let updated = sqlx::query_as::<_, Page>(&format!(
        "UPDATE pages SET root_element = $2, updated_at = now() WHERE id = $1 \
         RETURNING {PAGE_COLUMNS}"
    ))
    .bind(page.id)
    .bind(element_id.0)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((updated, element_id.0))
}

pub async fn find_page_by_app_and_slug(
    pool: &PgPool,
    app_id: Uuid,
    slug: &str,
) -> Result<Option<Page>, DbError> {
    let row = sqlx::query_as::<_, Page>(&format!(
        "SELECT {PAGE_COLUMNS} FROM pages WHERE app_id = $1 AND slug = $2"
    ))
    .bind(app_id)
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_page_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Page>, DbError> {
    let row = sqlx::query_as::<_, Page>(&format!("SELECT {PAGE_COLUMNS} FROM pages WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_pages_in_app(pool: &PgPool, app_id: Uuid) -> Result<Vec<Page>, DbError> {
    let rows = sqlx::query_as::<_, Page>(&format!(
        "SELECT {PAGE_COLUMNS} FROM pages WHERE app_id = $1 ORDER BY \
         CASE WHEN slug = '' THEN 0 ELSE 1 END, slug ASC"
    ))
    .bind(app_id)
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
    let row = sqlx::query_as::<_, Page>(&format!(
        r#"
        UPDATE pages
        SET title        = COALESCE($2, title),
            description  = CASE WHEN $3::boolean THEN $4 ELSE description END,
            published_at = CASE WHEN $5::boolean THEN
                                  CASE WHEN $6 THEN now() ELSE NULL END
                              ELSE published_at END,
            updated_at   = now()
        WHERE id = $1
        RETURNING {PAGE_COLUMNS}
        "#,
    ))
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
    let n = sqlx::query("DELETE FROM pages WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(n)
}
