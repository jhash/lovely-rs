//! Cross-collection {{coll.field}} interpolation in #text content
//! and attribute values.

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

async fn make_collection(app: &TestApp, name: &str, field: &str) {
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data", app.url))
        .form(&[("name", name), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data/{name}/fields", app.url))
        .form(&[("name", field), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
}

async fn add_record(app: &TestApp, coll: &str, field: &str, value: &str) {
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data/{coll}/records", app.url))
        .form(&[(field, value), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn text_with_coll_field_placeholders_interpolates_on_render() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    make_collection(&app, "site", "tagline").await;
    add_record(&app, "site", "tagline", "Hello world").await;

    // Create a page with a #text child carrying interpolation.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", "ip"),
            ("title", "P"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let root: uuid::Uuid =
        sqlx::query_scalar("SELECT root_element FROM pages WHERE slug = 'ip'")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/ip/elements", app.url))
        .form(&[
            ("tag", "#text"),
            ("text", "Site says: {{site.tagline}}"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();

    sqlx::query("UPDATE pages SET published_at = now() WHERE slug = 'ip'")
        .execute(&app.pg)
        .await
        .unwrap();

    let r = app
        .client
        .get(format!("{}/alice/ip", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        body.contains("Site says: Hello world"),
        "expected interpolated text, got: {body}"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn attr_with_coll_field_placeholders_interpolates_on_render() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    make_collection(&app, "links", "href").await;
    add_record(&app, "links", "href", "https://example.com").await;

    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", "ap"),
            ("title", "P"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let root: uuid::Uuid =
        sqlx::query_scalar("SELECT root_element FROM pages WHERE slug = 'ap'")
            .fetch_one(&app.pg)
            .await
            .unwrap();

    // Add an <a> element, then patch its href attr to a placeholder.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/ap/elements", app.url))
        .form(&[
            ("tag", "a"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let a_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM elements WHERE tag = 'a' LIMIT 1")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .patch(format!("{}/apps/personal/pages/ap/elements/{a_id}", app.url))
        .form(&[
            ("attr_name", "href"),
            ("attr_value", "{{links.href}}"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();

    sqlx::query("UPDATE pages SET published_at = now() WHERE slug = 'ap'")
        .execute(&app.pg)
        .await
        .unwrap();

    let r = app
        .client
        .get(format!("{}/alice/ap", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        body.contains("href=\"https://example.com\""),
        "expected interpolated href, got: {body}"
    );
}
