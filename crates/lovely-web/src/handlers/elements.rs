use crate::auth::{csrf, AuthUser};
use crate::state::AppState;
use crate::WebError;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use axum_htmx::HxRequest;
use lovely_db::{find_app_by_owner_and_slug, find_page_by_app_and_slug};
use lovely_tree::ElementTag;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

fn hx_ok_preview_stale() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert("HX-Trigger", HeaderValue::from_static("preview-stale"));
    (StatusCode::OK, headers, Html("")).into_response()
}

#[derive(Deserialize)]
pub struct AddElementForm {
    pub tag: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_add_element(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug)): Path<(String, String)>,
    HxRequest(is_htmx): HxRequest,
    jar: CookieJar,
    Form(form): Form<AddElementForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = super::pages::decode_slug_segment(&page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if page.author_id != user.id {
        return Err(WebError::Forbidden);
    }
    if ElementTag::from_name(&form.tag).is_none() {
        return Err(WebError::BadRequest(format!("unknown tag: {}", form.tag)));
    }
    let parent_id = form.parent_id.or(page.root_element);
    let parent_id = parent_id.ok_or(WebError::BadRequest("page has no root element".into()))?;
    // Refuse if parent is a leaf element (input/textarea/img/br/hr/...).
    let parent_tag: Option<(String,)> =
        sqlx::query_as("SELECT tag FROM elements WHERE id = $1 AND page_id = $2")
            .bind(parent_id)
            .bind(page.id)
            .fetch_optional(&state.pg)
            .await
            .map_err(lovely_db::DbError::Sqlx)?;
    if let Some((tag,)) = parent_tag.as_ref() {
        if let Some(t) = ElementTag::from_name(tag) {
            if t.is_leaf() {
                return Err(WebError::Unprocessable(format!(
                    "<{}> elements cannot have children",
                    tag
                )));
            }
        }
    }
    // Find the current last child of parent_id to set prev_sibling.
    let prev_sibling: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM elements \
         WHERE page_id = $1 AND parent_id = $2 \
         AND NOT EXISTS (SELECT 1 FROM elements e2 \
                         WHERE e2.page_id = $1 AND e2.parent_id = $2 \
                         AND e2.prev_sibling = elements.id)",
    )
    .bind(page.id)
    .bind(parent_id)
    .fetch_optional(&state.pg)
    .await
    .map_err(lovely_db::DbError::Sqlx)?;
    let payload = json!({"text": form.text.unwrap_or_default()});
    lovely_db::insert_element(
        &state.pg,
        lovely_db::InsertElement {
            page_id: page.id,
            parent_id: Some(parent_id),
            prev_sibling: prev_sibling.map(|t| t.0),
            tag: form.tag,
            attrs: serde_json::Value::Object(Default::default()),
            payload,
        },
    )
    .await?;
    lovely_db::snapshot_page(&state.pg, page.id).await?;
    if is_htmx {
        return Ok(hx_ok_preview_stale());
    }
    Ok(Redirect::to(&format!(
        "/apps/{}/pages/{}/edit",
        app.slug,
        super::pages::slug_path_segment(&page.slug)
    ))
    .into_response())
}

#[derive(Deserialize, Default)]
pub struct AddSiblingForm {
    pub tag: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

/// Insert a new sibling immediately before the target element.
pub async fn post_add_before(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug, target_id)): Path<(String, String, Uuid)>,
    HxRequest(is_htmx): HxRequest,
    jar: CookieJar,
    Form(form): Form<AddSiblingForm>,
) -> Result<Response, WebError> {
    add_sibling(
        &state,
        user.id,
        &app_slug,
        &page_slug,
        target_id,
        is_htmx,
        jar,
        form,
        Position::Before,
    )
    .await
}

/// Insert a new sibling immediately after the target element.
pub async fn post_add_after(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug, target_id)): Path<(String, String, Uuid)>,
    HxRequest(is_htmx): HxRequest,
    jar: CookieJar,
    Form(form): Form<AddSiblingForm>,
) -> Result<Response, WebError> {
    add_sibling(
        &state,
        user.id,
        &app_slug,
        &page_slug,
        target_id,
        is_htmx,
        jar,
        form,
        Position::After,
    )
    .await
}

#[derive(Copy, Clone)]
enum Position {
    Before,
    After,
}

