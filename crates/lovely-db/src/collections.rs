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

/// Replace the ordered list of field names. Does NOT migrate existing
/// records — call this only after handling record migration yourself
/// (see `rename_field` and `delete_field`).
pub async fn set_collection_fields(
    pool: &PgPool,
    id: Uuid,
    fields: &[String],
) -> Result<(), DbError> {
    let json = serde_json::Value::Array(
        fields
            .iter()
            .map(|s| serde_json::Value::String(s.clone()))
            .collect(),
    );
    sqlx::query("UPDATE collections SET fields_json = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(&json)
        .execute(pool)
        .await?;
    Ok(())
}

/// Renames a field in the collection AND migrates every record's
/// data_json so existing values move from the old key to the new.
pub async fn rename_field(
    pool: &PgPool,
    coll_id: Uuid,
    old: &str,
    new: &str,
) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    // Update the schema list.
    let mut fields: Vec<String> = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT fields_json FROM collections WHERE id = $1",
    )
    .bind(coll_id)
    .fetch_one(&mut *tx)
    .await?
    .as_array()
    .map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    })
    .unwrap_or_default();
    for f in fields.iter_mut() {
        if f == old {
            *f = new.to_string();
        }
    }
    let new_json = serde_json::Value::Array(
        fields
            .into_iter()
            .map(serde_json::Value::String)
            .collect(),
    );
    sqlx::query("UPDATE collections SET fields_json = $2, updated_at = now() WHERE id = $1")
        .bind(coll_id)
        .bind(&new_json)
        .execute(&mut *tx)
        .await?;

    // Migrate records: set data_json - old + new.
    sqlx::query(
        "UPDATE records \
         SET data_json = jsonb_set(data_json - $2::text, ARRAY[$3::text], data_json -> $2::text), \
             updated_at = now() \
         WHERE collection_id = $1 AND data_json ? $2::text",
    )
    .bind(coll_id)
    .bind(old)
    .bind(new)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Removes a field from the collection schema and strips the value
/// from every record's data_json.
pub async fn delete_field(pool: &PgPool, coll_id: Uuid, name: &str) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    let mut fields: Vec<String> = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT fields_json FROM collections WHERE id = $1",
    )
    .bind(coll_id)
    .fetch_one(&mut *tx)
    .await?
    .as_array()
    .map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    })
    .unwrap_or_default();
    fields.retain(|f| f != name);
    let new_json = serde_json::Value::Array(
        fields
            .into_iter()
            .map(serde_json::Value::String)
            .collect(),
    );
    sqlx::query("UPDATE collections SET fields_json = $2, updated_at = now() WHERE id = $1")
        .bind(coll_id)
        .bind(&new_json)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE records SET data_json = data_json - $2::text, updated_at = now() \
         WHERE collection_id = $1 AND data_json ? $2::text",
    )
    .bind(coll_id)
    .bind(name)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
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
