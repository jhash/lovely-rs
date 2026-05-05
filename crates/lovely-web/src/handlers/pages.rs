use crate::auth::{csrf, AuthUser, MaybeUser};
use crate::state::AppState;
use crate::views::pages as pages_views;
use crate::WebError;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use lovely_db::{
    create_page, find_app_by_owner_and_slug, find_default_app_for_username,
    find_page_by_app_and_slug, NewPage,
};
use lovely_tree::{ElementTag, Tree};
use serde::Deserialize;

pub async fn get_pages_new(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(app_slug): Path<String>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = pages_views::pages_new(&user, &app, &token, None).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

#[derive(Deserialize, Default)]
pub struct CreatePageForm {
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_pages_create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(app_slug): Path<String>,
    jar: CookieJar,
    Form(form): Form<CreatePageForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if !is_valid_slug(&form.slug) {
        let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
        let html = pages_views::pages_new(
            &user,
            &app,
            &token,
            Some("Slug must be empty (home page) or 1–80 chars: a-z, 0-9, hyphen"),
        )
        .into_string();
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            jar,
            axum::response::Html(html),
        )
            .into_response());
    }
    if form.title.trim().is_empty() {
        let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
        let html =
            pages_views::pages_new(&user, &app, &token, Some("Title is required")).into_string();
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            jar,
            axum::response::Html(html),
        )
            .into_response());
    }
    let new = NewPage {
        app_id: app.id,
        slug: form.slug,
        title: form.title,
        description: form.description.filter(|s| !s.is_empty()),
        author_id: user.id,
        root_tag: ElementTag::Div,
    };
    match create_page(&state.pg, new).await {
        Ok((page, _)) => Ok(Redirect::to(&format!(
            "/apps/{}/pages/{}/edit",
            app.slug,
            slug_path_segment(&page.slug)
        ))
        .into_response()),
        Err(lovely_db::DbError::Conflict(_)) => {
            let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
            let html = pages_views::pages_new(
                &user,
                &app,
                &token,
                Some("That slug is already used in this app"),
            )
            .into_string();
            Ok((
                StatusCode::UNPROCESSABLE_ENTITY,
                jar,
                axum::response::Html(html),
            )
                .into_response())
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn get_page_edit(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug)): Path<(String, String)>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = decode_slug_segment(&page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let rows = lovely_db::load_elements_for_page(&state.pg, page.id).await?;
    let tree = Tree::from_db_rows(&rows)?;
    let preview = tree.render();
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = pages_views::page_edit(&user, &app, &page, &rows, preview, &token).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

pub async fn get_public_user_root(
    State(state): State<AppState>,
    MaybeUser(viewer): MaybeUser,
    Path(username): Path<String>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    render_public(&state, viewer, &username, "", jar).await
}

pub async fn get_public_user_page(
    State(state): State<AppState>,
    MaybeUser(viewer): MaybeUser,
    Path((username, slug)): Path<(String, String)>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let real_slug = decode_slug_segment(&slug);
    render_public(&state, viewer, &username, &real_slug, jar).await
}

async fn render_public(
    state: &AppState,
    viewer: Option<lovely_db::User>,
    username: &str,
    slug: &str,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let Some((_owner, app)) = find_default_app_for_username(&state.pg, username).await? else {
        return Err(WebError::NotFound);
    };
    let page = find_page_by_app_and_slug(&state.pg, app.id, slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let rows = lovely_db::load_elements_for_page(&state.pg, page.id).await?;
    let tree = Tree::from_db_rows(&rows)?;
    let rendered = tree.render();
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = pages_views::published_page(viewer.as_ref(), &page, rendered, &token).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

#[derive(Deserialize, Default)]
pub struct DeletePageForm {
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn delete_page_handler(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<DeletePageForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = decode_slug_segment(&page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if page.author_id != user.id {
        return Err(WebError::Forbidden);
    }
    lovely_db::delete_page(&state.pg, page.id).await?;
    Ok(Redirect::to(&format!("/apps/{}", app.slug)).into_response())
}

#[derive(Deserialize, Default)]
pub struct UpdatePageForm {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub publish: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_page_update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<UpdatePageForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = decode_slug_segment(&page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if page.author_id != user.id {
        return Err(WebError::Forbidden);
    }
    let publish = form.publish.as_deref().map(|v| v == "on" || v == "true");
    lovely_db::update_page(
        &state.pg,
        page.id,
        lovely_db::PagePatch {
            title: form.title.filter(|s| !s.trim().is_empty()),
            description: Some(form.description.filter(|s| !s.is_empty())),
            publish,
        },
    )
    .await?;
    Ok(Redirect::to(&format!(
        "/apps/{}/pages/{}/edit",
        app.slug,
        slug_path_segment(&page.slug)
    ))
    .into_response())
}

fn is_valid_slug(s: &str) -> bool {
    s.is_empty()
        || (s.len() <= 80
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
}

/// Empty slug (home page) is represented as the literal `~home` in URLs
/// because axum's path matcher won't accept an empty segment.
pub fn slug_path_segment(s: &str) -> &str {
    if s.is_empty() {
        "~home"
    } else {
        s
    }
}

pub fn decode_slug_segment(s: &str) -> String {
    if s == "~home" {
        String::new()
    } else {
        s.to_string()
    }
}