#[allow(clippy::too_many_arguments)]
async fn add_sibling(
    state: &AppState,
    user_id: Uuid,
    app_slug: &str,
    page_slug: &str,
    target_id: Uuid,
    is_htmx: bool,
    jar: CookieJar,
    form: AddSiblingForm,
    pos: Position,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user_id, app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = super::pages::decode_slug_segment(page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if page.author_id != user_id {
        return Err(WebError::Forbidden);
    }
    if !is_valid_tag(&form.tag) {
        return Err(WebError::BadRequest(format!("unknown tag: {}", form.tag)));
    }
    // Look up target's parent + prev_sibling.
    let target: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT parent_id, prev_sibling FROM elements WHERE id = $1 AND page_id = $2",
    )
    .bind(target_id)
    .bind(page.id)
    .fetch_optional(&state.pg)
    .await
    .map_err(lovely_db::DbError::Sqlx)?;
    let Some((parent_id, target_prev)) = target else {
        return Err(WebError::NotFound);
    };
    let new_prev = match pos {
        Position::Before => target_prev, // new takes target's old prev
        Position::After => Some(target_id),
    };
    let payload = json!({"text": form.text.unwrap_or_default()});
    let new_row = lovely_db::insert_element(
        &state.pg,
        lovely_db::InsertElement {
            page_id: page.id,
            parent_id,
            prev_sibling: new_prev,
            tag: form.tag,
            attrs: serde_json::Value::Object(Default::default()),
            payload,
        },
    )
    .await?;
    // Before-position: relink target so its prev_sibling becomes the new id.
    if matches!(pos, Position::Before) {
        sqlx::query("UPDATE elements SET prev_sibling = $1 WHERE id = $2")
            .bind(new_row.id)
            .bind(target_id)
            .execute(&state.pg)
            .await
            .map_err(lovely_db::DbError::Sqlx)?;
    }
    lovely_db::snapshot_page(&state.pg, page.id).await?;
    if is_htmx {
        return Ok(hx_ok_preview_stale());
    }
    Ok(Redirect::to(&format!(
        "/apps/{}/pages/{}/edit",
        app.slug,
        super::pages::slug_path_segment(&page.slug)
    ))
    .into_response())
}

fn is_valid_tag(s: &str) -> bool {
    ElementTag::from_name(s).is_some()
}

#[derive(Deserialize, Default)]
pub struct DuplicateForm {
    #[serde(default)]
    pub _csrf: Option<String>,
}

/// Wrap the target element in a fresh parent of `tag`. Default tag is
/// `div`. The new wrapper inherits the target's parent + prev_sibling;
/// the target becomes the wrapper's only child.
#[derive(Deserialize, Default)]
pub struct WrapForm {
    #[serde(default = "default_wrap_tag")]
    pub tag: String,
    #[serde(default)]
    pub _csrf: Option<String>,
}

fn default_wrap_tag() -> String {
    "div".into()
}

pub async fn post_wrap_element(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug, target_id)): Path<(String, String, Uuid)>,
    HxRequest(is_htmx): HxRequest,
    jar: CookieJar,
    Form(form): Form<WrapForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = super::pages::decode_slug_segment(&page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if page.author_id != user.id {
        return Err(WebError::Forbidden);
    }
    if Some(target_id) == page.root_element {
        return Err(WebError::Unprocessable("cannot wrap the root element".into()));
    }
    if !is_valid_tag(&form.tag) || form.tag == "#text" {
        return Err(WebError::BadRequest(format!("unwrappable tag: {}", form.tag)));
    }
    let target: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT parent_id, prev_sibling FROM elements WHERE id = $1 AND page_id = $2",
    )
    .bind(target_id)
    .bind(page.id)
    .fetch_optional(&state.pg)
    .await
    .map_err(lovely_db::DbError::Sqlx)?;
    let Some((parent_id, prev_sibling)) = target else {
        return Err(WebError::NotFound);
    };
    let mut tx = state.pg.begin().await.map_err(lovely_db::DbError::Sqlx)?;
    // Insert the wrapper at the target's old position.
    let wrap_id: (Uuid,) = sqlx::query_as(
        "INSERT INTO elements (page_id, parent_id, prev_sibling, tag, attrs, payload) \
         VALUES ($1, $2, $3, $4, '{}'::jsonb, '{}'::jsonb) RETURNING id",
    )
    .bind(page.id)
    .bind(parent_id)
    .bind(prev_sibling)
    .bind(&form.tag)
    .fetch_one(&mut *tx)
    .await
    .map_err(lovely_db::DbError::Sqlx)?;
    // Anything that pointed at the target as its prev_sibling now points at the wrapper.
    sqlx::query(
        "UPDATE elements SET prev_sibling = $1, updated_at = now() \
         WHERE page_id = $2 AND id != $1 \
           AND parent_id IS NOT DISTINCT FROM $3 AND prev_sibling = $4",
    )
    .bind(wrap_id.0)
    .bind(page.id)
    .bind(parent_id)
    .bind(target_id)
    .execute(&mut *tx)
    .await
    .map_err(lovely_db::DbError::Sqlx)?;
    // Move the target under the wrapper as its only child.
    sqlx::query(
        "UPDATE elements SET parent_id = $1, prev_sibling = NULL, updated_at = now() WHERE id = $2",
    )
    .bind(wrap_id.0)
    .bind(target_id)
    .execute(&mut *tx)
    .await
    .map_err(lovely_db::DbError::Sqlx)?;
    tx.commit().await.map_err(lovely_db::DbError::Sqlx)?;
    lovely_db::snapshot_page(&state.pg, page.id).await?;
    if is_htmx {
        return Ok(hx_ok_preview_stale());
    }
    Ok(Redirect::to(&format!(
        "/apps/{}/pages/{}/edit",
        app.slug,
        super::pages::slug_path_segment(&page.slug)
    ))
    .into_response())
}

