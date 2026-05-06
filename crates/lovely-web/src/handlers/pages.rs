use crate::auth::{csrf, AuthUser, MaybeUser};
use crate::state::AppState;
use crate::views::builder::{
    builder, BuilderCtx, InspectorTab, Selection,
};
use crate::views::pages as pages_views;
use crate::WebError;
use axum::extract::{Path, Query, State};
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

#[derive(Deserialize, Default)]
pub struct EditQuery {
    #[serde(default)]
    pub sel: Option<String>,
    #[serde(default)]
    pub tab: Option<String>,
}

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

#[derive(Deserialize, Default)]
pub struct CheckPageSlugQuery {
    pub slug: Option<String>,
}

/// Live-validation for the New-Page slug field. Empty slug is allowed
/// (it's the home page) but only one home page per app — flag if taken.
pub async fn get_check_page_slug(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(app_slug): Path<String>,
    Query(q): Query<CheckPageSlugQuery>,
) -> Result<Response, WebError> {
    let app = find_app_by_owner_and_slug(&state.pg, user.id, &app_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let raw = q.slug.unwrap_or_default();
    let slug = raw.trim();
    // Validate format. Empty slug is the home page — allowed.
    if !slug.is_empty()
        && !slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Ok(axum::response::Html(
            r#"<span class="slug-error">Slugs can only contain lowercase letters, digits, and dashes.</span>"#,
        )
        .into_response());
    }
    let taken = find_page_by_app_and_slug(&state.pg, app.id, slug)
        .await?
        .is_some();
    let display = if slug.is_empty() {
        "the home page".to_string()
    } else {
        format!("\"{}\"", slug)
    };
    let html = if taken {
        format!(
            r#"<span class="slug-error">{} is already used in this app.</span>"#,
            display
        )
    } else if slug.is_empty() {
        r#"<span class="slug-ok">Will be the home page.</span>"#.to_string()
    } else {
        format!(r#"<span class="slug-ok">{} is available.</span>"#, display)
    };
    Ok(axum::response::Html(html).into_response())
}

pub async fn get_page_edit(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug)): Path<(String, String)>,
    Query(q): Query<EditQuery>,
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
    let collections = lovely_db::list_collections(&state.pg, app.id).await?;
    let root = page.root_element.unwrap_or_default();
    let selection = Selection::from_query(q.sel.as_deref(), root);
    let tab = InspectorTab::from_query(q.tab.as_deref());
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = builder(BuilderCtx {
        user: &user,
        app: &app,
        page: &page,
        elements: &rows,
        collections: &collections,
        selection,
        tab,
        csrf_token: &token,
    })
    .into_string();
    Ok((jar, axum::response::Html(html)).into_response())
}

#[derive(Deserialize, Default)]
pub struct HeadForm {
    #[serde(default)]
    pub head_html: String,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_page_head(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<HeadForm>,
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
    let cleaned = sanitize_head_html(&form.head_html);
    lovely_db::update_page_head(&state.pg, page.id, &cleaned).await?;
    Ok(Redirect::to(&format!(
        "/apps/{}/pages/{}/edit",
        app.slug,
        slug_path_segment(&page.slug)
    ))
    .into_response())
}

#[derive(Deserialize, Default)]
pub struct AccessForm {
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub unlisted: Option<String>,
    #[serde(default)]
    pub _csrf: Option<String>,
}

pub async fn post_page_access(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<AccessForm>,
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
    let unlisted = matches!(form.unlisted.as_deref(), Some("on") | Some("true"));
    let hash = if form.password.is_empty() {
        None
    } else {
        Some(crate::auth::password::hash_password(&form.password)?)
    };
    lovely_db::update_page_access(&state.pg, page.id, hash.as_deref(), unlisted).await?;
    Ok(Redirect::to(&format!(
        "/apps/{}/pages/{}/edit",
        app.slug,
        slug_path_segment(&page.slug)
    ))
    .into_response())
}

#[derive(Deserialize, Default)]
pub struct UnlockForm {
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub _csrf: Option<String>,
}

/// Sets a page-unlock cookie for the (username, slug) pair if the
/// password matches. The cookie is checked in `render_public`.
pub async fn post_page_unlock(
    State(state): State<AppState>,
    Path((username, slug)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<UnlockForm>,
) -> Result<Response, WebError> {
    let cookie_token = jar.get(csrf::CSRF_COOKIE).map(|c| c.value().to_string());
    csrf::verify_token(cookie_token.as_deref().unwrap_or(""), form._csrf.as_deref())?;
    let Some((_owner, app)) = find_default_app_for_username(&state.pg, &username).await? else {
        return Err(WebError::NotFound);
    };
    let real_slug = decode_slug_segment(&slug);
    let page = find_page_by_app_and_slug(&state.pg, app.id, &real_slug)
        .await?
        .ok_or(WebError::NotFound)?;
    let Some(hash) = page.password_hash.as_deref() else {
        return Err(WebError::NotFound);
    };
    if !crate::auth::password::verify_password(&form.password, hash) {
        return Err(WebError::Unprocessable("incorrect password".into()));
    }
    let target = if real_slug.is_empty() {
        format!("/{username}")
    } else {
        format!("/{username}/{real_slug}")
    };
    let cookie_name = format!("lovely_unlock_{}", page.id);
    let cookie = format!(
        "{cookie_name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400",
        value = page.id
    );
    Ok((
        StatusCode::SEE_OTHER,
        [("Location", target.as_str()), ("Set-Cookie", cookie.as_str())],
        "",
    )
        .into_response())
}

pub async fn get_page_preview(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, page_slug)): Path<(String, String)>,
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
    let rendered = tree.render();
    // Minimal owner-only render. No editor chrome, draft visible. The
    // user's CSS lives inside the iframe, so it can't fight ours.
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title>\
         <link rel=\"stylesheet\" href=\"/static/style.css\"></head><body class=\"public\">{}</body></html>",
        html_escape(&page.title),
        rendered.into_string()
    );
    Ok(axum::response::Html(html).into_response())
}

