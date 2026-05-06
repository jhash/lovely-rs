//! End-to-end tests for the page builder. Each phase has its own block.
//!
//! These were written *before* the implementation as the red half of
//! red/green TDD. Don't loosen them later — if a test starts to feel
//! wrong, that's a signal to revisit the phase, not the assertion.

use lovely_test_support::{PgTestContainer, TestApp};
use serde_json::json;

/// Registers `alice` (default Personal app gets created automatically),
/// then creates a single page with slug `about` and returns the page UUID
/// + the root element UUID.
async fn fixture(app: &TestApp) -> anyhow::Result<(uuid::Uuid, uuid::Uuid)> {
    let token = app.csrf_token().await?;
    let r = app
        .client
        .post(format!("{}/auth/register", app.url))
        .form(&[
            ("username", "alice"),
            ("password", "correct horse battery staple"),
            ("_csrf", &token),
        ])
        .send()
        .await?;
    assert!(r.status().is_redirection(), "register: {}", r.status());

    let token = app.csrf_token().await?;
    let r = app
        .client
        .post(format!("{}/apps/personal/pages", app.url))
        .form(&[
            ("slug", "about"),
            ("title", "About"),
            ("description", "About me"),
            ("_csrf", &token),
        ])
        .send()
        .await?;
    assert!(r.status().is_redirection(), "create page: {}", r.status());

    let row: (uuid::Uuid, uuid::Uuid) = sqlx::query_as(
        r#"SELECT p.id, p.root_element
             FROM pages p
             JOIN apps a ON a.id = p.app_id
             JOIN users u ON u.id = a.owner_id
            WHERE u.username = 'alice' AND a.slug = 'personal' AND p.slug = 'about'"#,
    )
    .fetch_one(&app.pg)
    .await?;
    Ok(row)
}

// =============================================================
// Phase 1 — public page has no editor chrome
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase1_public_page_has_no_top_nav() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (page_id, _root) = fixture(&app).await.unwrap();

    // Publish the page so anonymous viewers can see it.
    sqlx::query("UPDATE pages SET published_at = now() WHERE id = $1")
        .bind(page_id)
        .execute(&app.pg)
        .await
        .unwrap();

    // Anonymous client (no cookies) so we don't accidentally render an
    // owner-only "edit" badge.
    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/alice/about", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(
        !body.contains("class=\"top-nav\"") && !body.contains("nav.top-nav"),
        "public page must not include the editor top-nav: {body}"
    );
    assert!(
        body.contains("<title>About</title>"),
        "public page <title> must come from the page row"
    );
}

