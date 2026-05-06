//! Regression: clicking the Sign out button must (a) actually clear
//! the session and (b) redirect to "/" (not return a 403 "CSRF" page).

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

/// Fetch the rendered nav and submit the logout form using exactly the
/// hidden fields it ships with. This catches the original bug where the
/// nav rendered the form without a `_csrf` field, making every click on
/// "Sign out" 403 with body "CSRF".
fn extract_csrf_in_logout_form(html: &str) -> Option<String> {
    // Find the `<form ... action="/auth/logout" ...>` block.
    let start = html.find("action=\"/auth/logout\"")?;
    let after = &html[start..];
    let form_end = after.find("</form>")?;
    let form_html = &after[..form_end];
    // Look for `name="_csrf" value="..."`.
    let needle = "name=\"_csrf\"";
    let idx = form_html.find(needle)?;
    let tail = &form_html[idx..];
    let v_idx = tail.find("value=\"")?;
    let after_v = &tail[v_idx + 7..];
    let q = after_v.find('"')?;
    Some(after_v[..q].to_string())
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn logout_form_in_nav_carries_csrf_and_clears_session() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    // Before logout: /apps is reachable as alice.
    let r = app
        .client
        .get(format!("{}/apps", app.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "alice should reach /apps before logout");
    let body = r.text().await.unwrap();
    let token = extract_csrf_in_logout_form(&body)
        .expect("nav must render the logout form WITH a _csrf hidden input");

    // Submit the logout form using only the fields the nav shipped.
    let r = app
        .client
        .post(format!("{}/auth/logout", app.url))
        .form(&[("_csrf", token.as_str())])
        .send()
        .await
        .unwrap();
    // Should be a redirect to "/", never a 403 "CSRF" page.
    assert!(
        r.status().is_redirection(),
        "logout should redirect, got {}",
        r.status()
    );
    let loc = r
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(loc, "/", "logout should send users to the home page");
    let body = r.text().await.unwrap();
    assert!(
        !body.contains("CSRF"),
        "logout response must never display the bare text \"CSRF\""
    );

    // Verify session is actually cleared: /apps now bounces to login.
    let r = app
        .client
        .get(format!("{}/apps", app.url))
        .send()
        .await
        .unwrap();
    let status = r.status();
    let location = r
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        location.contains("/auth/login") || status == 401 || status == 403,
        "after logout, /apps should redirect to /auth/login (got status {status}, location \"{location}\")",
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn logout_button_in_nav_omits_username() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let app = TestApp::start_with_pool(pool).await.unwrap();
    register(&app, "alice").await.unwrap();

    let r = app
        .client
        .get(format!("{}/apps", app.url))
        .send()
        .await
        .unwrap();
    let body = r.text().await.unwrap();
    assert!(
        !body.contains("Sign out (alice)") && !body.contains("Sign out (\"alice\")"),
        "Sign out button must not embed the username"
    );
    assert!(
        body.contains("Sign out"),
        "Sign out button label must still read \"Sign out\""
    );
}
