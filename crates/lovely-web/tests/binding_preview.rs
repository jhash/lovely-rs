//! Reproduces "Internal Server Error" in the canvas preview iframe
//! after binding an element to a collection field.

use lovely_test_support::{PgTestContainer, TestApp};

#[tokio::test]
#[ignore = "requires Docker"]
async fn binding_does_not_500_the_preview() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();

    // Register alice, get personal app + a page.
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/auth/register", app.url))
        .form(&[
            ("username", "alice"),
            ("password", "correct horse battery staple"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection());

    let token = app.csrf_token().await.unwrap();
    let r = app
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
    assert!(r.status().is_redirection());

    // Create a `comments` collection with a `content` field.
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data", app.url))
        .form(&[("name", "comments"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200, "{}", r.status());
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/data/comments/fields", app.url))
        .form(&[("name", "content"), ("field_type", "text"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200, "{}", r.status());

    // Insert a record so resolve_bindings has something to substitute.
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/data/comments/records",
            app.url
        ))
        .form(&[("content", "hello world"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200, "{}", r.status());

    let root: uuid::Uuid =
        sqlx::query_scalar("SELECT root_element FROM pages WHERE slug = 'p1'")
            .fetch_one(&app.pg)
            .await
            .unwrap();

    // Bind the ROOT element directly (a div).
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/p1/elements/{root}",
            app.url
        ))
        .form(&[
            ("binding_collection", "comments"),
            ("binding_field", "content"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "patch root: {}", r.status());

    // GET the preview iframe (canvas) — must not 500.
    let r = app
        .client
        .get(format!("{}/apps/personal/pages/p1/canvas", app.url))
        .send()
        .await
        .unwrap();
    let status = r.status();
    let body = r.text().await.unwrap();
    assert_eq!(status, 200, "preview after bind on root should not 500: {body}");

    // Also: GET the public render path (this DOES run resolve_bindings).
    sqlx::query("UPDATE pages SET published_at = now() WHERE slug = 'p1'")
        .execute(&app.pg)
        .await
        .unwrap();
    let r = app
        .client
        .get(format!("{}/alice/p1", app.url))
        .send()
        .await
        .unwrap();
    let status = r.status();
    let body = r.text().await.unwrap();
    assert_eq!(status, 200, "public render after bind should not 500: {body}");
}
