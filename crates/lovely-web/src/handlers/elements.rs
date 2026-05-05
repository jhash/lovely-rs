use crate::auth::{csrf, AuthUser};
use crate::state::AppState;
use crate::WebError;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use lovely_db::{find_app_by_owner_and_slug, find_page_by_app_and_slug};
use lovely_tree::ElementTag;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

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
