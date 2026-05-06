//! After creating an element via htmx, the server should ask the editor
//! to select the new element. For #text additions, the JS receives
//! `focus=text` so the textarea grabs focus.

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

async fn make_page(app: &TestApp, slug: &str) -> uuid::Uuid {
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", slug),
            ("title", "T"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    sqlx::query_scalar("SELECT root_element FROM pages WHERE slug = $1")
        .bind(slug)
        .fetch_one(&app.pg)
        .await
        .unwrap()
}

fn assert_select_trigger(trigger: &str, expected_focus: &str) -> String {
    // Should be JSON like `{"lovely:select":{"id":"...","focus":"text"}}`.
    // `preview-stale` is intentionally omitted: the JS lovely:select
    // handler updates the asides' hx-get URLs and reloads the iframe
    // itself, so emitting preview-stale here would let the asides'
    // initial-render hx-get fetch with the OLD ?sel= and race the JS
    // swap to a stale inspector.
    let v: serde_json::Value =
        serde_json::from_str(trigger).unwrap_or_else(|_| panic!("HX-Trigger not JSON: {trigger}"));
    assert!(
        v.get("preview-stale").is_none(),
        "select trigger must NOT include preview-stale (would race JS swap): {trigger}"
    );
    let sel = v
        .get("lovely:select")
        .unwrap_or_else(|| panic!("trigger missing lovely:select: {trigger}"));
    let focus = sel.get("focus").and_then(|f| f.as_str()).unwrap_or("");
    assert_eq!(
        focus, expected_focus,
        "expected focus={expected_focus}, got {focus}"
    );
    sel.get("id")
        .and_then(|i| i.as_str())
        .expect("trigger missing id")
        .to_string()
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn add_text_node_signals_select_with_text_focus() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = make_page(&app, "tx").await;

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/tx/elements", app.url))
        .header("HX-Request", "true")
        .form(&[
            ("tag", "#text"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let trigger = r
        .headers()
        .get("HX-Trigger")
        .map(|v| v.to_str().unwrap().to_string())
        .expect("response missing HX-Trigger");
    let new_id = assert_select_trigger(&trigger, "text");
    // The id must be a real, freshly-inserted #text element.
    let tag: String = sqlx::query_scalar("SELECT tag FROM elements WHERE id = $1::uuid")
        .bind(&new_id)
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(tag, "#text");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn add_div_signals_select_without_focus() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = make_page(&app, "dv").await;

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/dv/elements", app.url))
        .header("HX-Request", "true")
        .form(&[
            ("tag", "div"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let trigger = r
        .headers()
        .get("HX-Trigger")
        .map(|v| v.to_str().unwrap().to_string())
        .expect("response missing HX-Trigger");
    let new_id = assert_select_trigger(&trigger, "");
    let tag: String = sqlx::query_scalar("SELECT tag FROM elements WHERE id = $1::uuid")
        .bind(&new_id)
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(tag, "div");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn add_after_signals_select() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = make_page(&app, "af").await;

    // First add a sibling under root (so we have a target).
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/af/elements", app.url))
        .header("HX-Request", "true")
        .form(&[
            ("tag", "div"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let target_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM elements WHERE parent_id = $1 LIMIT 1")
            .bind(root)
            .fetch_one(&app.pg)
            .await
            .unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/af/elements/{target_id}/add-after",
            app.url
        ))
        .header("HX-Request", "true")
        .form(&[("tag", "p"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let trigger = r
        .headers()
        .get("HX-Trigger")
        .map(|v| v.to_str().unwrap().to_string())
        .expect("response missing HX-Trigger");
    assert_select_trigger(&trigger, "");
}
