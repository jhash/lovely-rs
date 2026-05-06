//! Disk-backed SqliteAppStore: pool caching, on-disk persistence, and
//! delete_app cleaning up the file.

use std::sync::Arc;

use lovely_db::intent::{ColumnKind, ColumnSpec, Identifier, Intent};
use lovely_db::schema_service::SchemaService;
use lovely_db::{create_app, create_user, LocalSqliteAppStore, NewApp, NewUser, SqliteAppStore};
use lovely_test_support::PgTestContainer;

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

#[tokio::test]
#[ignore = "requires Docker"]
async fn get_pool_creates_file_on_disk_and_applies_pending() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let (user_id, app_id) = fresh_app(&pool).await;
    let schema = Arc::new(SchemaService::new(pool.clone()));
    let dir = tempfile::tempdir().unwrap();
    let store = LocalSqliteAppStore::new(dir.path(), schema.clone()).unwrap();

    schema
        .record(
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

    let p = store.get_pool(app_id).await.unwrap();
    sqlx::query("INSERT INTO posts (id) VALUES ('a')")
        .execute(&p)
        .await
        .unwrap();

    // File exists on disk under the expected name.
    let path = dir.path().join(format!("{app_id}.sqlite"));
    assert!(path.exists(), "expected sqlite file at {path:?}");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn get_pool_caches_and_replays_new_intents() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let (user_id, app_id) = fresh_app(&pool).await;
    let schema = Arc::new(SchemaService::new(pool.clone()));
    let dir = tempfile::tempdir().unwrap();
    let store = LocalSqliteAppStore::new(dir.path(), schema.clone()).unwrap();

    schema
        .record(
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

    let _ = store.get_pool(app_id).await.unwrap();

    // Record a NEW intent after the pool was cached. The next get_pool
    // must catch up.
    schema
        .record(
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

    let p = store.get_pool(app_id).await.unwrap();
    sqlx::query("INSERT INTO posts (id, title) VALUES ('a', 'hi')")
        .execute(&p)
        .await
        .unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM posts WHERE id = 'a'")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(title, "hi");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn delete_app_removes_file() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let (user_id, app_id) = fresh_app(&pool).await;
    let schema = Arc::new(SchemaService::new(pool.clone()));
    let dir = tempfile::tempdir().unwrap();
    let store = LocalSqliteAppStore::new(dir.path(), schema.clone()).unwrap();

    schema
        .record(
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
    let _ = store.get_pool(app_id).await.unwrap();
    let path = dir.path().join(format!("{app_id}.sqlite"));
    assert!(path.exists());

    store.delete_app(app_id).await.unwrap();
    assert!(!path.exists(), "delete_app should remove the file");

    // Calling delete again must be a no-op (NotFound is fine).
    store.delete_app(app_id).await.unwrap();
}
