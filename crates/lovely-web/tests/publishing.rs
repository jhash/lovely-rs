//! Red tests for: user public page publish toggle, app publish toggle,
//! owner-can-see-own-unpublished, autosave PATCH debounce, tag-aware
//! attribute fields, page selection in inspector, full-viewport root.

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

async fn make_page(app: &TestApp, page_slug: &str) -> uuid::Uuid {
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", page_slug),
            ("title", "T"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    sqlx::query_scalar(
        r#"SELECT p.root_element FROM pages p
             JOIN apps a ON a.id = p.app_id
             JOIN users u ON u.id = a.owner_id
            WHERE u.username = 'alice' AND p.slug = $1"#,
    )
    .bind(page_slug)
    .fetch_one(&app.pg)
    .await
    .unwrap()
}

// ============================================================
// User public page published flag
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn unpublished_home_page_redirects_anon_to_root() {
    // Each app now has a default Home page (slug = ""). When that home
    // page hasn't been published, anon visits to /alice get bounced to
    // the home route — not 404, per the most recent product call.
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon.get(format!("{}/alice", app.url)).send().await.unwrap();
    assert!(r.status().is_redirection(), "got {}", r.status());
    let loc = r
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(loc, "/");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn published_home_page_renders_for_anon() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    // Publish the home page directly (UI does this via the inspector).
    sqlx::query("UPDATE pages SET published_at = now() WHERE slug = ''")
        .execute(&app.pg)
        .await
        .unwrap();

    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon.get(format!("{}/alice", app.url)).send().await.unwrap();
    assert_eq!(r.status(), 200, "published home should render for anon");
}

// ============================================================
// Owner can view own unpublished pages
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn owner_can_view_own_unpublished_page() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let _ = make_page(&app, "draft").await;
    // page is NOT published.

    // Owner has session cookies in app.client; should see 200.
    let r = app
        .client
        .get(format!("{}/alice/draft", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "owner should see own draft");
}

// ============================================================
// Autosave PATCH (no Save button)
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn inspector_form_uses_hx_trigger_change() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    // Make a #text element so the content tab shows the textarea.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", "as"),
            ("title", "T"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let root: uuid::Uuid = sqlx::query_scalar("SELECT root_element FROM pages WHERE slug = 'as'")
        .fetch_one(&app.pg)
        .await
        .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/as/elements", app.url))
        .form(&[
            ("tag", "#text"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let txt_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM elements WHERE tag = '#text' LIMIT 1")
            .fetch_one(&app.pg)
            .await
            .unwrap();

    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/as/inspector?sel={txt_id}",
            app.url
        ))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        body.contains("hx-trigger=\"input changed delay:400ms, change\"")
            || body.contains("hx-trigger=\"input changed delay:400ms\""),
        "content textarea should autosave on input/change: {body}"
    );
    // Text content should auto-save — no explicit "Save" button anywhere
    // in the inspector. Delete / Add buttons are fine; they're separate
    // actions, not save-on-submit.
    assert!(
        !body.contains(">Save<") && !body.contains("\"Save\""),
        "autosave inspector should not have a Save button: {body}"
    );
}

// ============================================================
// Tag-aware attribute fields
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn anchor_inspector_shows_href_field() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = make_page(&app, "lk").await;

    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/lk/elements", app.url))
        .form(&[
            ("tag", "a"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let a: uuid::Uuid = sqlx::query_scalar("SELECT id FROM elements WHERE tag = 'a' LIMIT 1")
        .fetch_one(&app.pg)
        .await
        .unwrap();

    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/lk/inspector?sel={a}&tab=attrs",
            app.url
        ))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        body.contains("data-attr=\"href\"") || body.contains("name=\"attr_href\""),
        "anchor inspector should expose a dedicated href field: {body}"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn img_inspector_shows_src_and_alt() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = make_page(&app, "im").await;

    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/im/elements", app.url))
        .form(&[
            ("tag", "img"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let img: uuid::Uuid = sqlx::query_scalar("SELECT id FROM elements WHERE tag = 'img' LIMIT 1")
        .fetch_one(&app.pg)
        .await
        .unwrap();

    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/im/inspector?sel={img}&tab=attrs",
            app.url
        ))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(body.contains("data-attr=\"src\"") || body.contains("name=\"attr_src\""));
    assert!(body.contains("data-attr=\"alt\"") || body.contains("name=\"attr_alt\""));
}

// ============================================================
// Page selection
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn page_selection_inspector_shows_page_settings() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let _ = make_page(&app, "ps").await;

    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/ps/inspector?sel=page",
            app.url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    // Page-level settings: title, head html, password.
    assert!(body.contains("name=\"title\""));
    assert!(body.contains("name=\"head_html\""));
    assert!(body.contains("name=\"password\""));
}
