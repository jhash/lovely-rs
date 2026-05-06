//! Inspector tab visibility per element type.
//!
//! `#text` carries text and nothing else, so its inspector shows only
//! the Content tab. Every other element gets Attributes + Style +
//! (when the app has collections) Data, but NOT Content — text lives
//! on `#text` children.

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

async fn home_root(app: &TestApp) -> uuid::Uuid {
    sqlx::query_scalar(
        r#"SELECT p.root_element FROM pages p
             JOIN apps a ON a.id = p.app_id
             JOIN users u ON u.id = a.owner_id
            WHERE u.username = 'alice' AND p.slug = ''"#,
    )
    .fetch_one(&app.pg)
    .await
    .unwrap()
}

async fn add_child(app: &TestApp, parent: uuid::Uuid, tag: &str) -> uuid::Uuid {
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/~home/elements", app.url))
        .header("HX-Request", "true")
        .form(&[
            ("tag", tag),
            ("parent_id", parent.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "add_child {tag}: {}", r.status());
    sqlx::query_scalar(
        "SELECT id FROM elements WHERE parent_id = $1 AND tag = $2 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(parent)
    .bind(tag)
    .fetch_one(&app.pg)
    .await
    .unwrap()
}

async fn inspector_html(app: &TestApp, id: uuid::Uuid) -> String {
    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/~home/inspector?sel={id}",
            app.url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    r.text().await.unwrap()
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn non_text_element_has_no_content_tab() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = home_root(&app).await;
    let div_id = add_child(&app, root, "div").await;

    let body = inspector_html(&app, div_id).await;
    assert!(
        !body.contains(r#"data-tab="content""#),
        "non-text element should not expose a Content tab: {body}"
    );
    // Legacy explainer copy must not return — the Content tab itself
    // is gone, so the copy that lived inside it has no home.
    assert!(
        !body.contains("Text content lives on its own"),
        "explainer copy should be retired: {body}"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn text_element_hides_attributes_and_style_tabs() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = home_root(&app).await;
    let text_id = add_child(&app, root, "#text").await;

    let body = inspector_html(&app, text_id).await;
    assert!(
        !body.contains(r#"data-tab="attrs""#),
        "#text should not show Attributes tab: {body}"
    );
    assert!(
        !body.contains(r#"data-tab="style""#),
        "#text should not show Style tab: {body}"
    );
    assert!(
        body.contains(r#"data-tab="content""#),
        "#text should still expose Content tab"
    );
}
