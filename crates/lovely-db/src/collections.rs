use crate::errors::DbError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Collection {
    pub id: Uuid,
    pub app_id: Uuid,
    pub name: String,
    pub fields_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Collection {
    pub fn fields(&self) -> Vec<String> {
        self.fields_json
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Record {
    pub id: Uuid,
    pub collection_id: Uuid,
    pub data_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const COLLECTION_COLUMNS: &str =
    "id, app_id, name, fields_json, created_at, updated_at";
const RECORD_COLUMNS: &str = "id, collection_id, data_json, created_at, updated_at";

pub async fn create_collection(
    pool: &PgPool,
    app_id: Uuid,
    name: &str,
    fields: &[String],
) -> Result<Collection, DbError> {
    let fields_json = serde_json::Value::Array(
        fields
            .iter()
            .map(|s| serde_json::Value::String(s.clone()))
            .collect(),
    );
    let row = sqlx::query_as::<_, Collection>(&format!(
        "INSERT INTO collections (app_id, name, fields_json) \
         VALUES ($1, $2, $3) RETURNING {COLLECTION_COLUMNS}"
    ))
    .bind(app_id)
    .bind(name)
    .bind(&fields_json)
    .fetch_one(pool)
    .await
    .map_err(crate::users::map_unique_violation)?;
    Ok(row)
}

pub async fn list_collections(pool: &PgPool, app_id: Uuid) -> Result<Vec<Collection>, DbError> {
    let rows = sqlx::query_as::<_, Collection>(&format!(
        "SELECT {COLLECTION_COLUMNS} FROM collections WHERE app_id = $1 ORDER BY name ASC"
    ))
    .bind(app_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_collection_by_name(
    pool: &PgPool,
    app_id: Uuid,
    name: &str,
) -> Result<Option<Collection>, DbError> {
    let row = sqlx::query_as::<_, Collection>(&format!(
        "SELECT {COLLECTION_COLUMNS} FROM collections WHERE app_id = $1 AND name = $2"
    ))
    .bind(app_id)
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_collection(pool: &PgPool, id: Uuid) -> Result<u64, DbError> {
    let n = sqlx::query("DELETE FROM collections WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(n)
}

pub async fn insert_record(
    pool: &PgPool,
    collection_id: Uuid,
    data: serde_json::Value,
) -> Result<Record, DbError> {
    let row = sqlx::query_as::<_, Record>(&format!(
        "INSERT INTO records (collection_id, data_json) VALUES ($1, $2) \
         RETURNING {RECORD_COLUMNS}"
    ))
    .bind(collection_id)
    .bind(&data)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn list_records(pool: &PgPool, collection_id: Uuid) -> Result<Vec<Record>, DbError> {
    let rows = sqlx::query_as::<_, Record>(&format!(
        "SELECT {RECORD_COLUMNS} FROM records WHERE collection_id = $1 ORDER BY created_at ASC"
    ))
    .bind(collection_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn delete_record(pool: &PgPool, id: Uuid) -> Result<u64, DbError> {
    let n = sqlx::query("DELETE FROM records WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(n)
}
