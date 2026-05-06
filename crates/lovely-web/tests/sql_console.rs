//! Read-only SQL console against the per-app SQLite. Verifies that
//! valid SELECTs return rows, that writes are rejected, and that the
//! UI renders the result table.

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

async fn seed(app: &TestApp) {
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data", app.url))
        .form(&[("name", "posts"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data/posts/fields", app.url))
        .form(&[("name", "title"), ("type", "text"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    for t in ["alpha", "bravo"] {
        let token = app.csrf_token().await.unwrap();
        let _ = app
            .client
            .post(format!("{}/apps/personal/data/posts/records", app.url))
            .form(&[("title", t), ("_csrf", &token)])
            .send()
            .await
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn console_page_renders_for_owner() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let r = app
        .client
        .get(format!("{}/apps/personal/data/console", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(body.contains("SQL console"));
    assert!(body.contains("name=\"sql\""));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn select_returns_table_of_rows() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    seed(&app).await;

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data/console", app.url))
        .form(&[
            ("sql", "SELECT title FROM posts ORDER BY title"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(body.contains("alpha"), "missing alpha: {body}");
    assert!(body.contains("bravo"), "missing bravo");
    // Header should be the column name.
    assert!(body.contains("<th>title</th>"));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn write_statements_are_rejected() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    seed(&app).await;

    for sql in [
        "DELETE FROM posts",
        "UPDATE posts SET title='x'",
        "INSERT INTO posts (id) VALUES ('z')",
        "DROP TABLE posts",
        "SELECT 1; DROP TABLE posts",
    ] {
        let token = app.csrf_token().await.unwrap();
        let r = app
            .client
            .post(format!("{}/apps/personal/data/console", app.url))
            .form(&[("sql", sql), ("_csrf", &token)])
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let body = r.text().await.unwrap();
        assert!(
            body.contains("are allowed") || body.contains("multiple statements"),
            "expected rejection for {sql:?}, body did not contain it"
        );
    }

    // Postgres data unaffected.
    let n: (i64,) = sqlx::query_as("SELECT count(*) FROM records")
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(n.0, 2);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn empty_query_shows_friendly_error() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data/console", app.url))
        .form(&[("sql", "   "), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(body.contains("query is empty"), "body: {body}");
}
