//! Inspector exposes a Repeat-per-record section that PATCHes
//! data-lovely-repeat on the selected element.

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

#[tokio::test]
#[ignore = "requires Docker"]
async fn inspector_shows_repeat_section_for_non_leaf_elements_with_collections() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    // Create a collection so the section is meaningful.
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

    // Inspect the home page's root element (a div, non-leaf).
    let root: uuid::Uuid = sqlx::query_scalar(
        "SELECT root_element FROM pages WHERE slug = '' LIMIT 1",
    )
    .fetch_one(&app.pg)
    .await
    .unwrap();

    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/~home/inspector?sel={root}",
            app.url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(
        body.contains("Repeat per record"),
        "non-leaf element inspector should expose the Repeat section: {body}"
    );
    assert!(
        body.contains(r#"name="repeat_collection""#),
        "Repeat form must use the repeat_collection field name: {body}"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn patching_repeat_collection_persists_attribute() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    // Make a posts collection.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data", app.url))
        .form(&[("name", "posts"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();

    let root: uuid::Uuid = sqlx::query_scalar(
        "SELECT root_element FROM pages WHERE slug = '' LIMIT 1",
    )
    .fetch_one(&app.pg)
    .await
    .unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/~home/elements/{root}",
            app.url
        ))
        .form(&[("repeat_collection", "posts"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let attrs: serde_json::Value =
        sqlx::query_scalar("SELECT attrs FROM elements WHERE id = $1")
            .bind(root)
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert_eq!(
        attrs.get("data-lovely-repeat").and_then(|v| v.as_str()),
        Some("posts"),
        "repeat attr should be set: {attrs}"
    );

    // Disconnect: empty repeat_collection drops the attr.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/~home/elements/{root}",
            app.url
        ))
        .form(&[("repeat_collection", ""), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    let attrs: serde_json::Value =
        sqlx::query_scalar("SELECT attrs FROM elements WHERE id = $1")
            .bind(root)
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert!(
        attrs.get("data-lovely-repeat").is_none(),
        "repeat attr should be cleared: {attrs}"
    );
}
