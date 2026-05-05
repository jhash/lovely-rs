use crate::auth::{csrf, AuthUser};
use crate::state::AppState;
use crate::views::apps as apps_views;
use crate::WebError;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use lovely_db::{find_app_by_owner_and_slug, list_apps_by_owner, list_pages_in_app};

pub async fn get_apps_index(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let apps = list_apps_by_owner(&state.pg, user.id).await?;
    if apps.len() == 1 {
        return Ok(Redirect::to(&format!("/apps/{}", apps[0].slug)).into_response());
    }
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

// Legacy redirects so old /pages URLs still land somewhere.
pub async fn redirect_pages_index() -> Redirect {
    Redirect::to("/apps")
}

pub async fn redirect_pages_new() -> Redirect {
    Redirect::to("/apps/personal/pages/new")
}