/// For each element carrying `data-lovely-repeat=<collection>`, take
/// its first child as a template and duplicate it once per record.
/// `{{field}}` in the template's text gets replaced with the field
/// value. The template element itself is removed; clones replace it.
async fn expand_repeaters(
    pg: &sqlx::PgPool,
    app_id: uuid::Uuid,
    rows: &mut Vec<lovely_tree::ElementRow>,
) -> Result<(), WebError> {
    use lovely_tree::{ElementRow, ElementUuid};
    use std::collections::HashMap;
    // Snapshot of repeater-bearing elements; we mutate `rows` after.
    let repeaters: Vec<(uuid::Uuid, String)> = rows
        .iter()
        .filter_map(|r| {
            r.attrs_json
                .get("data-lovely-repeat")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|coll| (r.id.into_inner(), coll.to_string()))
        })
        .collect();
    if repeaters.is_empty() {
        return Ok(());
    }

    // Quick lookup: parent_id → ordered children (by sibling chain).
    fn ordered_children(rows: &[ElementRow], parent: uuid::Uuid) -> Vec<uuid::Uuid> {
        let kids: Vec<&ElementRow> = rows
            .iter()
            .filter(|r| r.parent_id.map(|p| p.into_inner()) == Some(parent))
            .collect();
        let mut chain: Vec<uuid::Uuid> = Vec::new();
        if let Some(first) = kids.iter().find(|r| r.prev_sibling.is_none()) {
            chain.push(first.id.into_inner());
            loop {
                let last = *chain.last().unwrap();
                match kids
                    .iter()
                    .find(|r| r.prev_sibling.map(|p| p.into_inner()) == Some(last))
                {
                    Some(next) => chain.push(next.id.into_inner()),
                    None => break,
                }
            }
        }
        chain
    }

    let mut coll_cache: HashMap<String, Vec<serde_json::Value>> = HashMap::new();

    for (parent_id, coll_name) in repeaters {
        // Resolve records.
        let records: Vec<serde_json::Value> = if let Some(v) = coll_cache.get(&coll_name) {
            v.clone()
        } else {
            let coll = lovely_db::find_collection_by_name(pg, app_id, &coll_name).await?;
            let v = match coll {
                Some(c) => lovely_db::list_records(pg, c.id)
                    .await?
                    .into_iter()
                    .map(|r| r.data_json)
                    .collect(),
                None => Vec::new(),
            };
            coll_cache.insert(coll_name.clone(), v.clone());
            v
        };

        // Find the template subtree (first child of this repeater).
        let kids = ordered_children(rows, parent_id);
        let Some(template_id) = kids.first().copied() else {
            continue;
        };

        // Snapshot template's subtree (template + descendants, ordered).
        let template_rows = collect_subtree(rows, template_id);

        // Remove the original subtree from rows.
        let to_remove: std::collections::HashSet<uuid::Uuid> =
            template_rows.iter().map(|r| r.id.into_inner()).collect();
        rows.retain(|r| !to_remove.contains(&r.id.into_inner()));

        // For each record, clone the subtree with new uuids, interpolating
        // `{{field}}` in text + attr values. Chain clones as siblings under
        // `parent_id` in record order.
        let mut prev_root_clone: Option<uuid::Uuid> = None;
        for record in &records {
            let mut id_map: HashMap<uuid::Uuid, uuid::Uuid> = HashMap::new();
            for orig in &template_rows {
                id_map.insert(orig.id.into_inner(), uuid::Uuid::new_v4());
            }
            for orig in &template_rows {
                let new_id = id_map[&orig.id.into_inner()];
                let new_parent = if orig.id.into_inner() == template_id {
                    Some(ElementUuid(parent_id))
                } else {
                    orig.parent_id
                        .map(|p| ElementUuid(*id_map.get(&p.into_inner()).unwrap_or(&p.into_inner())))
                };
                let new_prev = if orig.id.into_inner() == template_id {
                    prev_root_clone.map(ElementUuid)
                } else {
                    orig.prev_sibling
                        .map(|p| ElementUuid(*id_map.get(&p.into_inner()).unwrap_or(&p.into_inner())))
                };
                let new_text = orig
                    .text
                    .as_ref()
                    .map(|t| interpolate(t, record));
                let new_attrs = interpolate_attrs(&orig.attrs_json, record);
                rows.push(ElementRow {
                    id: ElementUuid(new_id),
                    parent_id: new_parent,
                    prev_sibling: new_prev,
                    tag: orig.tag.clone(),
                    attrs_json: new_attrs,
                    text: new_text,
                });
            }
            prev_root_clone = Some(id_map[&template_id]);
        }
    }
    Ok(())
}

