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
        RETURNING id, slug, name, description, owner_id, is_default, created_at, updated_at
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
        "SELECT id, slug, name, description, owner_id, is_default, created_at, updated_at \
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
        "SELECT id, slug, name, description, owner_id, is_default, created_at, updated_at \
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
        "SELECT id, slug, name, description, owner_id, is_default, created_at, updated_at \
         FROM apps WHERE owner_id = $1 ORDER BY is_default DESC, name ASC",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
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
