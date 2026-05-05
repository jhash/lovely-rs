use crate::errors::DbError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum UserRole {
    User,
    SuperAdmin,
}

#[derive(Clone, Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub totp_secret: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default)]
pub struct NewUser {
    pub username: String,
    pub email: Option<String>,
    pub password_hash: Option<String>,
}

pub async fn create_user(pool: &PgPool, new_user: NewUser) -> Result<User, DbError> {
    let row = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (username, email, password_hash)
        VALUES ($1, $2, $3)
        RETURNING id, username, email, password_hash, totp_secret, role, created_at, updated_at
        "#,
    )
    .bind(&new_user.username)
    .bind(&new_user.email)
    .bind(&new_user.password_hash)
    .fetch_one(pool)
    .await
    .map_err(map_unique_violation)?;
    Ok(row)
}

pub async fn find_user_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, DbError> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, totp_secret, role, created_at, updated_at \
         FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_user_by_username(pool: &PgPool, username: &str) -> Result<Option<User>, DbError> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, totp_secret, role, created_at, updated_at \
         FROM users WHERE username = $1",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn set_password_hash(pool: &PgPool, id: Uuid, hash: &str) -> Result<(), DbError> {
    let n = sqlx::query("UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(hash)
        .execute(pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(DbError::UserNotFound);
    }
    Ok(())
}

pub async fn set_totp_secret(pool: &PgPool, id: Uuid, secret: Option<&str>) -> Result<(), DbError> {
    let n = sqlx::query("UPDATE users SET totp_secret = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(secret)
        .execute(pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(DbError::UserNotFound);
    }
    Ok(())
}

pub(crate) fn map_unique_violation(e: sqlx::Error) -> DbError {
    if let Some(db_err) = e.as_database_error() {
        if db_err.code().as_deref() == Some("23505") {
            return DbError::Conflict(db_err.message().to_string());
        }
    }
    DbError::Sqlx(e)
}
