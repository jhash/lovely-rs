//! Schema-change intents for per-app SQLite databases.
//!
//! `Identifier` is the only safe way to put a user-supplied name into a
//! SQL DDL string. It enforces a strict character set, length cap, and a
//! reserved-word denylist at construction time. Once you hold one, you
//! can splat it into a format string without quoting (`"\"{name}\""`)
//! because every byte has been vetted.
//!
//! `Intent` is the serializable record of "what the user asked us to
//! change." A pure renderer turns each `Intent` into `forward_sql` (and
//! when meaningful, `reverse_sql`). Postgres stores the row; SQLite
//! gets the SQL applied during `ensure_migrated`.

use serde::{Deserialize, Serialize};

use crate::errors::DbError;

/// SQLite reserved words we refuse to accept as identifiers. Not an
/// exhaustive list — the regex already rules out punctuation/case — but
/// blocking the common ones turns user typos into a clean error rather
/// than a confusing SQL parse failure mid-migration.
const RESERVED: &[&str] = &[
    "select",
    "from",
    "where",
    "table",
    "index",
    "drop",
    "create",
    "insert",
    "update",
    "delete",
    "alter",
    "rename",
    "column",
    "primary",
    "foreign",
    "key",
    "unique",
    "constraint",
    "join",
    "inner",
    "outer",
    "left",
    "right",
    "on",
    "and",
    "or",
    "not",
    "null",
    "default",
    "values",
    "into",
    "set",
    "begin",
    "commit",
    "rollback",
    "transaction",
    "savepoint",
    "release",
    "with",
    "as",
    "by",
    "order",
    "group",
    "having",
    "limit",
    "offset",
    "case",
    "when",
    "then",
    "else",
    "end",
    "exists",
    "in",
    "between",
    "like",
    "is",
    "all",
    "any",
    "distinct",
    "having",
    "union",
    "intersect",
    "except",
    "view",
    "trigger",
    "if",
];

/// A validated SQL identifier. Construct via [`Identifier::new`].
///
/// Once constructed, the wrapped string is guaranteed to be:
/// - 1..=63 bytes long;
/// - lowercase ASCII letters, digits, and underscores only;
/// - leading character is a letter or underscore;
/// - not one of [`RESERVED`];
/// - not starting with `_lovely` (reserved internal namespace).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Identifier(String);

