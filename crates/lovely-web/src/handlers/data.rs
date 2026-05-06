//! Collections + records — per-app user-defined data models.

use crate::auth::{csrf, AuthUser, MaybeUser};
use crate::state::AppState;
use crate::views::data as data_views;
use crate::WebError;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum_extra::extract::cookie::CookieJar;
use lovely_db::{
    create_collection, delete_collection, find_app_by_owner_and_slug, find_collection_by_name,
    insert_record, list_collections, list_records,
};
use serde::Deserialize;

pub async fn get_data_index(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(app_slug): Path<String>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let cs = list_collections(&state.pg, app.id).await?;
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = data_views::data_index(&user, &app, &cs, &token).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

#[derive(Deserialize, Default)]
pub struct CreateCollectionForm {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn get_collection_new(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(app_slug): Path<String>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = data_views::collection_new(&user, &app, &token, None).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

pub async fn post_collection_create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(app_slug): Path<String>,
    jar: CookieJar,
    Form(form): Form<CreateCollectionForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if !is_valid_ident(&form.name) {
        return Err(WebError::Unprocessable(
            "name must be 1–40 chars: a-z, 0-9, underscore".into(),
        ));
    }
    create_collection(&state.pg, app.id, &form.name, &[] as &[lovely_db::Field]).await?;
    // Land on the field editor — fields get added one at a time there.
    Ok(Redirect::to(&format!("/apps/{}/data/{}/edit", app.slug, form.name)).into_response())
}

pub async fn get_collection_edit(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, coll_name)): Path<(String, String)>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let coll = find_collection_by_name(&state.pg, app.id, &coll_name)
        .await?
        .ok_or(WebError::NotFound)?;
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = data_views::collection_edit(&user, &app, &coll, &token).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

#[derive(Deserialize, Default)]
pub struct AddFieldForm {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_field_add(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, coll_name)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<AddFieldForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let coll = find_collection_by_name(&state.pg, app.id, &coll_name)
        .await?
        .ok_or(WebError::NotFound)?;
    if !is_valid_ident(&form.name) {
        return Err(WebError::Unprocessable(
            "field name must be 1–40 chars: a-z, 0-9, underscore".into(),
        ));
    }
    let mut fields = coll.typed_fields();
    if fields.iter().any(|f| f.name == form.name) {
        return Err(WebError::Unprocessable(format!(
            "field already exists: {}",
            form.name
        )));
    }
    let field_type = form
        .type_
        .as_deref()
        .and_then(lovely_db::FieldType::from_str)
        .unwrap_or(lovely_db::FieldType::Text);
    fields.push(lovely_db::Field {
        name: form.name,
        field_type,
    });
    lovely_db::collections::set_collection_fields(&state.pg, coll.id, &fields).await?;
    Ok(Redirect::to(&format!("/apps/{}/data/{}/edit", app.slug, coll.name)).into_response())
}

#[derive(Deserialize, Default)]
pub struct RenameCollectionForm {
    #[serde(default)]
    pub new_name: String,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_collection_rename(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, coll_name)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<RenameCollectionForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let coll = find_collection_by_name(&state.pg, app.id, &coll_name)
        .await?
        .ok_or(WebError::NotFound)?;
    if !is_valid_ident(&form.new_name) {
        return Err(WebError::Unprocessable(
            "name must be 1–40 chars: a-z, 0-9, underscore".into(),
        ));
    }
    if form.new_name == coll.name {
        return Ok(
            Redirect::to(&format!("/apps/{}/data/{}/edit", app.slug, coll.name)).into_response(),
        );
    }
    let updated = lovely_db::rename_collection(&state.pg, coll.id, &form.new_name).await?;
    Ok(Redirect::to(&format!("/apps/{}/data/{}/edit", app.slug, updated.name)).into_response())
}

#[derive(Deserialize, Default)]
pub struct RenameFieldForm {
    #[serde(default)]
    pub new_name: String,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_field_rename(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, coll_name, field_name)): Path<(String, String, String)>,
    jar: CookieJar,
    Form(form): Form<RenameFieldForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let coll = find_collection_by_name(&state.pg, app.id, &coll_name)
        .await?
        .ok_or(WebError::NotFound)?;
    if !is_valid_ident(&form.new_name) {
        return Err(WebError::Unprocessable(
            "field name must be 1–40 chars: a-z, 0-9, underscore".into(),
        ));
    }
    if form.new_name == field_name {
        return Ok(
            Redirect::to(&format!("/apps/{}/data/{}/edit", app.slug, coll.name)).into_response(),
        );
    }
    lovely_db::collections::rename_field(&state.pg, coll.id, &field_name, &form.new_name).await?;
    Ok(Redirect::to(&format!("/apps/{}/data/{}/edit", app.slug, coll.name)).into_response())
}

