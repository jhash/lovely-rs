use crate::errors::DbError;
use crate::users::{NewUser, User};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct OAuthIdentity {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub provider_user_id: String,
    pub raw_profile: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct UpsertOAuth {
    pub provider: String,
    pub provider_user_id: String,
    pub raw_profile: serde_json::Value,
    /// Username to use if a new user must be created. Caller must ensure
    /// uniqueness — typically derived from the OAuth profile (e.g. github
    /// login + numeric suffix on collision).
    pub fallback_username: String,
    pub email: Option<String>,
}

/// Idempotently link an OAuth identity to a user. If `(provider, provider_user_id)`
/// already exists, returns the existing user. Otherwise creates a new user
/// and links the identity inside one transaction.
pub async fn upsert_oauth_identity(
    pool: &PgPool,
    upsert: UpsertOAuth,
) -> Result<(User, OAuthIdentity), DbError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    if let Some(ident) = sqlx::query_as::<_, OAuthIdentity>(
        "SELECT id, user_id, provider, provider_user_id, raw_profile, created_at \
         FROM oauth_identities WHERE provider = $1 AND provider_user_id = $2",
    )
    .bind(&upsert.provider)
    .bind(&upsert.provider_user_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, username, email, password_hash, totp_secret, role, created_at, updated_at \
             FROM users WHERE id = $1",
        )
        .bind(ident.user_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok((user, ident));
    }
    // Create new user.
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (username, email)
        VALUES ($1, $2)
        RETURNING id, username, email, password_hash, totp_secret, role, created_at, updated_at
        "#,
    )
    .bind(&upsert.fallback_username)
    .bind(&upsert.email)
    .fetch_one(&mut *tx)
    .await?;
    let _ = NewUser::default(); // keep import alive for symmetry
    let ident = sqlx::query_as::<_, OAuthIdentity>(
        r#"
        INSERT INTO oauth_identities (user_id, provider, provider_user_id, raw_profile)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, provider, provider_user_id, raw_profile, created_at
        "#,
    )
    .bind(user.id)
    .bind(&upsert.provider)
    .bind(&upsert.provider_user_id)
    .bind(&upsert.raw_profile)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((user, ident))
}