// =============================================================
// Phase 2 — full-screen builder layout
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase2_edit_page_is_full_screen_builder() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let _ = fixture(&app).await.unwrap();

    let r = app
        .client
        .get(format!("{}/apps/personal/pages/about/edit", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(
        body.contains("class=\"builder\"") || body.contains("<body class=\"builder\""),
        "edit page <body> must carry the 'builder' class"
    );
    assert!(body.contains("id=\"tree\""), "expected #tree sidebar");
    assert!(
        body.contains("id=\"preview-canvas\""),
        "expected #preview-canvas inline render area"
    );
    assert!(
        body.contains("id=\"inspector\""),
        "expected #inspector right rail"
    );
    assert!(
        body.contains("/canvas"),
        "preview canvas should hx-get the /canvas fragment"
    );
}

// =============================================================
// Phase 3 — selection-driven inspector + tree fragment
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase3_inspector_renders_selected_element() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (_page, root) = fixture(&app).await.unwrap();

    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/about/inspector?sel={root}",
            app.url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(
        body.contains(&root.to_string()),
        "inspector should reference the selected element id"
    );
    assert!(
        body.contains("name=\"tab\"") || body.contains("data-tab"),
        "inspector should expose tabs (content/attrs/style)"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase3_tree_fragment_lists_elements_with_selection() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (_page, root) = fixture(&app).await.unwrap();

    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/about/tree?sel={root}",
            app.url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(
        body.contains(&format!("data-element-id=\"{root}\"")),
        "tree li must carry data-element-id"
    );
    assert!(
        body.contains("aria-current=\"true\"") || body.contains("aria-current=\"page\""),
        "selected node should expose aria-current"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase3_patch_text_returns_hx_trigger_preview_stale() {
    // Text now lives only on `#text` nodes — patch the text on a #text
    // child rather than on the root div.
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (_page, root) = fixture(&app).await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let _ = app
        .client
        .post(format!("{}/apps/personal/pages/about/elements", app.url))
        .form(&[
            ("tag", "#text"),
            ("parent_id", root.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    let txt: uuid::Uuid = sqlx::query_scalar("SELECT id FROM elements WHERE tag = '#text' LIMIT 1")
        .fetch_one(&app.pg)
        .await
        .unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/about/elements/{txt}",
            app.url
        ))
        .header("X-CSRF-Token", &token)
        .form(&[("text", "Hello world"), ("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "PATCH text element should succeed");
    let trigger = r.headers().get("HX-Trigger").map(|v| v.to_str().unwrap());
    assert_eq!(
        trigger,
        Some("preview-stale"),
        "PATCH must announce a stale preview"
    );

    let saved: Option<String> =
        sqlx::query_scalar("SELECT (payload->>'text')::text FROM elements WHERE id = $1")
            .bind(txt)
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert_eq!(saved.as_deref(), Some("Hello world"));
}

// =============================================================
// Phase 5 — owner-only iframe preview + HX-Trigger
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase5_owner_preview_route_renders_draft() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (_page, _root) = fixture(&app).await.unwrap();
    // page is a draft (published_at IS NULL) — owner preview must still render.

    let r = app
        .client
        .get(format!("{}/apps/personal/pages/about/canvas", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(
        body.contains("<div") || body.contains("<DIV"),
        "preview must render the root element tag"
    );
    assert!(
        !body.contains("class=\"top-nav\""),
        "preview iframe must not show editor chrome"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase5_preview_route_is_owner_only() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (_page, _root) = fixture(&app).await.unwrap();

    let anon = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = anon
        .get(format!("{}/apps/personal/pages/about/canvas", app.url))
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_redirection() || r.status() == 401 || r.status() == 403,
        "anon must not see the owner preview, got {}",
        r.status()
    );
}

// =============================================================
// Phase 4 — attribute editor (class, style, data-*, denylist)
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase4_attrs_tab_lists_existing_attrs() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (_page, root) = fixture(&app).await.unwrap();

    sqlx::query("UPDATE elements SET attrs = $2 WHERE id = $1")
        .bind(root)
        .bind(json!({ "class": "hero", "id": "top" }))
        .execute(&app.pg)
        .await
        .unwrap();

    let r = app
        .client
        .get(format!(
            "{}/apps/personal/pages/about/inspector?sel={root}&tab=attrs",
            app.url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.text().await.unwrap();
    assert!(body.contains("hero"), "attrs tab should show class=hero");
    assert!(body.contains("top"), "attrs tab should show id=top");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase4_patch_attr_persists_and_renders() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (_page, root) = fixture(&app).await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/about/elements/{root}",
            app.url
        ))
        .form(&[
            ("attr_name", "class"),
            ("attr_value", "hero"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let attrs: serde_json::Value = sqlx::query_scalar("SELECT attrs FROM elements WHERE id = $1")
        .bind(root)
        .fetch_one(&app.pg)
        .await
        .unwrap();
    assert_eq!(attrs.get("class").and_then(|v| v.as_str()), Some("hero"));

    // round-trip: rendered preview includes class="hero"
    let r = app
        .client
        .get(format!("{}/apps/personal/pages/about/canvas", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        body.contains("class=\"hero\""),
        "Tree::render should emit class=\"hero\""
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase4_patch_rejects_event_handler_attr() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (_page, root) = fixture(&app).await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .patch(format!(
            "{}/apps/personal/pages/about/elements/{root}",
            app.url
        ))
        .form(&[
            ("attr_name", "onclick"),
            ("attr_value", "alert(1)"),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        422,
        "event-handler attrs must be rejected at the boundary"
    );
}

// =============================================================
// Phase 6 — drag/drop reorder via /move
// =============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase6_move_into_self_returns_422() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (_page, root) = fixture(&app).await.unwrap();

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/about/elements/{root}/move",
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
    assert_eq!(r.status(), 422, "moving root under itself is a cycle");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn phase6_move_child_to_new_parent() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let (_page, root) = fixture(&app).await.unwrap();

    // Add two children to root, then move the second under the first.
    let token = app.csrf_token().await.unwrap();
    for _ in 0..2 {
        let r = app
            .client
            .post(format!("{}/apps/personal/pages/about/elements", app.url))
            .form(&[("tag", "p"), ("text", "x"), ("_csrf", &token)])
            .send()
            .await
            .unwrap();
        assert!(
            r.status().is_redirection() || r.status() == 200,
            "{}",
            r.status()
        );
    }
    let kids: Vec<uuid::Uuid> =
        sqlx::query_scalar("SELECT id FROM elements WHERE parent_id = $1 ORDER BY created_at ASC")
            .bind(root)
            .fetch_all(&app.pg)
            .await
            .unwrap();
    assert_eq!(kids.len(), 2);
    let (a, b) = (kids[0], kids[1]);

    let r = app
        .client
        .post(format!(
            "{}/apps/personal/pages/about/elements/{b}/move",
            app.url
        ))
        .form(&[
            ("parent_id", a.to_string().as_str()),
            ("prev_sibling", ""),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(
        r.headers().get("HX-Trigger").map(|v| v.to_str().unwrap()),
        Some("preview-stale")
    );

    let new_parent: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT parent_id FROM elements WHERE id = $1")
            .bind(b)
            .fetch_one(&app.pg)
            .await
            .unwrap();
    assert_eq!(new_parent, Some(a));
}
