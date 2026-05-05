use crate::errors::DbError;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Field types a collection's columns can take. Mirrors the lovely
/// Swift app's FieldType enum, plus a few HTML-friendly extras.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldType {
    Text,
    LongText,
    Number,
    Email,
    Url,
    Date,
    DateTime,
    Bool,
    Address,
}

impl FieldType {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "text" => Self::Text,
            "longtext" => Self::LongText,
            "number" => Self::Number,
            "email" => Self::Email,
            "url" => Self::Url,
            "date" => Self::Date,
            "datetime" => Self::DateTime,
            "bool" => Self::Bool,
            "address" => Self::Address,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::LongText => "longtext",
            Self::Number => "number",
            Self::Email => "email",
            Self::Url => "url",
            Self::Date => "date",
            Self::DateTime => "datetime",
            Self::Bool => "bool",
            Self::Address => "address",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::LongText => "Long text",
            Self::Number => "Number",
            Self::Email => "Email",
            Self::Url => "URL",
            Self::Date => "Date",
            Self::DateTime => "Date & time",
            Self::Bool => "True / False",
            Self::Address => "Address",
        }
    }

    pub const ALL: &'static [FieldType] = &[
        FieldType::Text,
        FieldType::LongText,
        FieldType::Number,
        FieldType::Email,
        FieldType::Url,
        FieldType::Date,
        FieldType::DateTime,
        FieldType::Bool,
        FieldType::Address,
    ];
}

/// One column in a collection's schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    pub name: String,
    pub field_type: FieldType,
}

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
    /// Decode the schema list, accepting both legacy strings and the
    /// new {name, type} object shape. Unknown types fall back to Text.
    pub fn typed_fields(&self) -> Vec<Field> {
        decode_fields(&self.fields_json)
    }

    /// Legacy accessor — just the field names, in order. Kept so the
    /// renderer/binding logic (which only needs names) doesn't need to
    /// thread types through every layer.
    pub fn fields(&self) -> Vec<String> {
        self.typed_fields().into_iter().map(|f| f.name).collect()
    }
}

pub fn decode_fields(json: &serde_json::Value) -> Vec<Field> {
    let arr = match json.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|v| match v {
            serde_json::Value::String(s) => Some(Field {
                name: s.clone(),
                field_type: FieldType::Text,
            }),
            serde_json::Value::Object(m) => {
                let name = m.get("name").and_then(|v| v.as_str())?.to_string();
                let ft = m
                    .get("type")
                    .and_then(|v| v.as_str())
                    .and_then(FieldType::from_str)
                    .unwrap_or(FieldType::Text);
                Some(Field {
                    name,
                    field_type: ft,
                })
            }
            _ => None,
        })
        .collect()
}

pub fn encode_fields(fields: &[Field]) -> serde_json::Value {
    serde_json::Value::Array(
        fields
            .iter()
            .map(|f| {
                serde_json::json!({
                    "name": f.name,
                    "type": f.field_type.as_str(),
                })
            })
            .collect(),
    )
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
    fields: &[Field],
) -> Result<Collection, DbError> {
    let fields_json = encode_fields(fields);
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

pub async fn rename_collection(
    pool: &PgPool,
    id: Uuid,
    new_name: &str,
) -> Result<Collection, DbError> {
    let row = sqlx::query_as::<_, Collection>(&format!(
        "UPDATE collections SET name = $2, updated_at = now() WHERE id = $1 \
         RETURNING {COLLECTION_COLUMNS}"
    ))
    .bind(id)
    .bind(new_name)
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

/// Replace the ordered list of fields. Does NOT migrate existing
/// records — call this only after handling record migration yourself
/// (see `rename_field` and `delete_field`).
pub async fn set_collection_fields(
    pool: &PgPool,
    id: Uuid,
    fields: &[Field],
) -> Result<(), DbError> {
    let json = encode_fields(fields);
    sqlx::query("UPDATE collections SET fields_json = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(&json)
        .execute(pool)
        .await?;
    Ok(())
}

async fn load_fields(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    coll_id: Uuid,
) -> Result<Vec<Field>, DbError> {
    let json: serde_json::Value =
        sqlx::query_scalar("SELECT fields_json FROM collections WHERE id = $1")
            .bind(coll_id)
            .fetch_one(&mut **tx)
            .await?;
    Ok(decode_fields(&json))
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
    let mut fields = load_fields(&mut tx, coll_id).await?;
    for f in fields.iter_mut() {
        if f.name == old {
            f.name = new.to_string();
        }
    }
    sqlx::query("UPDATE collections SET fields_json = $2, updated_at = now() WHERE id = $1")
        .bind(coll_id)
        .bind(encode_fields(&fields))
        .execute(&mut *tx)
        .await?;
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
    let mut fields = load_fields(&mut tx, coll_id).await?;
    fields.retain(|f| f.name != name);
    sqlx::query("UPDATE collections SET fields_json = $2, updated_at = now() WHERE id = $1")
        .bind(coll_id)
        .bind(encode_fields(&fields))
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
