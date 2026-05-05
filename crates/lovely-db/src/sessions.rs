use crate::errors::DbError;
use chrono::{DateTime, Utc};
use rand::RngCore;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: Uuid,
    pub csrf_token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub user_agent: Option<String>,
    pub ip: Option<sqlx::types::ipnetwork::IpNetwork>,
}

#[derive(Clone, Debug)]
pub struct NewSession {
    pub user_id: Uuid,
    pub ttl: chrono::Duration,
    pub user_agent: Option<String>,
}

pub async fn create_session(pool: &PgPool, new: NewSession) -> Result<Session, DbError> {
    let id = random_token(32);
    let csrf = random_token(32);
    let expires_at = Utc::now() + new.ttl;
    let row = sqlx::query_as::<_, Session>(
        r#"
        INSERT INTO sessions (id, user_id, csrf_token, expires_at, user_agent)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, user_id, csrf_token, expires_at, created_at, user_agent, ip
        "#,
    )
    .bind(&id)
    .bind(new.user_id)
    .bind(&csrf)
    .bind(expires_at)
    .bind(&new.user_agent)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn find_session(pool: &PgPool, id: &str) -> Result<Option<Session>, DbError> {
    let row = sqlx::query_as::<_, Session>(
        "SELECT id, user_id, csrf_token, expires_at, created_at, user_agent, ip \
         FROM sessions WHERE id = $1 AND expires_at > now()",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_session(pool: &PgPool, id: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_all_sessions_for_user(pool: &PgPool, user_id: Uuid) -> Result<u64, DbError> {
    let n = sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(n)
}

pub async fn purge_expired_sessions(pool: &PgPool) -> Result<u64, DbError> {
    let n = sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(pool)
        .await?
        .rows_affected();
    Ok(n)
}

/// Generate a random hex token of `byte_len` bytes (output is 2*byte_len chars).
fn random_token(byte_len: usize) -> String {
    let mut bytes = vec![0u8; byte_len];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut out = String::with_capacity(byte_len * 2);
    for b in bytes {
        out.push(hex_nibble(b >> 4));
        out.push(hex_nibble(b & 0x0F));
    }
    out
}

fn hex_nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => unreachable!(),
    }
}
