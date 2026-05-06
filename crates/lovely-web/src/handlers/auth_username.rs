use crate::auth::{csrf, extractor::SESSION_COOKIE, hash_password, verify_password, MaybeUser};
use crate::state::AppState;
use crate::views::auth as auth_views;
use crate::WebError;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Form;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use lovely_db::{create_user, find_user_by_username, NewUser};
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct LoginForm {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub _csrf: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct RegisterForm {
    #[serde(default)]
    pub username: String,
    pub email: Option<String>,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn get_login(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    jar: CookieJar,
) -> Response {
    if user.is_some() {
        return axum::response::Redirect::to("/apps").into_response();
    }
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = auth_views::login_page(&token, None).into_string();
    (jar, axum::response::Html(html)).into_response()
}

pub async fn post_login(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;

    let user = find_user_by_username(&state.pg, &form.username).await?;
    let Some(user) = user else {
        let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
        let html =
            auth_views::login_page(&token, Some("Invalid username or password")).into_string();
        return Ok((StatusCode::UNAUTHORIZED, jar, axum::response::Html(html)).into_response());
    };
    let Some(hash) = user.password_hash.as_deref() else {
        let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
        let html = auth_views::login_page(&token, Some("Use OAuth to sign in to this account"))
            .into_string();
        return Ok((StatusCode::UNAUTHORIZED, jar, axum::response::Html(html)).into_response());
    };
    if !verify_password(&form.password, hash) {
        let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
        let html =
            auth_views::login_page(&token, Some("Invalid username or password")).into_string();
        return Ok((StatusCode::UNAUTHORIZED, jar, axum::response::Html(html)).into_response());
    }

    // Create session
    let session = lovely_db::create_session(
        &state.pg,
        lovely_db::NewSession {
            user_id: user.id,
            ttl: chrono::Duration::days(30),
            user_agent: None,
        },
    )
    .await?;
    let secure = state.base_url.starts_with("https://");
    let cookie = Cookie::build((SESSION_COOKIE, session.id))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(time::Duration::days(30));
    let jar = jar.add(cookie);
    Ok((jar, axum::response::Redirect::to("/pages")).into_response())
}

#[derive(serde::Deserialize, Default)]
pub struct CheckUsernameQuery {
    #[serde(default)]
    pub username: Option<String>,
}

/// Live-validation for the registration form's username field. Mirrors
/// `/apps/check-slug` semantics: returns a `.slug-feedback`-flavored
/// fragment (`.slug-error` / `.slug-ok`) so the existing JS can flip
/// the input's aria-invalid + disable the submit button.
pub async fn get_check_username(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<CheckUsernameQuery>,
) -> Result<Response, WebError> {
    let raw = q.username.unwrap_or_default();
    let trimmed = raw.trim();
    // Same shape rules as the registration form (3..=40 alphanumeric + underscore).
    if trimmed.len() < 3 {
        return Ok(axum::response::Html("").into_response());
    }
    if trimmed.len() > 40
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Ok(axum::response::Html(
            r#"<span class="slug-error">Use 3–40 letters, digits, dashes, or underscores.</span>"#,
        )
        .into_response());
    }
    let taken = lovely_db::find_user_by_username(&state.pg, trimmed)
        .await?
        .is_some();
    let body = if taken {
        format!(
            r#"<span class="slug-error">"{}" is already taken.</span>"#,
            html_escape(trimmed)
        )
    } else {
        format!(
            r#"<span class="slug-ok">"{}" is available.</span>"#,
            html_escape(trimmed)
        )
    };
    Ok(axum::response::Html(body).into_response())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub async fn get_register(
    State(state): State<AppState>,
    MaybeUser(maybe_user): MaybeUser,
    jar: CookieJar,
) -> Response {
    if maybe_user.is_some() {
        return axum::response::Redirect::to("/pages").into_response();
    }
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = auth_views::register_page(&token, None).into_string();
    (jar, axum::response::Html(html)).into_response()
}

pub async fn post_register(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<RegisterForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;

    if form.username.len() < 3 || form.username.len() > 40 {
        return Ok(register_error(
            &state,
            jar,
            "Username must be 3–40 characters",
        ));
    }
    if form.password.len() < 8 {
        return Ok(register_error(
            &state,
            jar,
            "Password must be at least 8 characters",
        ));
    }
    if !form
        .username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Ok(register_error(
            &state,
            jar,
            "Username may only contain a-z, 0-9, _, -",
        ));
    }
    let hash = hash_password(&form.password)?;
    let user = match create_user(
        &state.pg,
        NewUser {
            username: form.username,
            email: form.email.filter(|s| !s.is_empty()),
            password_hash: Some(hash),
        },
    )
    .await
    {
        Ok(u) => u,
        Err(lovely_db::DbError::Conflict(_)) => {
            return Ok(register_error(
                &state,
                jar,
                "Username or email already taken",
            ));
        }
        Err(e) => return Err(e.into()),
    };

    // Auto-create the user's default "Personal" app.
    lovely_db::create_app(
        &state.pg,
        lovely_db::NewApp {
            slug: "personal".into(),
            name: "Personal".into(),
            description: None,
            owner_id: user.id,
            is_default: true,
        },
    )
    .await?;

    let session = lovely_db::create_session(
        &state.pg,
        lovely_db::NewSession {
            user_id: user.id,
            ttl: chrono::Duration::days(30),
            user_agent: None,
        },
    )
    .await?;
    let secure = state.base_url.starts_with("https://");
    let cookie = Cookie::build((SESSION_COOKIE, session.id))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(time::Duration::days(30));
    let jar = jar.add(cookie);
    Ok((jar, axum::response::Redirect::to("/pages")).into_response())
}

fn register_error(state: &AppState, jar: CookieJar, msg: &'static str) -> Response {
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = auth_views::register_page(&token, Some(msg)).into_string();
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        jar,
        axum::response::Html(html),
    )
        .into_response()
}

#[derive(Deserialize, Default)]
pub struct LogoutForm {
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_logout(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<LogoutForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    if let Some(c) = jar.get(SESSION_COOKIE) {
        let _ = lovely_db::delete_session(&state.pg, c.value()).await;
    }
    let jar = jar.remove(SESSION_COOKIE);
    Ok((jar, axum::response::Redirect::to("/")).into_response())
}