pub async fn post_duplicate_element(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug, target_id)): Path<(String, String, Uuid)>,
    HxRequest(is_htmx): HxRequest,
    jar: CookieJar,
    Form(form): Form<DuplicateForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = super::pages::decode_slug_segment(&page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if page.author_id != user.id {
        return Err(WebError::Forbidden);
    }
    // Pull the source row's tag/attrs/payload.
    let src: Option<(Option<Uuid>, String, serde_json::Value, serde_json::Value)> = sqlx::query_as(
        "SELECT parent_id, tag, attrs, payload FROM elements WHERE id = $1 AND page_id = $2",
    )
    .bind(target_id)
    .bind(page.id)
    .fetch_optional(&state.pg)
    .await
    .map_err(lovely_db::DbError::Sqlx)?;
    let Some((parent_id, tag, attrs, payload)) = src else {
        return Err(WebError::NotFound);
    };
    // Insert the dup as a new sibling immediately after the source.
    let new_row = lovely_db::insert_element(
        &state.pg,
        lovely_db::InsertElement {
            page_id: page.id,
            parent_id,
            prev_sibling: Some(target_id),
            tag,
            attrs,
            payload,
        },
    )
    .await?;
    let _ = new_row;
    lovely_db::snapshot_page(&state.pg, page.id).await?;
    if is_htmx {
        return Ok(hx_ok_preview_stale());
    }
    Ok(Redirect::to(&format!(
        "/apps/{}/pages/{}/edit",
        app.slug,
        super::pages::slug_path_segment(&page.slug)
    ))
    .into_response())
}

#[derive(Deserialize, Default)]
pub struct DeleteElementForm {
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_delete_element(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug, element_id)): Path<(String, String, Uuid)>,
    HxRequest(is_htmx): HxRequest,
    jar: CookieJar,
    Form(form): Form<DeleteElementForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = super::pages::decode_slug_segment(&page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if page.author_id != user.id {
        return Err(WebError::Forbidden);
    }
    if Some(element_id) == page.root_element {
        return Err(WebError::BadRequest("cannot delete root element".into()));
    }
    lovely_db::delete_element(&state.pg, element_id).await?;
    lovely_db::snapshot_page(&state.pg, page.id).await?;
    if is_htmx {
        return Ok(hx_ok_preview_stale());
    }
    Ok(Redirect::to(&format!(
        "/apps/{}/pages/{}/edit",
        app.slug,
        super::pages::slug_path_segment(&page.slug)
    ))
    .into_response())
}

#[derive(Deserialize, Default)]
pub struct UpdateElementForm {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_update_element(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug, element_id)): Path<(String, String, Uuid)>,
    jar: CookieJar,
    Form(form): Form<UpdateElementForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = super::pages::decode_slug_segment(&page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if page.author_id != user.id {
        return Err(WebError::Forbidden);
    }
    let payload = json!({"text": form.text.unwrap_or_default()});
    lovely_db::update_element(
        &state.pg,
        element_id,
        lovely_db::ElementPatch {
            tag: None,
            attrs: None,
            payload: Some(payload),
        },
    )
    .await?;
    Ok(Redirect::to(&format!(
        "/apps/{}/pages/{}/edit",
        app.slug,
        super::pages::slug_path_segment(&page.slug)
    ))
    .into_response())
}
