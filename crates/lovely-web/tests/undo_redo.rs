//! Undo/redo invariants. Per the user, undo recently nuked the entire
//! DOM — capture every scenario we can think of so the regression
//! can't sneak back.
//!
//! Tree shape used throughout: page has its root <div>, and we add
//! children to it with distinct tags so we can identify them by tag.

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

async fn root_id(app: &TestApp, slug: &str) -> uuid::Uuid {
    sqlx::query_scalar(
        r#"SELECT p.root_element FROM pages p
             JOIN apps a ON a.id = p.app_id
             JOIN users u ON u.id = a.owner_id
            WHERE u.username = 'alice' AND p.slug = $1"#,
    )
    .bind(slug)
    .fetch_one(&app.pg)
    .await
    .unwrap()
}

async fn add_child(app: &TestApp, slug: &str, parent: uuid::Uuid, tag: &str) {
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/{slug}/elements", app.url))
        .header("HX-Request", "true")
        .form(&[
            ("tag", tag),
            ("parent_id", parent.to_string().as_str()),
            ("_csrf", &token),
        ])
        .send()
        .await
        .unwrap();
    assert!(r.status() == 200, "add_child: {}", r.status());
}

async fn count_children(app: &TestApp, parent: uuid::Uuid) -> i64 {
    let n: (i64,) = sqlx::query_as("SELECT count(*) FROM elements WHERE parent_id = $1")
        .bind(parent)
        .fetch_one(&app.pg)
        .await
        .unwrap();
    n.0
}

async fn count_total(app: &TestApp, slug: &str) -> i64 {
    let n: (i64,) = sqlx::query_as(
        r#"SELECT count(*) FROM elements e
             JOIN pages p ON p.id = e.page_id
            WHERE p.slug = $1"#,
    )
    .bind(slug)
    .fetch_one(&app.pg)
    .await
    .unwrap();
    n.0
}

async fn click_undo(app: &TestApp, slug: &str) {
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/{slug}/undo", app.url))
        .header("HX-Request", "true")
        .form(&[("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "undo: {}", r.status());
}

async fn click_redo(app: &TestApp, slug: &str) {
    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/apps/personal/pages/{slug}/redo", app.url))
        .header("HX-Request", "true")
        .form(&[("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "redo: {}", r.status());
}

// ============================================================
// Bug regressions
// ============================================================

#[tokio::test]
#[ignore = "requires Docker"]
async fn undo_with_no_history_is_a_noop() {
    // Fresh app + home page (auto-created with one root <div>). Click
    // undo before doing anything else — must not delete the root.
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let _root = root_id(&app, "").await;

    let before = count_total(&app, "").await;
    click_undo(&app, "~home").await;
    let after = count_total(&app, "").await;
    assert_eq!(
        after, before,
        "undo with no history must not modify the page"
    );
    assert!(after >= 1, "page should still have its root element");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn undo_first_edit_returns_to_initial_state() {
    // Add one element, undo. Should revert to the just-the-root state.
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = root_id(&app, "").await;

    add_child(&app, "~home", root, "h1").await;
    assert_eq!(count_children(&app, root).await, 1);

    click_undo(&app, "~home").await;
    assert_eq!(
        count_children(&app, root).await,
        0,
        "undo should remove the freshly-added element"
    );
    let total = count_total(&app, "").await;
    assert_eq!(
        total, 1,
        "root must remain after undo (got {total} elements)"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn undo_restores_through_multiple_steps() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = root_id(&app, "").await;

    add_child(&app, "~home", root, "h1").await;
    add_child(&app, "~home", root, "h2").await;
    add_child(&app, "~home", root, "h3").await;
    assert_eq!(count_children(&app, root).await, 3);

    click_undo(&app, "~home").await;
    assert_eq!(count_children(&app, root).await, 2);
    click_undo(&app, "~home").await;
    assert_eq!(count_children(&app, root).await, 1);
    click_undo(&app, "~home").await;
    assert_eq!(count_children(&app, root).await, 0);
    // Further undo: no history, no-op.
    click_undo(&app, "~home").await;
    assert_eq!(count_children(&app, root).await, 0);
    assert_eq!(count_total(&app, "").await, 1, "root preserved");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn redo_restores_in_order() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = root_id(&app, "").await;

    add_child(&app, "~home", root, "h1").await;
    add_child(&app, "~home", root, "h2").await;
    click_undo(&app, "~home").await;
    click_undo(&app, "~home").await;
    assert_eq!(count_children(&app, root).await, 0);

    click_redo(&app, "~home").await;
    assert_eq!(count_children(&app, root).await, 1);
    click_redo(&app, "~home").await;
    assert_eq!(count_children(&app, root).await, 2);
    click_redo(&app, "~home").await;
    // Past-the-end redo: no-op.
    assert_eq!(count_children(&app, root).await, 2);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn new_edit_after_undo_truncates_redo_branch() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = root_id(&app, "").await;

    add_child(&app, "~home", root, "h1").await;
    add_child(&app, "~home", root, "h2").await;
    click_undo(&app, "~home").await; // now at {h1}
    add_child(&app, "~home", root, "section").await; // diverges from h2 history
    assert_eq!(count_children(&app, root).await, 2);
    let tags: Vec<String> =
        sqlx::query_scalar("SELECT tag FROM elements WHERE parent_id = $1 ORDER BY created_at ASC")
            .bind(root)
            .fetch_all(&app.pg)
            .await
            .unwrap();
    assert!(tags.contains(&"h1".to_string()) && tags.contains(&"section".to_string()));

    // Redo should be a no-op now (the branch was truncated).
    click_redo(&app, "~home").await;
    assert_eq!(count_children(&app, root).await, 2);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn undo_never_empties_the_page() {
    // Stress: alternate undo/redo many times, never lose the root.
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();
    let root = root_id(&app, "").await;

    add_child(&app, "~home", root, "h1").await;
    add_child(&app, "~home", root, "h2").await;
    for _ in 0..5 {
        click_undo(&app, "~home").await;
        click_redo(&app, "~home").await;
        click_undo(&app, "~home").await;
        click_undo(&app, "~home").await;
        click_redo(&app, "~home").await;
        click_redo(&app, "~home").await;
    }
    assert!(count_total(&app, "").await >= 1, "root must always remain");
}
