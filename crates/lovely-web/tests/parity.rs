//! Red-then-green tests for the remaining parity phases:
//! 9 — repeaters, 10 — undo/redo, 11 — theme vars, 12 — page metadata,
//! 13 — page-level password / unlisted.

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

/// Creates a published page named `home` with an explicit slug so the
/// public route is `/{user}/home`.
async fn make_page(app: &TestApp, page_slug: &str) -> uuid::Uuid {
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", page_slug),
            ("title", "Page"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection(), "{}", r.status());
    sqlx::query("UPDATE pages SET published_at = now() WHERE slug = $1")
        .bind(page_slug)
        .execute(&app.pg)
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

// =============================================================
// Phase 9 — repeaters / iterators
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase9_repeat_renders_one_child_per_record() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = make_page(&app, "feed").await;

    // Create a `posts` collection with 3 records.
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
    for t in ["one", "two", "three"] {
        let token = app.csrf_token().await.unwrap();
        let _ = app
            .client
            .post(format!("{}/apps/personal/data/posts/records", app.url))
            .form(&[("title", t), ("_csrf", &token)])
            .send()
            .await
            .unwrap();
    }

    // Add a child <li> to the root with text "{{title}}".
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/feed/elements", app.url))
        .form(&[
            ("tag", "li"),
            ("text", "{{title}}"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();

    // Bind the root to repeat over `posts`.
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/feed/elements/{root}",
            app.url
        ))
        .form(&[
            ("repeat_collection", "posts"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "PATCH should accept repeat_collection");

    // Render publicly, expect three repeated entries.
    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon.get(format!("{}/alice/feed", app.url)).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    for t in ["one", "two", "three"] {
        assert!(body.contains(t), "rendered should contain {t}: {body}");
    }
    let li_count = body.matches("<li").count();
    assert!(li_count >= 3, "expected 3+ <li>, got {li_count}");
}

// =============================================================
// Phase 10 — undo/redo
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase10_undo_text_change() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = make_page(&app, "history").await;

    // First text mutation.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/history/elements/{root}",
            app.url
        ))
        .form(&[("text", "first"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();

    // Second text mutation.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/history/elements/{root}",
            app.url
        ))
        .form(&[("text", "second"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();

    // Undo brings us back to "first".
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/history/undo", app.url))
        .form(&[("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status() == 200 || r.status() == 303, "{}", r.status());

    let txt: Option<String> =
        sqlx::query_scalar("SELECT (payload->>'text') FROM elements WHERE id = $1")
            .bind(root)
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert_eq!(txt.as_deref(), Some("first"));

    // Redo goes back to "second".
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/history/redo", app.url))
        .form(&[("_csrf", &token)])
        .send()
        .await
        .unwrap();

    let txt: Option<String> =
        sqlx::query_scalar("SELECT (payload->>'text') FROM elements WHERE id = $1")
            .bind(root)
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert_eq!(txt.as_deref(), Some("second"));
}

// =============================================================
// Phase 11 — theme variables
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase11_theme_vars_inject_into_public_render() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let _ = make_page(&app, "themed").await;

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/theme", app.url))
        .form(&[
            ("primary", "#ff0066"),
            ("background", "#101010"),
            ("ink", "#fafafa"),
            ("font", "Lora"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200, "{}", r.status());

    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/alice/themed", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        body.contains("#ff0066") && body.contains("--lovely-primary"),
        "theme vars not injected: {body}"
    );
}

// =============================================================
// Phase 12 — page metadata + custom head
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase12_custom_head_html_injected() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let _ = make_page(&app, "metas").await;

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/metas/head",
            app.url
        ))
        .form(&[
            (
                "head_html",
                "<meta property=\"og:image\" content=\"/x.png\">",
            ),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200, "{}", r.status());

    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/alice/metas", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(body.contains("og:image"), "custom head not injected: {body}");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase12_head_html_strips_scripts() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let _ = make_page(&app, "scripted").await;

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/scripted/head",
            app.url
        ))
        .form(&[
            ("head_html", "<script>alert(1)</script><meta name=\"a\" content=\"b\">"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200);

    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/alice/scripted", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(!body.contains("<script>"), "scripts must be stripped: {body}");
    assert!(body.contains("name=\"a\""), "safe meta should pass through");
}

// =============================================================
// Phase 13 — page password / unlisted
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase13_password_protected_page_gates_anon() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let _ = make_page(&app, "secret").await;

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/secret/access",
            app.url
        ))
        .form(&[
            ("password", "letmein"),
            ("unlisted", "off"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200);

    // Anonymous request — should hit the unlock gate, not the page.
    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/alice/secret", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401, "should require unlock: got {}", r.status());
    let body = r.text().await.unwrap();
    assert!(body.contains("password") || body.contains("Password"));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase13_password_unlocks_page() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let _ = make_page(&app, "secret2").await;

    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!(
            "{}/apps/personal/pages/secret2/access",
            app.url
        ))
        .form(&[
            ("password", "open-sesame"),
            ("unlisted", "off"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();

    // Anonymous client with cookies enabled.
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let anon = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    // Provoke a CSRF cookie.
    let _ = anon
        .get(format!("{}/alice/secret2", app.url))
        .send()
        .await
        .unwrap();
    use reqwest::cookie::CookieStore;
    let url: reqwest::Url = format!("{}/", app.url).parse().unwrap();
    let cookies = jar.cookies(&url).unwrap();
    let s = cookies.to_str().unwrap();
    let mut csrf_v: Option<String> = None;
    for piece in s.split(';') {
        let piece = piece.trim();
        if let Some(rest) = piece.strip_prefix("csrf_token=") {
            csrf_v = Some(rest.to_string());
        }
    }
    let csrf = csrf_v.expect("csrf cookie set");
    let r = anon
        .post(format!("{}/p/alice/secret2/_unlock", app.url))
        .form(&[("password", "open-sesame"), ("_csrf", &csrf)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200, "{}", r.status());

    let r = anon
        .get(format!("{}/alice/secret2", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "after unlock should render");
}
