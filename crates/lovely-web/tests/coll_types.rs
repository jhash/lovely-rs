//! Red tests for collection rename + typed fields.

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

async fn make_coll(app: &TestApp, name: &str) {
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data", app.url))
        .form(&[("name", name), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection(), "{}", r.status());
}

// ============================================================
// Rename collection
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn rename_collection_persists() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    make_coll(&app, "posts").await;

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data/posts/rename", app.url))
        .form(&[("new_name", "articles"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection(), "{}", r.status());
    let loc = r.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        loc.contains("/data/articles"),
        "should redirect to renamed collection: {loc}"
    );

    let n: (i64,) = sqlx::query_as("SELECT count(*) FROM collections WHERE name = 'articles'")
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(n.0, 1);
    let old: (i64,) = sqlx::query_as("SELECT count(*) FROM collections WHERE name = 'posts'")
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(old.0, 0);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn rename_collection_rejects_invalid_name() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    make_coll(&app, "posts").await;

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data/posts/rename", app.url))
        .form(&[("new_name", "Bad Name!"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 422);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn collection_view_has_general_edit_button() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    make_coll(&app, "posts").await;

    let r = app
        .client
        .get(format!("{}/apps/personal/data/posts", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    // The button label should be just "Edit", not "Edit fields".
    assert!(
        body.contains(">Edit<"),
        "collection view needs a general 'Edit' button"
    );
    assert!(
        !body.contains(">Edit fields<"),
        "should be 'Edit', not 'Edit fields'"
    );
}

// ============================================================
// Field types
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn add_field_with_type_persists_object_shape() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    make_coll(&app, "posts").await;

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data/posts/fields", app.url))
        .form(&[("name", "views"), ("type", "number"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection(), "{}", r.status());

    let fields: serde_json::Value =
        sqlx::query_scalar("SELECT fields_json FROM collections WHERE name = 'posts'")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    // Stored as a list of objects {name, type}.
    let arr = fields.as_array().expect("array");
    assert_eq!(arr.len(), 1);
    let f = &arr[0];
    assert_eq!(f.get("name").and_then(|v| v.as_str()), Some("views"));
    assert_eq!(f.get("type").and_then(|v| v.as_str()), Some("number"));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn record_form_renders_typed_inputs() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    make_coll(&app, "posts").await;

    for (n, t) in [
        ("title", "text"),
        ("body", "longtext"),
        ("views", "number"),
        ("contact", "email"),
        ("href", "url"),
        ("when", "datetime"),
        ("published_on", "date"),
        ("featured", "bool"),
    ] {
        let token = app.csrf_token().await.unwrap();
        let r = app
            .client
            .post(format!("{}/apps/personal/data/posts/fields", app.url))
            .form(&[("name", n), ("type", t), ("_csrf", &token)])
            .send()
            .await
            .unwrap();
        assert!(r.status().is_redirection(), "add {n}: {}", r.status());
    }

    let r = app
        .client
        .get(format!("{}/apps/personal/data/posts", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(body.contains("name=\"title\""));
    assert!(
        body.contains("type=\"number\"") && body.contains("name=\"views\""),
        "number input missing"
    );
    assert!(
        body.contains("type=\"email\"") && body.contains("name=\"contact\""),
        "email input missing"
    );
    assert!(body.contains("type=\"url\""));
    assert!(body.contains("type=\"date\""));
    assert!(body.contains("type=\"datetime-local\""));
    assert!(body.contains("type=\"checkbox\""));
    assert!(body.contains("<textarea") && body.contains("name=\"body\""));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn legacy_string_fields_migrate_to_text_type() {
    // Existing data may have fields_json stored as ["name", "name"];
    // the new shape is [{name, type}]. Verify that legacy rows still
    // load and are rendered as text inputs.
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    make_coll(&app, "posts").await;

    sqlx::query("UPDATE collections SET fields_json = $1 WHERE name = 'posts'")
        .bind(serde_json::json!(["title", "body"]))
        .execute(&app.pg)
        .await
        .unwrap();

    let r = app
        .client
        .get(format!("{}/apps/personal/data/posts", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(body.contains("name=\"title\""));
    assert!(body.contains("name=\"body\""));
}
