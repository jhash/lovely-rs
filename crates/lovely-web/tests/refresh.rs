//! Red tests for the round of UX cleanup: full-width nav on builder,
//! tag-changing in the inspector, /settings split, /pages + /data
//! parity, tree selection visibility, first-class #text node UX.

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
// builder full-width nav
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn builder_keeps_top_nav() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let _ = make_page(&app, "fwn").await;

    let r = app
        .client
        .get(format!("{}/apps/personal/pages/fwn/edit", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(body.contains("class=\"builder\""));
    assert!(
        body.contains("nav") && body.contains("top-nav"),
        "builder must include the global top-nav: missing"
    );
    // The container should NOT be max-width-clamped on the builder.
    assert!(
        body.contains("top-nav-fullwidth")
            || body.contains("class=\"container fullwidth\"")
            || body.contains("class=\"top-nav fullwidth\""),
        "builder nav must opt out of the centered .container clamp"
    );
}

// ============================================================
// inspector tag picker
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn inspector_can_change_tag() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = make_page(&app, "tags").await;

    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/tags/inspector?sel={root}",
            app.url
        ))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        body.contains("name=\"tag\""),
        "inspector should expose a tag picker"
    );

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/tags/elements/{root}",
            app.url
        ))
        .form(&[("tag", "section"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let tag: String = sqlx::query_scalar("SELECT tag FROM elements WHERE id = $1")
        .bind(root)
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(tag, "section");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn change_to_text_preserves_text() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = make_page(&app, "ttext").await;

    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/ttext/elements", app.url))
        .form(&[
            ("tag", "p"),
            ("text", "hello"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let p: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM elements WHERE parent_id = $1 LIMIT 1")
            .bind(root)
            .fetch_one(&app.pg)
            .await
            .unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/ttext/elements/{p}",
            app.url
        ))
        .form(&[("tag", "#text"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let (tag, text): (String, Option<String>) = sqlx::query_as(
        "SELECT tag, payload->>'text' FROM elements WHERE id = $1",
    )
    .bind(p)
    .fetch_one(&app.pg)
    .await
    .unwrap();
    assert_eq!(tag, "#text");
    assert_eq!(text.as_deref(), Some("hello"));
}

// ============================================================
// /settings page split
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn settings_page_holds_rename_and_theme() {
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
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(body.contains("/apps/personal/rename"), "rename form on settings");
    assert!(body.contains("/apps/personal/theme"), "theme form on settings");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn dashboard_does_not_show_theme_or_rename() {
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
    assert!(!body.contains("/apps/personal/rename"), "rename should be on settings");
    assert!(
        !body.to_lowercase().contains("theme")
            || body.contains("/apps/personal/settings"),
        "theme heading should be on settings, not dashboard"
    );
}

// ============================================================
// /pages + /data parity
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn dashboard_shows_pages_and_data_summaries() {
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
    // The h2 anchors must point at index pages.
    assert!(
        body.contains("href=\"/apps/personal/pages\""),
        "Pages section header must link to /apps/personal/pages"
    );
    assert!(
        body.contains("href=\"/apps/personal/data\""),
        "Data section header must link to /apps/personal/data"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn pages_index_lists_pages() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let _ = make_page(&app, "indexed").await;

    let r = app
        .client
        .get(format!("{}/apps/personal/pages", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(body.contains("indexed"));
}

// ============================================================
// first-class #text — no implicit text field on add forms
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn text_child_in_div_renders_inline() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = make_page(&app, "tnode").await;
    sqlx::query("UPDATE pages SET published_at = now() WHERE slug = 'tnode'")
        .execute(&app.pg)
        .await
        .unwrap();

    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/tnode/elements", app.url))
        .form(&[
            ("tag", "#text"),
            ("text", "hello world"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();

    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/alice/tnode", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        body.contains("<div>hello world</div>") || body.contains(">hello world<"),
        "inline text should sit inside the div without a wrapper: {body}"
    );
    assert!(
        !body.contains("<#text"),
        "no literal #text tag should appear in output"
    );
}