impl Identifier {
    pub fn new(s: impl Into<String>) -> Result<Self, DbError> {
        let raw: String = s.into();
        if raw.is_empty() || raw.len() > 63 {
            return Err(DbError::InvalidIdentifier(raw));
        }
        let bytes = raw.as_bytes();
        let first_ok = matches!(bytes[0], b'a'..=b'z' | b'_');
        if !first_ok {
            return Err(DbError::InvalidIdentifier(raw));
        }
        for &b in &bytes[1..] {
            if !matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_') {
                return Err(DbError::InvalidIdentifier(raw));
            }
        }
        if RESERVED.contains(&raw.as_str()) {
            return Err(DbError::InvalidIdentifier(raw));
        }
        if raw.starts_with("_lovely") {
            return Err(DbError::InvalidIdentifier(raw));
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Identifier {
    type Error = DbError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<Identifier> for String {
    fn from(id: Identifier) -> Self {
        id.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    Text,
    Integer,
    Real,
    Boolean,
    Blob,
    Datetime,
    Json,
    Uuid,
}

impl ColumnKind {
    /// SQLite column type. Booleans become INTEGER 0/1 by SQLite
    /// convention; UUIDs are stored as TEXT (canonical hyphenated form).
    fn sqlite_type(self) -> &'static str {
        match self {
            ColumnKind::Text | ColumnKind::Json | ColumnKind::Uuid | ColumnKind::Datetime => "TEXT",
            ColumnKind::Integer | ColumnKind::Boolean => "INTEGER",
            ColumnKind::Real => "REAL",
            ColumnKind::Blob => "BLOB",
        }
    }
}

/// Defaults are restricted to forms that survive a SQL render without
/// requiring any user-supplied string to land in a DDL string. The Text
/// variant carries an arbitrary string but the renderer escapes it via
/// SQLite's standard `''` doubling rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DefaultValue {
    Null,
    CurrentTimestamp,
    Integer { value: i64 },
    Boolean { value: bool },
    Text { value: String },
}

impl DefaultValue {
    fn render_sqlite(&self) -> String {
        match self {
            DefaultValue::Null => "NULL".to_string(),
            DefaultValue::CurrentTimestamp => "CURRENT_TIMESTAMP".to_string(),
            DefaultValue::Integer { value } => value.to_string(),
            DefaultValue::Boolean { value } => {
                if *value {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            DefaultValue::Text { value } => format!("'{}'", value.replace('\'', "''")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub name: Identifier,
    pub kind: ColumnKind,
    #[serde(default = "default_true")]
    pub nullable: bool,
    #[serde(default)]
    pub default: Option<DefaultValue>,
}

fn default_true() -> bool {
    true
}

impl ColumnSpec {
    fn render_sqlite(&self) -> String {
        let mut s = format!("\"{}\" {}", self.name, self.kind.sqlite_type());
        if !self.nullable {
            s.push_str(" NOT NULL");
        }
        if let Some(d) = &self.default {
            s.push_str(&format!(" DEFAULT {}", d.render_sqlite()));
        }
        s
    }
}

/// Every user-driven schema change. Stored as JSONB in
/// `app_schema_migrations.intent`; a pure function maps each variant to
/// the SQLite SQL it produces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Intent {
    CreateTable {
        name: Identifier,
        columns: Vec<ColumnSpec>,
    },
    DropTable {
        name: Identifier,
    },
    AddColumn {
        table: Identifier,
        column: ColumnSpec,
    },
    DropColumn {
        table: Identifier,
        column: Identifier,
    },
    RenameColumn {
        table: Identifier,
        from: Identifier,
        to: Identifier,
    },
    AddIndex {
        table: Identifier,
        name: Identifier,
        columns: Vec<Identifier>,
        unique: bool,
    },
    DropIndex {
        name: Identifier,
    },
}

impl Intent {
    /// One-line human description used by audit views.
    pub fn summary(&self) -> String {
        match self {
            Intent::CreateTable { name, columns } => {
                format!("create table `{name}` ({} cols)", columns.len())
            }
            Intent::DropTable { name } => format!("drop table `{name}`"),
            Intent::AddColumn { table, column } => {
                format!("add `{table}.{}` ({:?})", column.name, column.kind)
            }
            Intent::DropColumn { table, column } => {
                format!("drop `{table}.{column}`")
            }
            Intent::RenameColumn { table, from, to } => {
                format!("rename `{table}.{from}` → `{to}`")
            }
            Intent::AddIndex {
                table,
                name,
                unique,
                ..
            } => {
                if *unique {
                    format!("add unique index `{name}` on `{table}`")
                } else {
                    format!("add index `{name}` on `{table}`")
                }
            }
            Intent::DropIndex { name } => format!("drop index `{name}`"),
        }
    }

    /// Render the SQLite DDL pair for this intent.
    ///
    /// `reverse_sql` is `None` when the intent's reverse loses
    /// information (e.g. dropping a table or column — we don't keep
    /// enough context to recreate the data).
    pub fn render_sqlite(&self) -> Result<RenderedDdl, DbError> {
        let (forward, reverse) = match self {
            Intent::CreateTable { name, columns } => {
                if columns.is_empty() {
                    return Err(DbError::SchemaConflict(format!(
                        "create_table {name}: at least one column required"
                    )));
                }
                let cols = columns
                    .iter()
                    .map(ColumnSpec::render_sqlite)
                    .collect::<Vec<_>>()
                    .join(", ");
                (
                    format!("CREATE TABLE \"{name}\" ({cols})"),
                    Some(format!("DROP TABLE \"{name}\"")),
                )
            }
            Intent::DropTable { name } => (format!("DROP TABLE \"{name}\""), None),
            Intent::AddColumn { table, column } => (
                format!(
                    "ALTER TABLE \"{table}\" ADD COLUMN {}",
                    column.render_sqlite()
                ),
                Some(format!(
                    "ALTER TABLE \"{table}\" DROP COLUMN \"{}\"",
                    column.name
                )),
            ),
            Intent::DropColumn { table, column } => (
                format!("ALTER TABLE \"{table}\" DROP COLUMN \"{column}\""),
                None,
            ),
            Intent::RenameColumn { table, from, to } => (
                format!("ALTER TABLE \"{table}\" RENAME COLUMN \"{from}\" TO \"{to}\""),
                Some(format!(
                    "ALTER TABLE \"{table}\" RENAME COLUMN \"{to}\" TO \"{from}\""
                )),
            ),
            Intent::AddIndex {
                table,
                name,
                columns,
                unique,
            } => {
                if columns.is_empty() {
                    return Err(DbError::SchemaConflict(format!(
                        "add_index {name}: at least one column required"
                    )));
                }
                let cols = columns
                    .iter()
                    .map(|c| format!("\"{c}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                let kw = if *unique { "UNIQUE INDEX" } else { "INDEX" };
                (
                    format!("CREATE {kw} \"{name}\" ON \"{table}\" ({cols})"),
                    Some(format!("DROP INDEX \"{name}\"")),
                )
            }
            Intent::DropIndex { name } => (format!("DROP INDEX \"{name}\""), None),
        };
        Ok(RenderedDdl { forward, reverse })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedDdl {
    pub forward: String,
    pub reverse: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(s: &str) -> Identifier {
        Identifier::new(s).expect("test identifier should validate")
    }

    #[test]
    fn identifier_accepts_lowercase_underscore_digits() {
        assert!(Identifier::new("posts").is_ok());
        assert!(Identifier::new("a").is_ok());
        assert!(Identifier::new("_underscore").is_ok());
        assert!(Identifier::new("name_2").is_ok());
        assert!(Identifier::new("a_very_long_but_still_legal_identifier_name_under_63").is_ok());
    }

    #[test]
    fn identifier_rejects_uppercase() {
        assert!(Identifier::new("Posts").is_err());
        assert!(Identifier::new("name_A").is_err());
    }

    #[test]
    fn identifier_rejects_punctuation_and_quotes() {
        for bad in [
            "drop;",
            "name'col",
            "name\"col",
            "table-name",
            "name col",
            "name`col",
            "name/*",
            "name--",
            "x; drop table users",
        ] {
            assert!(Identifier::new(bad).is_err(), "expected reject for {bad:?}");
        }
    }

    #[test]
    fn identifier_rejects_reserved_words() {
        for kw in ["select", "table", "where", "drop", "create"] {
            assert!(Identifier::new(kw).is_err(), "expected reject for {kw}");
        }
    }

    #[test]
    fn identifier_rejects_internal_namespace() {
        assert!(Identifier::new("_lovely").is_err());
        assert!(Identifier::new("_lovely_schema_version").is_err());
    }

    #[test]
    fn identifier_rejects_too_long() {
        let too_long = "a".repeat(64);
        assert!(Identifier::new(too_long).is_err());
        let just_right = "a".repeat(63);
        assert!(Identifier::new(just_right).is_ok());
    }

    #[test]
    fn identifier_rejects_leading_digit() {
        assert!(Identifier::new("1abc").is_err());
    }

    #[test]
    fn identifier_rejects_empty() {
        assert!(Identifier::new("").is_err());
    }

    #[test]
    fn render_create_table() {
        let intent = Intent::CreateTable {
            name: ident("posts"),
            columns: vec![
                ColumnSpec {
                    name: ident("id"),
                    kind: ColumnKind::Uuid,
                    nullable: false,
                    default: None,
                },
                ColumnSpec {
                    name: ident("title"),
                    kind: ColumnKind::Text,
                    nullable: true,
                    default: Some(DefaultValue::Text {
                        value: "untitled".into(),
                    }),
                },
            ],
        };
        let ddl = intent.render_sqlite().unwrap();
        assert_eq!(
            ddl.forward,
            r#"CREATE TABLE "posts" ("id" TEXT NOT NULL, "title" TEXT DEFAULT 'untitled')"#
        );
        assert_eq!(ddl.reverse.as_deref(), Some(r#"DROP TABLE "posts""#));
    }

    #[test]
    fn render_create_table_requires_columns() {
        let intent = Intent::CreateTable {
            name: ident("posts"),
            columns: vec![],
        };
        assert!(intent.render_sqlite().is_err());
    }

    #[test]
    fn render_add_column() {
        let intent = Intent::AddColumn {
            table: ident("posts"),
            column: ColumnSpec {
                name: ident("body"),
                kind: ColumnKind::Text,
                nullable: true,
                default: None,
            },
        };
        let ddl = intent.render_sqlite().unwrap();
        assert_eq!(ddl.forward, r#"ALTER TABLE "posts" ADD COLUMN "body" TEXT"#);
        assert_eq!(
            ddl.reverse.as_deref(),
            Some(r#"ALTER TABLE "posts" DROP COLUMN "body""#)
        );
    }

    #[test]
    fn render_drop_column_no_reverse() {
        let intent = Intent::DropColumn {
            table: ident("posts"),
            column: ident("body"),
        };
        let ddl = intent.render_sqlite().unwrap();
        assert_eq!(ddl.forward, r#"ALTER TABLE "posts" DROP COLUMN "body""#);
        assert!(ddl.reverse.is_none());
    }

    #[test]
    fn render_rename_column_round_trips() {
        let intent = Intent::RenameColumn {
            table: ident("posts"),
            from: ident("title"),
            to: ident("headline"),
        };
        let ddl = intent.render_sqlite().unwrap();
        assert_eq!(
            ddl.forward,
            r#"ALTER TABLE "posts" RENAME COLUMN "title" TO "headline""#
        );
        assert_eq!(
            ddl.reverse.as_deref(),
            Some(r#"ALTER TABLE "posts" RENAME COLUMN "headline" TO "title""#)
        );
    }

    #[test]
    fn render_add_unique_index() {
        let intent = Intent::AddIndex {
            table: ident("posts"),
            name: ident("posts_slug_uq"),
            columns: vec![ident("slug")],
            unique: true,
        };
        let ddl = intent.render_sqlite().unwrap();
        assert_eq!(
            ddl.forward,
            r#"CREATE UNIQUE INDEX "posts_slug_uq" ON "posts" ("slug")"#
        );
        assert_eq!(
            ddl.reverse.as_deref(),
            Some(r#"DROP INDEX "posts_slug_uq""#)
        );
    }

    #[test]
    fn default_text_escapes_single_quotes() {
        let intent = Intent::AddColumn {
            table: ident("posts"),
            column: ColumnSpec {
                name: ident("note"),
                kind: ColumnKind::Text,
                nullable: false,
                default: Some(DefaultValue::Text {
                    value: "it's fine".into(),
                }),
            },
        };
        let ddl = intent.render_sqlite().unwrap();
        // Must produce 'it''s fine' — doubled, not backslash-escaped.
        assert!(
            ddl.forward.contains("DEFAULT 'it''s fine'"),
            "got: {}",
            ddl.forward
        );
    }

    #[test]
    fn intent_round_trips_through_json() {
        let intent = Intent::AddColumn {
            table: ident("posts"),
            column: ColumnSpec {
                name: ident("score"),
                kind: ColumnKind::Integer,
                nullable: false,
                default: Some(DefaultValue::Integer { value: 0 }),
            },
        };
        let s = serde_json::to_string(&intent).unwrap();
        let back: Intent = serde_json::from_str(&s).unwrap();
        assert_eq!(intent, back);
    }
}