fn collect_subtree(
    rows: &[lovely_tree::ElementRow],
    root: uuid::Uuid,
) -> Vec<lovely_tree::ElementRow> {
    use std::collections::HashSet;
    let mut included: HashSet<uuid::Uuid> = HashSet::new();
    included.insert(root);
    // BFS through parent pointers — repeat until stable.
    loop {
        let before = included.len();
        for r in rows {
            if let Some(pid) = r.parent_id {
                if included.contains(&pid.into_inner()) {
                    included.insert(r.id.into_inner());
                }
            }
        }
        if included.len() == before {
            break;
        }
    }
    rows.iter()
        .filter(|r| included.contains(&r.id.into_inner()))
        .cloned()
        .collect()
}

fn interpolate(template: &str, record: &serde_json::Value) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = template[i + 2..].find("}}") {
                let key = template[i + 2..i + 2 + end].trim();
                if let Some(v) = record.get(key).and_then(|v| v.as_str()) {
                    out.push_str(v);
                } else if let Some(v) = record.get(key) {
                    out.push_str(&v.to_string());
                }
                i += 2 + end + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn interpolate_attrs(attrs: &serde_json::Value, record: &serde_json::Value) -> serde_json::Value {
    match attrs {
        serde_json::Value::Object(m) => {
            let mut out = serde_json::Map::new();
            for (k, v) in m {
                if let Some(s) = v.as_str() {
                    out.insert(k.clone(), serde_json::Value::String(interpolate(s, record)));
                } else {
                    out.insert(k.clone(), v.clone());
                }
            }
            serde_json::Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Walks the rows and, for any element carrying `data-lovely-bind`,
/// replaces the element's text with the value pulled from the
/// referenced collection's first record. Cheap O(n × bindings) for
/// now — caches collection lookups within the call.
async fn resolve_bindings(
    pg: &sqlx::PgPool,
    app_id: uuid::Uuid,
    rows: &mut [lovely_tree::ElementRow],
) -> Result<(), WebError> {
    use std::collections::HashMap;
    let mut cache: HashMap<String, Option<serde_json::Value>> = HashMap::new();
    for row in rows.iter_mut() {
        let Some(bind) = row
            .attrs_json
            .get("data-lovely-bind")
            .and_then(|v| v.as_str())
        else {
            continue;
        };
        // `coll` alone (no `.field`) is a collection-context bind — no
        // direct value substitution; only relevant for repeats and
        // future interpolation in descendants. Skip it here.
        let Some((coll_name, field)) = bind.split_once('.') else {
            continue;
        };
        if field.is_empty() {
            continue;
        }
        let key = coll_name.to_string();
        let first_row_data = if let Some(v) = cache.get(&key) {
            v.clone()
        } else {
            let coll = lovely_db::find_collection_by_name(pg, app_id, coll_name).await?;
            let v = match coll {
                Some(c) => {
                    let recs = lovely_db::list_records(pg, c.id).await?;
                    recs.into_iter().next().map(|r| r.data_json)
                }
                None => None,
            };
            cache.insert(key, v.clone());
            v
        };
        if let Some(data) = first_row_data {
            if let Some(s) = data.get(field).and_then(|v| v.as_str()) {
                match row.tag.as_str() {
                    // <input> is void — populate the value attribute.
                    "input" => {
                        if let serde_json::Value::Object(m) = &mut row.attrs_json {
                            m.insert("value".into(), serde_json::Value::String(s.to_string()));
                        } else {
                            row.attrs_json = serde_json::json!({ "value": s });
                        }
                    }
                    // <textarea> uses its inner text as the value.
                    "textarea" => row.text = Some(s.to_string()),
                    // <select> — too involved; skip for now.
                    "select" => {}
                    _ => row.text = Some(s.to_string()),
                }
            }
        }
    }
    Ok(())
}

/// Strips dangerous chunks from user-supplied <head> HTML. Allows
/// `<meta>`, `<link>`, `<style>`, plain text comments. Drops anything
/// that looks like an event handler or a `<script>` block.
fn sanitize_head_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let lower = s.to_ascii_lowercase();
    let mut i = 0;
    while i < s.len() {
        // Drop <script>...</script> entirely (including the close tag).
        if lower[i..].starts_with("<script") {
            if let Some(end) = lower[i..].find("</script>") {
                i += end + "</script>".len();
                continue;
            } else {
                break; // unterminated; bail
            }
        }
        // Drop on*= attributes.
        if lower[i..].starts_with(" on") {
            if let Some(eq) = lower[i + 1..].find('=') {
                let after = i + 1 + eq + 1;
                let mut j = after;
                let bytes = s.as_bytes();
                let quote = if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                    let q = bytes[j];
                    j += 1;
                    Some(q)
                } else {
                    None
                };
                while j < bytes.len() {
                    let b = bytes[j];
                    match (quote, b) {
                        (Some(q), c) if c == q => {
                            j += 1;
                            break;
                        }
                        (None, b' ') | (None, b'>') => break,
                        _ => j += 1,
                    }
                }
                i = j;
                continue;
            }
        }
        out.push(s.as_bytes()[i] as char);
        i += 1;
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
    let Some((owner, app)) = find_default_app_for_username(&state.pg, username).await? else {
        return Err(WebError::NotFound);
    };
    let is_owner = viewer.as_ref().map(|v| v.id == owner.id).unwrap_or(false);
    let page_opt = find_page_by_app_and_slug(&state.pg, app.id, slug).await?;

    // No page at this slug — fall through to a profile listing if the
    // user has published their profile (or the viewer is the owner).
    let Some(page) = page_opt else {
        if slug.is_empty() && (is_owner || owner.public_published_at.is_some()) {
            let apps = if is_owner {
                lovely_db::list_apps_by_owner(&state.pg, owner.id).await?
            } else {
                lovely_db::list_published_apps_by_owner(&state.pg, owner.id).await?
            };
            let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
            let html =
                pages_views::user_profile(&owner, &apps, viewer.as_ref(), &token).into_string();
            return Ok((jar, axum::response::Html(html)).into_response());
        }
        return Err(WebError::NotFound);
    };

    // Owner always sees their page. Anonymous viewers may need to
    // unlock (password), be hidden (unlisted), or be turned away if
    // the page is still a draft.
    if !is_owner {
        if page.published_at.is_none() {
            return Err(WebError::NotFound);
        }
        if page.unlisted {
            return Err(WebError::NotFound);
        }
        if let Some(_hash) = page.password_hash.as_ref() {
            let unlock_cookie = format!("lovely_unlock_{}", page.id);
            let unlocked = jar
                .get(&unlock_cookie)
                .map(|c| c.value() == page.id.to_string())
                .unwrap_or(false);
            if !unlocked {
                let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
                let html = pages_views::password_gate(&page, username, slug, &token).into_string();
                return Ok((
                    StatusCode::UNAUTHORIZED,
                    jar,
                    axum::response::Html(html),
                )
                    .into_response());
            }
        }
    }

    let mut rows = lovely_db::load_elements_for_page(&state.pg, page.id).await?;
    expand_repeaters(&state.pg, app.id, &mut rows).await?;
    resolve_bindings(&state.pg, app.id, &mut rows).await?;
    let tree = Tree::from_db_rows(&rows)?;
    let rendered = tree.render();
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let html = pages_views::published_page(
        viewer.as_ref(),
        owner.id,
        &app.slug,
        &app.theme_json,
        &page,
        rendered,
        &token,
    )
    .into_string();
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
