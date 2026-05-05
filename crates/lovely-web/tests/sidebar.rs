//! Red tests for sidebar polish: add-before/add-after, inline #text,
//! and dropdown actions.

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

async fn fixture(app: &TestApp, page_slug: &str) -> uuid::Uuid {
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
// add-before / add-after
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn add_before_inserts_with_correct_prev_sibling() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = fixture(&app, "ord").await;

    // Existing child A.
    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/ord/elements", app.url))
        .form(&[
            ("tag", "p"),
            ("text", "A"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let a: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM elements WHERE parent_id = $1 LIMIT 1")
            .bind(root)
            .fetch_one(&app.pg)
            .await
            .unwrap();

    // Insert B *before* A.
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/ord/elements/{a}/add-before",
            app.url
        ))
        .form(&[("tag", "p"), ("text", "B"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200);

    // Now A.prev_sibling == B.id and B.prev_sibling IS NULL.
    let rows: Vec<(uuid::Uuid, Option<uuid::Uuid>, Option<String>)> = sqlx::query_as(
        "SELECT id, prev_sibling, payload->>'text' FROM elements \
         WHERE parent_id = $1 ORDER BY created_at ASC",
    )
    .bind(root)
    .fetch_all(&app.pg)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    let (a_row, _, _) = rows.iter().find(|r| r.2.as_deref() == Some("A")).unwrap();
    let (b_row, b_prev, _) = rows.iter().find(|r| r.2.as_deref() == Some("B")).unwrap();
    assert_eq!(b_prev, &None, "B should be first (prev_sibling NULL)");
    let (_, a_prev, _) = rows.iter().find(|r| &r.0 == a_row).unwrap();
    assert_eq!(a_prev, &Some(*b_row), "A.prev_sibling should now be B");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn add_after_relinks_next_sibling() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = fixture(&app, "ord2").await;

    let token = app.csrf_token().await.unwrap();
    for t in ["A", "C"] {
        let _ = app
            .client
            .post(format!("{}/apps/personal/pages/ord2/elements", app.url))
            .form(&[
                ("tag", "p"),
                ("text", t),
                ("parent_id", root.to_string().as_str()),
                ("_csrf", &token),
            ])
            .send()
            .await
            .unwrap();
    }
    let a: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM elements WHERE parent_id = $1 AND payload->>'text' = 'A'",
    )
    .bind(root)
    .fetch_one(&app.pg)
    .await
    .unwrap();
    let c: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM elements WHERE parent_id = $1 AND payload->>'text' = 'C'",
    )
    .bind(root)
    .fetch_one(&app.pg)
    .await
    .unwrap();
    // sanity: C.prev_sibling = A
    let prev_c: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT prev_sibling FROM elements WHERE id = $1")
            .bind(c)
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert_eq!(prev_c, Some(a));

    // Insert B after A.
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/ord2/elements/{a}/add-after",
            app.url
        ))
        .form(&[("tag", "p"), ("text", "B"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200);

    let b: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM elements WHERE parent_id = $1 AND payload->>'text' = 'B'",
    )
    .bind(root)
    .fetch_one(&app.pg)
    .await
    .unwrap();
    let new_prev_c: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT prev_sibling FROM elements WHERE id = $1")
            .bind(c)
            .fetch_one(&app.pg)
            .await
            .unwrap();
    let prev_b: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT prev_sibling FROM elements WHERE id = $1")
            .bind(b)
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert_eq!(prev_b, Some(a), "B.prev_sibling should be A");
    assert_eq!(
        new_prev_c,
        Some(b),
        "C.prev_sibling should now be B (relinked)"
    );
}

// ============================================================
// inline #text element
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn inline_text_renders_without_wrapping_tag() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = fixture(&app, "inline").await;
    sqlx::query("UPDATE pages SET published_at = now() WHERE slug = 'inline'")
        .execute(&app.pg)
        .await
        .unwrap();

    let token = app.csrf_token().await.unwrap();
    // [#text 'follow the link: ', <a>here</a>, #text '!']
    for (tag, text) in [
        ("#text", "follow the link: "),
        ("a", "here"),
        ("#text", "!"),
    ] {
        let _ = app
            .client
            .post(format!("{}/apps/personal/pages/inline/elements", app.url))
            .form(&[
                ("tag", tag),
                ("text", text),
                ("parent_id", root.to_string().as_str()),
                ("_csrf", &token),
            ])
            .send()
            .await
            .unwrap();
    }

    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/alice/inline", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(
        body.contains("follow the link: <a>here</a>!")
            || body.contains("follow the link: <a >here</a>!"),
        "inline text should render adjacent: {body}"
    );
    // The #text tag itself must not appear in the rendered output.
    assert!(!body.contains("<#text"), "#text tag should not render literally");
}

// ============================================================
// sidebar dropdown actions
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn sidebar_tree_row_has_actions_menu() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = fixture(&app, "menu").await;

    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/menu/tree?sel={root}",
            app.url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(body.contains("data-actions"), "row should expose data-actions menu");
    assert!(body.contains("aria-current=\"true\""), "selected row marked");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn sidebar_duplicate_action() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = fixture(&app, "dup").await;

    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/dup/elements", app.url))
        .form(&[
            ("tag", "p"),
            ("text", "X"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM elements WHERE parent_id = $1")
        .bind(root)
        .fetch_one(&app.pg)
        .await
        .unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/dup/elements/{id}/duplicate",
            app.url
        ))
        .form(&[("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 200);
    let n: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM elements WHERE parent_id = $1 AND payload->>'text' = 'X'",
    )
    .bind(root)
    .fetch_one(&app.pg)
    .await
    .unwrap();
    assert_eq!(n.0, 2, "duplicate should create a sibling copy");
}
