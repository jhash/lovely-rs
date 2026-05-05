use lovely_test_support::{PgTestContainer, TestApp};

#[tokio::test]
#[ignore = "requires Docker"]
async fn healthz_ok() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let r = app
        .client
        .get(format!("{}/healthz", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.text().await.unwrap(), "ok");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn register_login_logout_flow() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();

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
    assert!(
        r.status().is_redirection(),
        "expected redirect, got {}",
        r.status()
    );

    // Hit /apps — should now be authenticated and render.
    let r = app
        .client
        .get(format!("{}/apps", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let token = app.csrf_token().await.unwrap();
    let r = app
        .client
        .post(format!("{}/auth/logout", app.url))
        .form(&[("_csrf", &token)])
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection());

    // /apps now redirects to /auth/login (no session).
    let r = app
        .client
        .get(format!("{}/apps", app.url))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection() || r.status() == 303);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn csrf_post_without_token_returns_403() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let r = app
        .client
        .post(format!("{}/auth/login", app.url))
        .form(&[("username", "x"), ("password", "y")])
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn anonymous_apps_redirects_to_login() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    let r = app
        .client
        .get(format!("{}/apps", app.url))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_redirection());
    let loc = r.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.contains("/auth/login"));
}
