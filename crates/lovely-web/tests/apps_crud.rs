//! Red tests for app create/list/rename/delete + collections.

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

// ============================================================
// Phase 7a — App CRUD
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn create_app_returns_redirect_and_persists() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps", app.url))
        .form(&[
            ("slug", "blog"),
            ("name", "Blog"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection(), "create app: {}", r.status());

    let n: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM apps a JOIN users u ON u.id = a.owner_id \
         WHERE u.username = 'alice' AND a.slug = 'blog'",
    )
    .fetch_one(&app.pg)
    .await
    .unwrap();
    assert_eq!(n.0, 1);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn cannot_delete_last_app() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/delete", app.url))
        .form(&[("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 422, "should refuse to delete the last app");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn delete_app_removes_pages() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps", app.url))
        .form(&[
            ("slug", "blog"),
            ("name", "Blog"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/blog/delete", app.url))
        .form(&[("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection(), "{}", r.status());

    let n: (i64,) = sqlx::query_as("SELECT count(*) FROM apps WHERE slug = 'blog'")
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(n.0, 0);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn rename_app_updates_slug() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/rename", app.url))
        .form(&[
            ("slug", "main"),
            ("name", "Main"),
            ("description", "Renamed"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection(), "{}", r.status());

    let row: (String, String) =
        sqlx::query_as("SELECT slug, name FROM apps WHERE name = 'Main'")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert_eq!(row.0, "main");
    assert_eq!(row.1, "Main");
}

// ============================================================
// Phase 7b — Collections + records
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn create_collection_persists() {
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
    assert!(
        r.status().is_redirection() || r.status() == 200,
        "create collection: {}",
        r.status()
    );
    for f in ["title", "body"] {
        let token = app.csrf_token().await.unwrap();
        let _ = app
            .client
            .post(format!("{}/apps/personal/data/posts/fields", app.url))
            .form(&[("name", f), ("_csrf", &token)])
            .send()
            .await
            .unwrap();
    }

    let r = app
        .client
        .get(format!("{}/apps/personal/data/posts", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(body.contains("posts"));
    assert!(body.contains("title"));
    assert!(body.contains("body"));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn insert_record_persists_and_renders() {
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
    for f in ["title", "body"] {
        let token = app.csrf_token().await.unwrap();
        let _ = app
            .client
            .post(format!("{}/apps/personal/data/posts/fields", app.url))
            .form(&[("name", f), ("_csrf", &token)])
            .send()
            .await
            .unwrap();
    }

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data/posts/records", app.url))
        .form(&[
            ("title", "Hello"),
            ("body", "World"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_redirection() || r.status() == 200,
        "insert: {}",
        r.status()
    );

    let r = app
        .client
        .get(format!("{}/apps/personal/data/posts", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(body.contains("Hello"), "should list inserted record title");
    assert!(body.contains("World"));
}

// ============================================================
// Phase 7c — bind element to collection
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn bind_element_to_collection_field_renders_value() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    // Create a page, a collection with one row.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", "home2"),
            ("title", "Home"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data", app.url))
        .form(&[("name", "site"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data/site/fields", app.url))
        .form(&[
            ("name", "tagline"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data/site/records", app.url))
        .form(&[("tagline", "Hello from data"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();

    // Find the page's root element + bind it.
    let root: uuid::Uuid = sqlx::query_scalar(
        "SELECT p.root_element FROM pages p JOIN apps a ON a.id = p.app_id \
          JOIN users u ON u.id = a.owner_id \
          WHERE u.username = 'alice' AND p.slug = 'home2'",
    )
    .fetch_one(&app.pg)
    .await
    .unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/home2/elements/{root}",
            app.url
        ))
        .form(&[
            ("binding_collection", "site"),
            ("binding_field", "tagline"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // Publish + render publicly.
    sqlx::query("UPDATE pages SET published_at = now() WHERE slug = 'home2'")
        .execute(&app.pg)
        .await
        .unwrap();
    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/alice/home2", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(
        body.contains("Hello from data"),
        "rendered tree should interpolate bound value: {body}"
    );
}
