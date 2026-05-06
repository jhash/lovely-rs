use crate::errors::DbError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct App {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_default: bool,
    #[sqlx(default)]
    pub theme_json: serde_json::Value,
    #[sqlx(default)]
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewApp {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_default: bool,
}

pub async fn create_app(pool: &PgPool, new: NewApp) -> Result<App, DbError> {
    let row = sqlx::query_as::<_, App>(
        r#"
        INSERT INTO apps (slug, name, description, owner_id, is_default)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, slug, name, description, owner_id, is_default, theme_json, published_at, created_at, updated_at
        "#,
    )
    .bind(&new.slug)
    .bind(&new.name)
    .bind(&new.description)
    .bind(new.owner_id)
    .bind(new.is_default)
    .fetch_one(pool)
    .await
    .map_err(crate::users::map_unique_violation)?;
    Ok(row)
}

pub async fn find_default_app_for_owner(
    pool: &PgPool,
    owner_id: Uuid,
) -> Result<Option<App>, DbError> {
    let row = sqlx::query_as::<_, App>(
        "SELECT id, slug, name, description, owner_id, is_default, theme_json, published_at, created_at, updated_at \
         FROM apps WHERE owner_id = $1 AND is_default = TRUE",
    )
    .bind(owner_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_app_by_owner_and_slug(
    pool: &PgPool,
    owner_id: Uuid,
    slug: &str,
) -> Result<Option<App>, DbError> {
    let row = sqlx::query_as::<_, App>(
        "SELECT id, slug, name, description, owner_id, is_default, theme_json, published_at, created_at, updated_at \
         FROM apps WHERE owner_id = $1 AND slug = $2",
    )
    .bind(owner_id)
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_apps_by_owner(pool: &PgPool, owner_id: Uuid) -> Result<Vec<App>, DbError> {
    let rows = sqlx::query_as::<_, App>(
        "SELECT id, slug, name, description, owner_id, is_default, theme_json, published_at, created_at, updated_at \
         FROM apps WHERE owner_id = $1 ORDER BY is_default DESC, name ASC",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Clone, Debug, Default)]
pub struct AppPatch {
    pub slug: Option<String>,
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

pub async fn update_app(pool: &PgPool, id: Uuid, patch: AppPatch) -> Result<App, DbError> {
    let row = sqlx::query_as::<_, App>(
        r#"
        UPDATE apps
        SET slug        = COALESCE($2, slug),
            name        = COALESCE($3, name),
            description = CASE WHEN $4::boolean THEN $5 ELSE description END,
            updated_at  = now()
        WHERE id = $1
        RETURNING id, slug, name, description, owner_id, is_default, theme_json, published_at, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(patch.slug.as_deref())
    .bind(patch.name.as_deref())
    .bind(patch.description.is_some())
    .bind(patch.description.flatten())
    .fetch_one(pool)
    .await
    .map_err(crate::users::map_unique_violation)?;
    Ok(row)
}

pub async fn update_app_theme(
    pool: &PgPool,
    id: Uuid,
    theme: serde_json::Value,
) -> Result<(), DbError> {
    sqlx::query("UPDATE apps SET theme_json = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(&theme)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_app_published(pool: &PgPool, id: Uuid, publish: bool) -> Result<(), DbError> {
    sqlx::query(
        "UPDATE apps SET published_at = CASE WHEN $2 THEN now() ELSE NULL END, \
         updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(publish)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_published_apps_by_owner(
    pool: &PgPool,
    owner_id: Uuid,
) -> Result<Vec<App>, DbError> {
    let rows = sqlx::query_as::<_, App>(
        "SELECT id, slug, name, description, owner_id, is_default, theme_json, published_at, \
         created_at, updated_at \
         FROM apps WHERE owner_id = $1 AND published_at IS NOT NULL \
         ORDER BY is_default DESC, name ASC",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete_app(pool: &PgPool, id: Uuid) -> Result<u64, DbError> {
    let n = sqlx::query("DELETE FROM apps WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(n)
}

pub async fn count_apps_for_owner(pool: &PgPool, owner_id: Uuid) -> Result<i64, DbError> {
    let n: (i64,) = sqlx::query_as("SELECT count(*) FROM apps WHERE owner_id = $1")
        .bind(owner_id)
        .fetch_one(pool)
        .await?;
    Ok(n.0)
}

/// Look up the default app for a user identified by username — used by
/// the public `/:username/:slug` URL resolver.
pub async fn find_default_app_for_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<(crate::users::User, App)>, DbError> {
    let Some(user) = crate::users::find_user_by_username(pool, username).await? else {
        return Ok(None);
    };
    let Some(app) = find_default_app_for_owner(pool, user.id).await? else {
        return Ok(None);
    };
    Ok(Some((user, app)))
}