pub async fn post_field_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, coll_name, field_name)): Path<(String, String, String)>,
    jar: CookieJar,
    Form(form): Form<DeleteForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let coll = find_collection_by_name(&state.pg, app.id, &coll_name)
        .await?
        .ok_or(WebError::NotFound)?;
    lovely_db::collections::delete_field(&state.pg, coll.id, &field_name).await?;
    Ok(Redirect::to(&format!("/apps/{}/data/{}/edit", app.slug, coll.name)).into_response())
}

pub async fn get_collection(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, coll_name)): Path<(String, String)>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let coll = find_collection_by_name(&state.pg, app.id, &coll_name)
        .await?
        .ok_or(WebError::NotFound)?;
    let recs = list_records(&state.pg, coll.id).await?;
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = data_views::collection_view(&user, &app, &coll, &recs, &token).into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

pub async fn post_collection_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, coll_name)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<DeleteForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let coll = find_collection_by_name(&state.pg, app.id, &coll_name)
        .await?
        .ok_or(WebError::NotFound)?;
    delete_collection(&state.pg, coll.id).await?;
    Ok(Redirect::to(&format!("/apps/{}/data", app.slug)).into_response())
}

#[derive(Deserialize, Default)]
pub struct DeleteForm {
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_record_create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, coll_name)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<std::collections::HashMap<String, String>>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    let csrf_in = form.get("_csrf").map(|s| s.as_str());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), csrf_in)?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let coll = find_collection_by_name(&state.pg, app.id, &coll_name)
        .await?
        .ok_or(WebError::NotFound)?;
    let mut data = serde_json::Map::new();
    for f in coll.fields() {
        if let Some(v) = form.get(&f) {
            data.insert(f, serde_json::Value::String(v.clone()));
        }
    }
    insert_record(&state.pg, coll.id, serde_json::Value::Object(data)).await?;
    Ok(Redirect::to(&format!("/apps/{}/data/{}", app.slug, coll.name)).into_response())
}

#[derive(Deserialize, Default)]
pub struct DeleteRecordForm {
    pub id: uuid::Uuid,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_record_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, coll_name)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<DeleteRecordForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let coll = find_collection_by_name(&state.pg, app.id, &coll_name)
        .await?
        .ok_or(WebError::NotFound)?;
    // Confirm record belongs to this collection.
    let owns: Option<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM records WHERE id = $1 AND collection_id = $2")
            .bind(form.id)
            .bind(coll.id)
            .fetch_optional(&state.pg)
            .await
            .map_err(lovely_db::DbError::Sqlx)?;
    if owns.is_none() {
        return Err(WebError::NotFound);
    }
    lovely_db::delete_record(&state.pg, form.id).await?;
    Ok(Redirect::to(&format!("/apps/{}/data/{}", app.slug, coll.name)).into_response())
}

/// Public form-submit endpoint: anyone can post to a published page's
/// form to create a record. Owner gates writes via the `bind_collection`
/// data attribute the form was rendered with — the path itself enforces
/// the collection name.
pub async fn post_public_submit(
    State(state): State<AppState>,
    MaybeUser(_viewer): MaybeUser,
    Path((username, slug, coll_name)): Path<(String, String, String)>,
    jar: CookieJar,
    Form(form): Form<std::collections::HashMap<String, String>>,
) -> Result<Response, WebError> {
    // Resolve the user's default app + verify the page exists + published.
    let Some((_owner, app)) =
        lovely_db::find_default_app_for_username(&state.pg, &username).await?
    else {
        return Err(WebError::NotFound);
    };
    let real_slug = super::pages::decode_slug_segment(&slug);
    let page = lovely_db::find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    if page.published_at.is_none() {
        return Err(WebError::NotFound);
    }
    let coll = find_collection_by_name(&state.pg, app.id, &coll_name)
        .await?
        .ok_or(WebError::NotFound)?;
    // Enforce CSRF the same way as authed endpoints — public forms still
    // ship a token (rendered into the page) and the cookie comes along.
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    let csrf_in = form.get("_csrf").map(|s| s.as_str());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), csrf_in)?;
    let mut data = serde_json::Map::new();
    for f in coll.fields() {
        if let Some(v) = form.get(&f) {
            data.insert(f, serde_json::Value::String(v.clone()));
        }
    }
    insert_record(&state.pg, coll.id, serde_json::Value::Object(data)).await?;
    let redirect = if real_slug.is_empty() {
        format!("/{username}")
    } else {
        format!("/{username}/{real_slug}")
    };
    Ok((StatusCode::SEE_OTHER, [("Location", redirect)]).into_response())
}

fn is_valid_ident(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
