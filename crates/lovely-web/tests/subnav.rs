//! Red tests for app sub-nav + collection field editor.

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

// =============================================================
// app sub-nav
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn dashboard_subnav_marks_home_active() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let r = app
        .client
        .get(format!("{}/apps/personal", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        body.contains("class=\"app-subnav\"") || body.contains("app-subnav"),
        "missing sub-nav"
    );
    // Dashboard is the Home tab now (Pages got its own /pages route).
    let home_active = body
        .lines()
        .any(|l| l.contains(">Home<") && l.contains("aria-current=\"page\""));
    assert!(
        home_active,
        "Home tab should have aria-current=page on dashboard"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn pages_route_subnav_marks_pages_active() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let r = app
        .client
        .get(format!("{}/apps/personal/pages", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    let pages_active = body
        .lines()
        .any(|l| l.contains(">Pages<") && l.contains("aria-current=\"page\""));
    assert!(pages_active, "Pages tab should be active on /pages");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn data_subnav_marks_data_active() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let r = app
        .client
        .get(format!("{}/apps/personal/data", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    let data_active = body
        .lines()
        .any(|l| l.contains(">Data<") && l.contains("aria-current=\"page\""));
    assert!(data_active, "Data tab should have aria-current=page");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn settings_subnav_marks_settings_active() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let r = app
        .client
        .get(format!("{}/apps/personal/settings", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    let settings_active = body
        .lines()
        .any(|l| l.contains(">Settings<") && l.contains("aria-current=\"page\""));
    assert!(
        settings_active,
        "Settings tab should have aria-current=page"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn dashboard_drops_legacy_settings_breadcrumb_link() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let r = app
        .client
        .get(format!("{}/apps/personal", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        !body.contains("(settings)"),
        "the muted (settings) breadcrumb link must be gone"
    );
}

// =============================================================
// collection field editor
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn create_collection_with_name_only() {
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
    let loc = r.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        loc.ends_with("/apps/personal/data/posts/edit"),
        "should redirect to field editor, got {loc}"
    );

    // No fields yet — coll exists with empty fields_json.
    let n: (i64,) = sqlx::query_as("SELECT count(*) FROM collections WHERE name = 'posts'")
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(n.0, 1);
    let fields: serde_json::Value =
        sqlx::query_scalar("SELECT fields_json FROM collections WHERE name = 'posts'")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert_eq!(fields, serde_json::json!([]));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn add_field_appends_to_collection() {
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

    // Add `title`, then `body`.
    for f in ["title", "body"] {
        let token = app.csrf_token().await.unwrap();
        let r = app
            .client
            .post(format!("{}/apps/personal/data/posts/fields", app.url))
            .form(&[("name", f), ("_csrf", &token)])
            .send()
            .await
            .unwrap();
        assert!(
            r.status().is_redirection() || r.status() == 200,
            "{}",
            r.status()
        );
    }

    let fields: serde_json::Value =
        sqlx::query_scalar("SELECT fields_json FROM collections WHERE name = 'posts'")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    let arr = fields.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0].get("name").and_then(|v| v.as_str()), Some("title"));
    assert_eq!(arr[1].get("name").and_then(|v| v.as_str()), Some("body"));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn rename_field_migrates_record_data() {
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
    let _ = app
        .client
        .post(format!("{}/apps/personal/data/posts/fields", app.url))
        .form(&[("name", "title"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data/posts/records", app.url))
        .form(&[("title", "hi"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/data/posts/fields/title/rename",
            app.url
        ))
        .form(&[("new_name", "headline"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200);

    let rec: serde_json::Value = sqlx::query_scalar("SELECT data_json FROM records LIMIT 1")
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(rec.get("headline").and_then(|v| v.as_str()), Some("hi"));
    assert!(rec.get("title").is_none());
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn delete_field_clears_record_data() {
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
    let _ = app
        .client
        .post(format!("{}/apps/personal/data/posts/fields", app.url))
        .form(&[("name", "title"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data/posts/records", app.url))
        .form(&[("title", "hi"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/data/posts/fields/title/delete",
            app.url
        ))
        .form(&[("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200);

    let fields: serde_json::Value =
        sqlx::query_scalar("SELECT fields_json FROM collections WHERE name = 'posts'")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert_eq!(fields, serde_json::json!([]));
    let rec: serde_json::Value = sqlx::query_scalar("SELECT data_json FROM records LIMIT 1")
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert!(rec.get("title").is_none(), "field value should be stripped");
}
