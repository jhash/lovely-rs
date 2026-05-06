//! Collection + field mutations push intents into the per-app SQLite.
//!
//! These tests exercise the dual-write path: every collection / field
//! handler should leave the SQLite database with structure that mirrors
//! the Postgres truth.

use lovely_test_support::{PgTestContainer, TestApp};

async fn register(app: &TestApp, username: &str) -> anyhow::Result<()> {
    let token = app.csrf_token().await?;
    let r = app
        .client
        .post(format!("{}/auth/register", app.url))
        .form(&[
            ("username", username),
            ("password", "correct horse battery staple"),
            ("_csrf", &token),
        ])
        .send()
        .await?;
    assert!(r.status().is_redirection(), "register: {}", r.status());
    Ok(())
}

async fn app_id(app: &TestApp, username: &str, slug: &str) -> uuid::Uuid {
    sqlx::query_scalar(
        r#"SELECT a.id FROM apps a JOIN users u ON u.id = a.owner_id
            WHERE u.username = $1 AND a.slug = $2"#,
    )
    .bind(username)
    .bind(slug)
    .fetch_one(&app.pg)
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn create_collection_records_create_table_intent() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data", app.url))
        .form(&[("name", "posts"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection(), "{}", r.status());

    let app_uuid = app_id(&app, "alice", "personal").await;
    let intents: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT intent FROM app_schema_migrations WHERE app_id = $1 ORDER BY version",
    )
    .bind(app_uuid)
    .fetch_all(&app.pg)
    .await
    .unwrap();
    assert_eq!(intents.len(), 1, "should be exactly one intent recorded");
    assert_eq!(intents[0]["op"].as_str(), Some("create_table"));
    assert_eq!(intents[0]["name"].as_str(), Some("posts"));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn add_field_records_add_column_intent() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data", app.url))
        .form(&[("name", "posts"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data/posts/fields", app.url))
        .form(&[("name", "title"), ("type", "text"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200);

    let app_uuid = app_id(&app, "alice", "personal").await;
    let intents: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT intent FROM app_schema_migrations WHERE app_id = $1 ORDER BY version",
    )
    .bind(app_uuid)
    .fetch_all(&app.pg)
    .await
    .unwrap();
    assert_eq!(intents.len(), 2);
    assert_eq!(intents[1]["op"].as_str(), Some("add_column"));
    assert_eq!(intents[1]["table"].as_str(), Some("posts"));
    assert_eq!(intents[1]["column"]["name"].as_str(), Some("title"));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn collection_with_invalid_name_is_rejected() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    // Reserved word — must reject before touching Postgres.
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data", app.url))
        .form(&[("name", "select"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        422,
        "reserved word should be unprocessable: {}",
        r.status()
    );

    let n: (i64,) = sqlx::query_as("SELECT count(*) FROM collections")
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(n.0, 0, "no collection should have been created");
}
