//! Builder fragment routes — tree sidebar, inspector, PATCH, MOVE.
//!
//! All return small HTML fragments (or 200 with HX-Trigger) so the
//! editor can swap pieces in place without full reloads.

use crate::auth::{csrf, AuthUser};
use crate::state::AppState;
use crate::views::builder::{
    inspector_fragment, tree_fragment, BuilderCtx, InspectorTab, Selection,
};
use crate::WebError;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use lovely_db::{find_app_by_owner_and_slug, find_page_by_app_and_slug, ElementPatch};
use lovely_tree::AttrName;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize, Default)]
pub struct SelTabQuery {
    #[serde(default)]
    pub sel: Option<String>,
    #[serde(default)]
    pub tab: Option<String>,
}

pub async fn get_tree_fragment(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug)): Path<(String, String)>,
    Query(q): Query<SelTabQuery>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = super::pages::decode_slug_segment(&page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let rows = lovely_db::load_elements_for_page(&state.pg, page.id).await?;
    let collections = lovely_db::list_collections(&state.pg, app.id).await?;
    let root = page.root_element.unwrap_or_default();
    let selection = Selection::from_query(q.sel.as_deref(), root);
    let tab = InspectorTab::from_query(q.tab.as_deref());
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let frag = tree_fragment(&BuilderCtx {
        user: &user,
        app: &app,
        page: &page,
        elements: &rows,
        collections: &collections,
        selection,
        tab,
        csrf_token: &token,
    });
    Ok((jar, Html(frag.into_string())).into_response())
}

pub async fn get_inspector_fragment(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug)): Path<(String, String)>,
    Query(q): Query<SelTabQuery>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let real_slug = super::pages::decode_slug_segment(&page_slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let rows = lovely_db::load_elements_for_page(&state.pg, page.id).await?;
    let collections = lovely_db::list_collections(&state.pg, app.id).await?;
    let root = page.root_element.unwrap_or_default();
    let selection = Selection::from_query(q.sel.as_deref(), root);
    let tab = InspectorTab::from_query(q.tab.as_deref());
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let frag = inspector_fragment(&BuilderCtx {
        user: &user,
        app: &app,
        page: &page,
        elements: &rows,
        collections: &collections,
        selection,
        tab,
        csrf_token: &token,
    });
    Ok((jar, Html(frag.into_string())).into_response())
}

