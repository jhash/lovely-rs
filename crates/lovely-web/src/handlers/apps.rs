use crate::auth::{csrf, AuthUser};
use crate::state::AppState;
use crate::views::apps as apps_views;
use crate::WebError;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use lovely_db::{
    count_apps_for_owner, create_app, delete_app, find_app_by_owner_and_slug,
    list_apps_by_owner, list_pages_in_app, update_app, AppPatch, NewApp,
};
use serde::Deserialize;

pub async fn get_apps_index(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let apps = list_apps_by_owner(&state.pg, user.id).await?;
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = apps_views::apps_index(&user, &apps, &token).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

pub async fn get_app_dashboard(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(app_slug): Path<String>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let pages = list_pages_in_app(&state.pg, app.id).await?;
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = apps_views::app_dashboard(&user, &app, &pages, &token).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

pub async fn get_apps_new(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = apps_views::apps_new(&user, &token, None).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

#[derive(Deserialize, Default)]
pub struct CreateAppForm {
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_apps_create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    jar: CookieJar,
    Form(form): Form<CreateAppForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    if !is_valid_app_slug(&form.slug) {
        let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
        let html = apps_views::apps_new(
            &user,
            &token,
            Some("Slug must be 1–40 chars: a-z, 0-9, hyphen"),
        )
        .into_string();
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, jar, axum::response::Html(html))
            .into_response());
    }
    if form.name.trim().is_empty() {
        let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
        let html =
            apps_views::apps_new(&user, &token, Some("Name is required")).into_string();
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, jar, axum::response::Html(html))
            .into_response());
    }
    match create_app(
        &state.pg,
        NewApp {
            slug: form.slug.clone(),
            name: form.name,
            description: form.description.filter(|s| !s.is_empty()),
            owner_id: user.id,
            is_default: false,
        },
    )
    .await
    {
        Ok(app) => Ok(Redirect::to(&format!("/apps/{}", app.slug)).into_response()),
        Err(lovely_db::DbError::Conflict(_)) => {
            let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
            let html = apps_views::apps_new(
                &user,
                &token,
                Some("That slug is already used"),
            )
            .into_string();
            Ok((StatusCode::UNPROCESSABLE_ENTITY, jar, axum::response::Html(html))
                .into_response())
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Deserialize, Default)]
pub struct RenameAppForm {
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_app_rename(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(app_slug): Path<String>,
    jar: CookieJar,
    Form(form): Form<RenameAppForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if !is_valid_app_slug(&form.slug) {
        return Err(WebError::Unprocessable(
            "Slug must be 1–40 chars: a-z, 0-9, hyphen".into(),
        ));
    }
    if form.name.trim().is_empty() {
        return Err(WebError::Unprocessable("Name is required".into()));
    }
    let updated = update_app(
        &state.pg,
        app.id,
        AppPatch {
            slug: Some(form.slug),
            name: Some(form.name),
            description: Some(form.description.filter(|s| !s.is_empty())),
        },
    )
    .await?;
    Ok(Redirect::to(&format!("/apps/{}", updated.slug)).into_response())
}

#[derive(Deserialize, Default)]
pub struct DeleteAppForm {
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_app_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(app_slug): Path<String>,
    jar: CookieJar,
    Form(form): Form<DeleteAppForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let n = count_apps_for_owner(&state.pg, user.id).await?;
    if n <= 1 {
        return Err(WebError::Unprocessable(
            "Cannot delete your only app — create another first.".into(),
        ));
    }
    delete_app(&state.pg, app.id).await?;
    Ok(Redirect::to("/apps").into_response())
}

#[derive(Deserialize, Default)]
pub struct ThemeForm {
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub ink: Option<String>,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_app_theme(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(app_slug): Path<String>,
    jar: CookieJar,
    Form(form): Form<ThemeForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let mut obj = serde_json::Map::new();
    let put = |obj: &mut serde_json::Map<String, serde_json::Value>, key: &str, val: Option<&str>| {
        if let Some(v) = val.filter(|s| !s.is_empty()) {
            // Whitelist a small set of value-grammars to keep CSS sane.
            let ok = v
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ",.-_# ()%/".contains(c));
            if ok {
                obj.insert(key.to_string(), serde_json::Value::String(v.to_string()));
            }
        }
    };
    put(&mut obj, "primary", form.primary.as_deref());
    put(&mut obj, "background", form.background.as_deref());
    put(&mut obj, "ink", form.ink.as_deref());
    put(&mut obj, "font", form.font.as_deref());
    lovely_db::update_app_theme(&state.pg, app.id, serde_json::Value::Object(obj)).await?;
    Ok(Redirect::to(&format!("/apps/{}", app.slug)).into_response())
}

// Legacy redirects so old /pages URLs still land somewhere.
pub async fn redirect_pages_index() -> Redirect {
    Redirect::to("/apps")
}

pub async fn redirect_pages_new() -> Redirect {
    Redirect::to("/apps/personal/pages/new")
}

fn is_valid_app_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
