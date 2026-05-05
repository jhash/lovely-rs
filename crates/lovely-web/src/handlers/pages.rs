use crate::auth::{csrf, AuthUser};
use crate::state::AppState;
use crate::views::pages as pages_views;
use crate::WebError;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use lovely_db::{create_page, list_pages_by_author, NewPage};
use lovely_tree::{ElementTag, Tree};
use serde::Deserialize;

pub async fn get_pages_index(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let pages = list_pages_by_author(&state.pg, user.id).await?;
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = pages_views::pages_index(&user, &pages, &token).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

pub async fn get_pages_new(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = pages_views::pages_new(&user, &token, None).into_string();
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
    jar: CookieJar,
    Form(form): Form<CreatePageForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    if !is_valid_slug(&form.slug) {
        let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
        let html = pages_views::pages_new(
            &user,
            &token,
            Some("Slug must be 1–80 chars: a-z, 0-9, hyphen"),
        )
        .into_string();
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            jar,
            axum::response::Html(html),
        )
            .into_response());
    }
    let new_page = NewPage {
        slug: form.slug,
        title: form.title,
        description: form.description.filter(|s| !s.is_empty()),
        author_id: user.id,
        root_tag: ElementTag::Div,
    };
    match create_page(&state.pg, new_page).await {
        Ok((page, _)) => Ok(Redirect::to(&format!("/pages/{}", page.slug)).into_response()),
        Err(lovely_db::DbError::Conflict(_)) => {
            let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
            let html =
                pages_views::pages_new(&user, &token, Some("That slug is taken")).into_string();
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

pub async fn get_page_by_slug(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let page = lovely_db::find_page_by_slug(&state.pg, &slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let rows = lovely_db::load_elements_for_page(&state.pg, page.id).await?;
    let tree = Tree::from_db_rows(&rows)?;
    let rendered = tree.render();
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = pages_views::published_page(&page, rendered, &token).into_string();
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
    Path(slug): Path<String>,
    jar: CookieJar,
    Form(form): Form<DeletePageForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let page = lovely_db::find_page_by_slug(&state.pg, &slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if page.author_id != user.id {
        return Err(WebError::Forbidden);
    }
    lovely_db::delete_page(&state.pg, page.id).await?;
    Ok(Redirect::to("/pages").into_response())
}

fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
