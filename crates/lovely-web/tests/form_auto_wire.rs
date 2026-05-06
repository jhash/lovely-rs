//! Form auto-wiring: a <form> whose descendants carry
//! data-lovely-source gets its action + method rewritten to the
//! public submit endpoint, descendants get their `name` attr set to
//! the field name, and a hidden _csrf input is injected.

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
async fn form_with_source_descendant_auto_wires_action_and_csrf() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    // Create a `comments` collection with one field.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data", app.url))
        .form(&[("name", "comments"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/data/comments/fields", app.url))
        .form(&[("name", "body"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();

    // Create a page with: <form> > <input> where input has data-lovely-source.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", "wire"),
            ("title", "P"),
            ("description", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let root: uuid::Uuid =
        sqlx::query_scalar("SELECT root_element FROM pages WHERE slug = 'wire'")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/wire/elements", app.url))
        .form(&[
            ("tag", "form"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let form_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM elements WHERE tag = 'form' LIMIT 1")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/wire/elements", app.url))
        .form(&[
            ("tag", "input"),
            ("parent_id", form_id.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let input_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM elements WHERE tag = 'input' LIMIT 1")
            .fetch_one(&app.pg)
            .await
            .unwrap();
    // Mark the input as a data source for comments.body.
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/wire/elements/{input_id}",
            app.url
        ))
        .form(&[
            ("source_collection", "comments"),
            ("source_field", "body"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // Publish so anon can fetch.
    sqlx::query("UPDATE pages SET published_at = now() WHERE slug = 'wire'")
        .execute(&app.pg)
        .await
        .unwrap();

    let r = app
        .client
        .get(format!("{}/alice/wire", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();

    // The form's action + method should be auto-set.
    assert!(
        body.contains(r#"action="/p/alice/~home/_submit/comments""#)
            || body.contains(r#"action="/p/alice/wire/_submit/comments""#),
        "form action must point at the submit endpoint: {body}"
    );
    assert!(body.contains(r#"method="post""#), "form method must be post: {body}");
    // The input must have name="body" (mapped from source field).
    assert!(
        body.contains(r#"name="body""#),
        "source input must carry name=\"body\": {body}"
    );
    // A hidden _csrf input must have been injected.
    assert!(
        body.contains(r#"name="_csrf""#),
        "auto-wired form must include a _csrf hidden input: {body}"
    );

    // End-to-end: extract the action + csrf from the rendered form,
    // submit it, and verify a comments row lands in the DB.
    let action_idx = body.find(r#"action=""#).expect("action attr");
    let after_action = &body[action_idx + r#"action=""#.len()..];
    let action = &after_action[..after_action.find('"').unwrap()];
    let csrf_idx = body
        .find(r#"name="_csrf""#)
        .expect("csrf hidden input present");
    let after_csrf = &body[csrf_idx..];
    let v_idx = after_csrf.find(r#"value=""#).expect("value=");
    let after_val = &after_csrf[v_idx + r#"value=""#.len()..];
    let csrf_val = &after_val[..after_val.find('"').unwrap()];

    let r = app
        .client
        .post(format!("{}{}", app.url, action))
        .form(&[("body", "auto-wired post"), ("_csrf", csrf_val)])
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_redirection() || r.status() == 200,
        "submit: {}",
        r.status()
    );

    let body: Option<String> = sqlx::query_scalar(
        "SELECT data_json->>'body' FROM records r \
         JOIN collections c ON c.id = r.collection_id \
         WHERE c.name = 'comments' LIMIT 1",
    )
    .fetch_optional(&app.pg)
    .await
    .unwrap();
    assert_eq!(body.as_deref(), Some("auto-wired post"));
}
