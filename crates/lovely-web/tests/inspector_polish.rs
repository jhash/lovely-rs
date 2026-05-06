//! Polish round: inspector tab visibility, attr rendering parity,
//! drag-drop ordering parity, root vs non-root action symmetry.
//!
//! Several of these were the user's punch list:
//! - Content tab should disappear on non-text elements (no more "Text
//!   content lives on its own #text child" copy).
//! - "Other attributes" section is hidden until we have proper
//!   multi-row + validation UX.
//! - #text elements have no attrs or inline style — hide those tabs.
//! - href edits on `<a>` must reach the rendered HTML.
//! - Drag-drop reorders must show in the canvas + public render.

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

async fn patch_attr(app: &TestApp, id: uuid::Uuid, name: &str, value: &str) {
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/~home/elements/{id}",
            app.url
        ))
        .form(&[
            ("attr_name", name),
            ("attr_value", value),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "patch_attr {name}: {}", r.status());
}

async fn canvas(app: &TestApp) -> String {
    let r = app
        .client
        .get(format!("{}/apps/personal/pages/~home/canvas", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    r.text().await.unwrap()
}

async fn inspector(app: &TestApp, id: uuid::Uuid) -> String {
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

// =============================================================
// inspector tab visibility
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn non_text_element_hides_content_tab_and_explainer() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = home_root(&app).await;
    let div_id = add_child(&app, root, "div").await;

    let body = inspector(&app, div_id).await;
    // No "Content" tab.
    assert!(
        !body.contains(r#"data-tab="content""#),
        "non-text element should not expose a Content tab: {body}"
    );
    // The legacy explainer copy is gone.
    assert!(
        !body.contains("Text content lives on its own"),
        "explanatory copy should be removed: {body}"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn other_attributes_section_is_hidden() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = home_root(&app).await;
    let a_id = add_child(&app, root, "a").await;

    let body = inspector(&app, a_id).await;
    // Tag-attributes section is fine; "Other attributes" goes away
    // pending proper validation + multi-row UI.
    assert!(
        !body.contains("Other attributes"),
        "Other attributes should be hidden for now: {body}"
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

    let body = inspector(&app, text_id).await;
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

// =============================================================
// href rendering
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn href_edits_on_anchor_render_in_canvas() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = home_root(&app).await;
    let a_id = add_child(&app, root, "a").await;

    patch_attr(&app, a_id, "href", "https://example.com").await;

    // DB sanity check.
    let attrs: serde_json::Value = sqlx::query_scalar("SELECT attrs FROM elements WHERE id = $1")
        .bind(a_id)
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(
        attrs.get("href").and_then(|v| v.as_str()),
        Some("https://example.com"),
        "href should be persisted in attrs JSON"
    );

    // Render path must emit it.
    let body = canvas(&app).await;
    assert!(
        body.contains(r#"href="https://example.com""#),
        "canvas should render href: {body}"
    );
}

// =============================================================
// drag-drop ordering
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn move_element_reorders_in_canvas_render() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = home_root(&app).await;

    let h1 = add_child(&app, root, "h1").await;
    let h2 = add_child(&app, root, "h2").await;
    let h3 = add_child(&app, root, "h3").await;

    // Sanity: initial order is h1, h2, h3.
    let body = canvas(&app).await;
    let i1 = body.find("<h1>").expect("h1");
    let i2 = body.find("<h2>").expect("h2");
    let i3 = body.find("<h3>").expect("h3");
    assert!(i1 < i2 && i2 < i3, "initial order should be h1, h2, h3");

    // Drag h3 to be the FIRST child (prev_sibling = null).
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/~home/elements/{h3}/move",
            app.url
        ))
        .form(&[
            ("parent_id", root.to_string().as_str()),
            ("prev_sibling", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "move 1: {}", r.status());

    // Drop h2 between h3 and h1 (prev_sibling = h3) — i.e. final order
    // should be h3, h2, h1.
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/~home/elements/{h2}/move",
            app.url
        ))
        .form(&[
            ("parent_id", root.to_string().as_str()),
            ("prev_sibling", h3.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "move 2: {}", r.status());

    let body = canvas(&app).await;
    let i1 = body.find("<h1>").expect("h1");
    let i2 = body.find("<h2>").expect("h2");
    let i3 = body.find("<h3>").expect("h3");
    assert!(
        i3 < i2 && i2 < i1,
        "after moves, order should be h3, h2, h1; got body=\n{body}"
    );

    // h1 unused locally but kept to mirror the assertion's intent.
    let _ = h1;
}

// =============================================================
// root vs non-root action symmetry
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn root_element_inspector_exposes_add_before_after() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = home_root(&app).await;

    let body = inspector(&app, root).await;
    assert!(
        body.contains("Add before"),
        "root inspector should offer Add before now: {body}"
    );
    assert!(
        body.contains("Add after"),
        "root inspector should offer Add after now"
    );
}
