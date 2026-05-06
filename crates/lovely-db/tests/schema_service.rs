//! Round-trip the intent log into a real per-app SQLite database.
//!
//! These tests boot a Postgres container, create users + apps, then
//! drive `SchemaService` through the same paths that `lovely-web` will
//! use at runtime.

use lovely_db::intent::{ColumnKind, ColumnSpec, DefaultValue, Identifier, Intent};
use lovely_db::schema_service::SchemaService;
use lovely_db::{create_app, create_user, NewApp, NewUser};
use lovely_test_support::PgTestContainer;
use sqlx::SqlitePool;

fn ident(s: &str) -> Identifier {
    Identifier::new(s).unwrap()
}

async fn fresh_app(pool: &sqlx::PgPool) -> (uuid::Uuid, uuid::Uuid) {
    let user = create_user(
        pool,
        NewUser {
            username: "alice".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let app = create_app(
        pool,
        NewApp {
            slug: "personal".into(),
            name: "Personal".into(),
            description: None,
            owner_id: user.id,
            is_default: true,
        },
    )
    .await
    .unwrap();
    (user.id, app.id)
}

async fn fresh_sqlite() -> SqlitePool {
    SqlitePool::connect("sqlite::memory:").await.unwrap()
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn record_then_ensure_creates_table_in_sqlite() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let (user_id, app_id) = fresh_app(&pool).await;
    let svc = SchemaService::new(pool.clone());

    let v = svc
        .record(
            app_id,
            user_id,
            Intent::CreateTable {
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
                        default: None,
                    },
                ],
            },
        )
        .await
        .unwrap();
    assert_eq!(v, 1, "first migration is version 1");

    let sqlite = fresh_sqlite().await;
    svc.ensure_migrated(app_id, &sqlite).await.unwrap();

    // Insert a row and read it back — proves the table exists with the
    // right columns.
    sqlx::query("INSERT INTO posts (id, title) VALUES (?, ?)")
        .bind("11111111-1111-1111-1111-111111111111")
        .bind("hello")
        .execute(&sqlite)
        .await
        .unwrap();
    let title: String =
        sqlx::query_scalar("SELECT title FROM posts WHERE title = 'hello'")
            .fetch_one(&sqlite)
            .await
            .unwrap();
    assert_eq!(title, "hello");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn ensure_is_idempotent() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let (user_id, app_id) = fresh_app(&pool).await;
    let svc = SchemaService::new(pool.clone());

    svc.record(
        app_id,
        user_id,
        Intent::CreateTable {
            name: ident("posts"),
            columns: vec![ColumnSpec {
                name: ident("id"),
                kind: ColumnKind::Uuid,
                nullable: false,
                default: None,
            }],
        },
    )
    .await
    .unwrap();

    let sqlite = fresh_sqlite().await;
    svc.ensure_migrated(app_id, &sqlite).await.unwrap();
    // Second call must be a no-op (would error if it tried to recreate).
    svc.ensure_migrated(app_id, &sqlite).await.unwrap();
    svc.ensure_migrated(app_id, &sqlite).await.unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn add_column_then_drop_column_round_trip() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let (user_id, app_id) = fresh_app(&pool).await;
    let svc = SchemaService::new(pool.clone());

    svc.record(
        app_id,
        user_id,
        Intent::CreateTable {
            name: ident("posts"),
            columns: vec![ColumnSpec {
                name: ident("id"),
                kind: ColumnKind::Uuid,
                nullable: false,
                default: None,
            }],
        },
    )
    .await
    .unwrap();
    svc.record(
        app_id,
        user_id,
        Intent::AddColumn {
            table: ident("posts"),
            column: ColumnSpec {
                name: ident("body"),
                kind: ColumnKind::Text,
                nullable: true,
                default: Some(DefaultValue::Text {
                    value: "draft".into(),
                }),
            },
        },
    )
    .await
    .unwrap();
    let v3 = svc
        .record(
            app_id,
            user_id,
            Intent::DropColumn {
                table: ident("posts"),
                column: ident("body"),
            },
        )
        .await
        .unwrap();
    assert_eq!(v3, 3, "versions must be sequential");

    let sqlite = fresh_sqlite().await;
    svc.ensure_migrated(app_id, &sqlite).await.unwrap();

    // The body column should be gone — selecting it must error.
    let r = sqlx::query("SELECT body FROM posts").fetch_all(&sqlite).await;
    assert!(r.is_err(), "body column should have been dropped");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn record_for_unknown_app_errors() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let svc = SchemaService::new(pool.clone());
    let bogus_app = uuid::Uuid::new_v4();
    let bogus_user = uuid::Uuid::new_v4();
    let err = svc
        .record(
            bogus_app,
            bogus_user,
            Intent::CreateTable {
                name: ident("posts"),
                columns: vec![ColumnSpec {
                    name: ident("id"),
                    kind: ColumnKind::Uuid,
                    nullable: false,
                    default: None,
                }],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, lovely_db::DbError::AppNotFound(_)));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn second_call_on_fresh_sqlite_replays_full_log() {
    // Simulates losing the SQLite file: open a new pool and call
    // ensure_migrated — every recorded intent must be replayed.
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let (user_id, app_id) = fresh_app(&pool).await;
    let svc = SchemaService::new(pool.clone());

    svc.record(
        app_id,
        user_id,
        Intent::CreateTable {
            name: ident("posts"),
            columns: vec![ColumnSpec {
                name: ident("id"),
                kind: ColumnKind::Uuid,
                nullable: false,
                default: None,
            }],
        },
    )
    .await
    .unwrap();
    svc.record(
        app_id,
        user_id,
        Intent::AddColumn {
            table: ident("posts"),
            column: ColumnSpec {
                name: ident("title"),
                kind: ColumnKind::Text,
                nullable: true,
                default: None,
            },
        },
    )
    .await
    .unwrap();

    let sqlite_a = fresh_sqlite().await;
    svc.ensure_migrated(app_id, &sqlite_a).await.unwrap();
    sqlx::query("INSERT INTO posts (id, title) VALUES ('x', 'a')")
        .execute(&sqlite_a)
        .await
        .unwrap();

    // Brand new pool: must catch up to v=2 from scratch.
    let sqlite_b = fresh_sqlite().await;
    svc.ensure_migrated(app_id, &sqlite_b).await.unwrap();
    sqlx::query("INSERT INTO posts (id, title) VALUES ('y', 'b')")
        .execute(&sqlite_b)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM posts")
        .fetch_one(&sqlite_b)
        .await
        .unwrap();
    assert_eq!(n, 1, "fresh pool starts empty even though Pg has the log");
}
