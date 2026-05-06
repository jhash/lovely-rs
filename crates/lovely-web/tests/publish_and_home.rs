//! Regression tests for: page publish toggle persists both ways and
//! returns OOB pill swaps; default Home page is created with each app
//! and is undeletable; non-owner views of unpublished pages redirect
//! to "/" (not 404).

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
async fn registering_creates_a_home_page_per_app() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    // The personal app must have a single page with empty slug + title "Home".
    let n: (i64,) = sqlx::query_as(
        r#"SELECT count(*) FROM pages p
            JOIN apps a ON a.id = p.app_id
            JOIN users u ON u.id = a.owner_id
           WHERE u.username = 'alice'
             AND a.slug = 'personal'
             AND p.slug = ''"#,
    )
    .fetch_one(&app.pg)
    .await
    .unwrap();
    assert_eq!(n.0, 1, "Personal app should ship with one home page");
    let title: String = sqlx::query_scalar(
        "SELECT p.title FROM pages p JOIN apps a ON a.id = p.app_id \
         JOIN users u ON u.id = a.owner_id \
         WHERE u.username = 'alice' AND a.slug = 'personal' AND p.slug = ''",
    )
    .fetch_one(&app.pg)
    .await
    .unwrap();
    assert_eq!(title, "Home");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn deleting_the_home_page_is_rejected() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/~home/delete", app.url))
        .form(&[("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        422,
        "Home page deletion should be rejected (got {})",
        r.status()
    );
    let still: (i64,) = sqlx::query_as("SELECT count(*) FROM pages WHERE slug = ''")
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(still.0, 1, "home page should still exist");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn publish_toggle_persists_on_then_off_with_oob_swap() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    // Create a page so we have something to toggle.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", "p1"),
            ("title", "P1"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();

    // Toggle ON: publish=on, _publish_form=1.
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/p1", app.url))
        .form(&[("publish", "on"), ("_publish_form", "1"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(
        body.contains(r#"id="topbar-publish-pill""#)
            && body.contains(r#"hx-swap-oob="true""#)
            && body.contains("pill-published")
            && body.contains("published"),
        "publish-on response must OOB-swap topbar pill to published: {body}"
    );
    assert!(
        body.contains(r#"id="tree-page-pill""#),
        "publish-on response must OOB-swap tree page pill: {body}"
    );

    let pub_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT published_at FROM pages WHERE slug = 'p1'")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert!(pub_at.is_some(), "publish_at should be set after ON");

    // Toggle OFF: only _publish_form=1, no publish field (browser
    // doesn't send unchecked checkboxes).
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/p1", app.url))
        .form(&[("_publish_form", "1"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(
        body.contains("pill-draft") && body.contains("draft"),
        "publish-off response must OOB-swap pills to draft: {body}"
    );

    let pub_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT published_at FROM pages WHERE slug = 'p1'")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert!(pub_at.is_none(), "publish_at should be cleared after OFF");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn anon_view_of_published_page_renders_200() {
    // Regression: the /-redirect must only fire for UNPUBLISHED pages.
    // A published page must render for anon viewers.
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", "live"),
            ("title", "Live"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    // Publish via the same flow the UI uses (publish form).
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/live", app.url))
        .form(&[("publish", "on"), ("_publish_form", "1"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "publish toggle: {}", r.status());

    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/alice/live", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        200,
        "published page must render for anon (got {})",
        r.status()
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn anon_view_of_unpublished_page_redirects_to_home() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    // Make a draft page.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", "draft"),
            ("title", "Draft"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();

    // Anon viewer (no cookies).
    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/alice/draft", app.url))
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_redirection(),
        "anon should be redirected, got {}",
        r.status()
    );
    let loc = r
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(loc, "/", "redirect should land on home");
}