#[derive(Deserialize, Default)]
pub struct PatchElementForm {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub attr_name: Option<String>,
    #[serde(default)]
    pub attr_value: Option<String>,
    /// Bind this element to `{collection}.{field}` from the app's data
    /// store. The binding is stored as a `data-lovely-bind` attribute
    /// (so it renders harmlessly into HTML and is observable in the DOM)
    /// and resolved at public-render time before `Tree::render` runs.
    #[serde(default)]
    pub binding_collection: Option<String>,
    #[serde(default)]
    pub binding_field: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn patch_element(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug, element_id)): Path<(String, String, Uuid)>,
    jar: CookieJar,
    Form(form): Form<PatchElementForm>,
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

    // Confirm element belongs to this page.
    let owns: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM elements WHERE id = $1 AND page_id = $2")
            .bind(element_id)
            .bind(page.id)
            .fetch_optional(&state.pg)
            .await
            .map_err(lovely_db::DbError::Sqlx)?;
    if owns.is_none() {
        return Err(WebError::NotFound);
    }

    let mut patch = ElementPatch::default();

    // Apply attr update if present. Validate name through AttrName so we
    // pick up the on*/hx-* denylist for free.
    if let Some(name) = form.attr_name.as_deref().filter(|s| !s.is_empty()) {
        if AttrName::new(name).is_err() {
            return Err(WebError::Unprocessable(format!(
                "invalid attribute name: {name}"
            )));
        }
        let value = form.attr_value.clone().unwrap_or_default();
        // Read current attrs, merge, write back.
        let current: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT attrs FROM elements WHERE id = $1")
                .bind(element_id)
                .fetch_optional(&state.pg)
                .await
                .map_err(lovely_db::DbError::Sqlx)?;
        let mut obj = match current {
            Some(serde_json::Value::Object(o)) => o,
            _ => serde_json::Map::new(),
        };
        if value.is_empty() {
            obj.remove(name);
        } else {
            obj.insert(name.to_string(), serde_json::Value::String(value));
        }
        patch.attrs = Some(serde_json::Value::Object(obj));
    }

    // Apply text update if present.
    if let Some(text) = form.text {
        patch.payload = Some(json!({ "text": text }));
    }

    // Apply binding update if present. Stored as `data-lovely-bind` attr.
    if let Some(coll) = form.binding_collection.as_deref() {
        let field = form.binding_field.as_deref().unwrap_or("");
        let bind_value = if coll.is_empty() {
            String::new()
        } else {
            format!("{coll}.{field}")
        };
        // Merge into existing attrs (or the freshly-merged map from above).
        let merged = match patch.attrs.take() {
            Some(serde_json::Value::Object(o)) => o,
            _ => {
                let current: Option<serde_json::Value> =
                    sqlx::query_scalar("SELECT attrs FROM elements WHERE id = $1")
                        .bind(element_id)
                        .fetch_optional(&state.pg)
                        .await
                        .map_err(lovely_db::DbError::Sqlx)?;
                match current {
                    Some(serde_json::Value::Object(o)) => o,
                    _ => serde_json::Map::new(),
                }
            }
        };
        let mut merged = merged;
        if bind_value.is_empty() {
            merged.remove("data-lovely-bind");
        } else {
            merged.insert(
                "data-lovely-bind".to_string(),
                serde_json::Value::String(bind_value),
            );
        }
        patch.attrs = Some(serde_json::Value::Object(merged));
    }

    if patch.attrs.is_some() || patch.payload.is_some() {
        lovely_db::update_element(&state.pg, element_id, patch).await?;
    }

    let mut headers = HeaderMap::new();
    headers.insert("HX-Trigger", HeaderValue::from_static("preview-stale"));
    Ok((StatusCode::OK, headers, Html("")).into_response())
}

#[derive(Deserialize, Default)]
pub struct MoveElementForm {
    pub parent_id: Uuid,
    #[serde(default)]
    pub prev_sibling: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_move_element(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug, element_id)): Path<(String, String, Uuid)>,
    jar: CookieJar,
    Form(form): Form<MoveElementForm>,
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

    // Element must belong to this page.
    let owns: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM elements WHERE id = $1 AND page_id = $2")
            .bind(element_id)
            .bind(page.id)
            .fetch_optional(&state.pg)
            .await
            .map_err(lovely_db::DbError::Sqlx)?;
    if owns.is_none() {
        return Err(WebError::NotFound);
    }

    // Cycle check: walking parent_id chain from form.parent_id back up
    // must never hit element_id.
    let mut cursor = Some(form.parent_id);
    while let Some(p) = cursor {
        if p == element_id {
            return Err(WebError::Unprocessable(
                "cannot move element under itself".into(),
            ));
        }
        let next: Option<(Option<Uuid>,)> =
            sqlx::query_as("SELECT parent_id FROM elements WHERE id = $1 AND page_id = $2")
                .bind(p)
                .bind(page.id)
                .fetch_optional(&state.pg)
                .await
                .map_err(lovely_db::DbError::Sqlx)?;
        match next {
            Some((parent_of_p,)) => cursor = parent_of_p,
            None => return Err(WebError::NotFound),
        }
    }

    let prev_sibling = form
        .prev_sibling
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| WebError::BadRequest("invalid prev_sibling".into()))?;

    sqlx::query(
        "UPDATE elements SET parent_id = $2, prev_sibling = $3, updated_at = now() \
         WHERE id = $1",
    )
    .bind(element_id)
    .bind(form.parent_id)
    .bind(prev_sibling)
    .execute(&state.pg)
    .await
    .map_err(lovely_db::DbError::Sqlx)?;

    let mut headers = HeaderMap::new();
    headers.insert("HX-Trigger", HeaderValue::from_static("preview-stale"));
    Ok((StatusCode::OK, headers, Html("")).into_response())
}
