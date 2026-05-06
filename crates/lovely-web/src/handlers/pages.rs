use crate::auth::{csrf, AuthUser, MaybeUser};
use crate::state::AppState;
use crate::views::builder::{builder, BuilderCtx, InspectorTab, Selection};
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
    let markup = pages_views::pages_new(&user, &app, &token, None);
    Ok((jar, markup).into_response())
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
        let markup = pages_views::pages_new(
            &user,
            &app,
            &token,
            Some("Slug must be empty (home page) or 1–80 chars: a-z, 0-9, hyphen"),
        );
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, jar, markup).into_response());
    }
    if form.title.trim().is_empty() {
        let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
        let markup = pages_views::pages_new(&user, &app, &token, Some("Title is required"));
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, jar, markup).into_response());
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
            let markup = pages_views::pages_new(
                &user,
                &app,
                &token,
                Some("That slug is already used in this app"),
            );
            Ok((StatusCode::UNPROCESSABLE_ENTITY, jar, markup).into_response())
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
    let markup = builder(BuilderCtx {
        user: &user,
        app: &app,
        page: &page,
        elements: &rows,
        collections: &collections,
        selection,
        tab,
        csrf_token: &token,
    });
    Ok((jar, markup).into_response())
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
        [
            ("Location", target.as_str()),
            ("Set-Cookie", cookie.as_str()),
        ],
        "",
    )
        .into_response())
}

/// For each element carrying `data-lovely-repeat=<collection>`, take
/// its first child as a template and duplicate it once per record.
/// `{{field}}` in the template's text gets replaced with the field
/// value. The template element itself is removed; clones replace it.
pub(crate) async fn expand_repeaters(
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
                    orig.parent_id.map(|p| {
                        ElementUuid(*id_map.get(&p.into_inner()).unwrap_or(&p.into_inner()))
                    })
                };
                let new_prev = if orig.id.into_inner() == template_id {
                    prev_root_clone.map(ElementUuid)
                } else {
                    orig.prev_sibling.map(|p| {
                        ElementUuid(*id_map.get(&p.into_inner()).unwrap_or(&p.into_inner()))
                    })
                };
                let new_text = orig.text.as_ref().map(|t| interpolate(t, record));
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

/// Auto-wire forms whose descendants carry `data-lovely-source`. Walks
/// the tree, groups source descendants by their nearest <form> parent,
/// and rewrites the form's `action`/`method` to the public submit
/// endpoint. Each source descendant's `name` attr is mapped to the
/// field name so the browser submits with the right keys. Also injects
/// a synthetic `<input type="hidden" name="_csrf">` child carrying the
/// active CSRF token so the public submit endpoint accepts the post.
/// Idempotent: it overwrites existing action/method only when sources
/// are present.
pub(crate) fn auto_wire_forms(
    rows: &mut Vec<lovely_tree::ElementRow>,
    username: &str,
    page_slug: &str,
    csrf_token: &str,
) {
    use lovely_tree::ElementUuid;
    use std::collections::HashMap;
    // Build child→parent map for ancestor walks.
    let parent_of: HashMap<uuid::Uuid, uuid::Uuid> = rows
        .iter()
        .filter_map(|r| r.parent_id.map(|p| (r.id.into_inner(), p.into_inner())))
        .collect();
    let id_to_idx: HashMap<uuid::Uuid, usize> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (r.id.into_inner(), i))
        .collect();
    // Walk: for every row carrying data-lovely-source, find its
    // nearest <form> ancestor (or none).
    let mut form_to_source_collection: HashMap<uuid::Uuid, String> = HashMap::new();
    let mut row_field_for_source: Vec<(usize, String)> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let Some(source) = row
            .attrs_json
            .get("data-lovely-source")
            .and_then(|v| v.as_str())
        else {
            continue;
        };
        let (coll, field) = match source.split_once('.') {
            Some((c, f)) if !f.is_empty() => (c.to_string(), f.to_string()),
            _ => continue,
        };
        // Walk up to find the form ancestor.
        let mut cur = row.parent_id.map(|p| p.into_inner());
        let mut form_id: Option<uuid::Uuid> = None;
        while let Some(c) = cur {
            if let Some(&idx) = id_to_idx.get(&c) {
                if rows[idx].tag == "form" {
                    form_id = Some(c);
                    break;
                }
            }
            cur = parent_of.get(&c).copied();
        }
        if let Some(fid) = form_id {
            form_to_source_collection.insert(fid, coll);
            row_field_for_source.push((i, field));
        }
    }
    // Rewrite each source row's `name` attribute.
    for (i, field) in row_field_for_source {
        let attrs = match &mut rows[i].attrs_json {
            serde_json::Value::Object(m) => m,
            _ => continue,
        };
        attrs.insert("name".into(), serde_json::Value::String(field));
    }
    // Rewrite each form's action + method, and append a CSRF hidden
    // input as a child of each wired form.
    let segment = if page_slug.is_empty() {
        "~home"
    } else {
        page_slug
    };
    let mut csrf_inputs: Vec<lovely_tree::ElementRow> = Vec::new();
    for (form_id, coll) in form_to_source_collection {
        if let Some(&idx) = id_to_idx.get(&form_id) {
            let action = format!("/p/{username}/{segment}/_submit/{coll}");
            let attrs = match &mut rows[idx].attrs_json {
                serde_json::Value::Object(m) => m,
                _ => continue,
            };
            attrs.insert("action".into(), serde_json::Value::String(action));
            attrs.insert("method".into(), serde_json::Value::String("post".into()));
            // Synthetic CSRF hidden input — sits as the LAST child of
            // the form via prev_sibling = current tail (or None if
            // the form has no children yet).
            let new_id = ElementUuid::new_v4();
            let attrs_json = serde_json::json!({
                "type": "hidden",
                "name": "_csrf",
                "value": csrf_token,
            });
            // Find the form's current tail child (no successor pointing at it).
            let tail = rows
                .iter()
                .filter(|r| r.parent_id.map(|p| p.into_inner()) == Some(form_id))
                .find(|r| {
                    !rows.iter().any(|other| {
                        other.prev_sibling.map(|p| p.into_inner()) == Some(r.id.into_inner())
                    })
                })
                .map(|r| r.id);
            csrf_inputs.push(lovely_tree::ElementRow {
                id: new_id,
                parent_id: Some(ElementUuid(form_id)),
                prev_sibling: tail,
                tag: "input".into(),
                attrs_json,
                text: None,
            });
        }
    }
    rows.extend(csrf_inputs);
}

/// Cross-collection interpolation: replaces `{{coll.field}}` tokens
/// in every row's text + attribute values with the first record's
/// value from the named collection. Distinct from
/// [`expand_repeaters`], which works against per-iteration records.
/// Caches the per-collection first record so we don't re-fetch.
pub(crate) async fn interpolate_collection_refs(
    pg: &sqlx::PgPool,
    app_id: uuid::Uuid,
    rows: &mut [lovely_tree::ElementRow],
) -> Result<(), WebError> {
    use std::collections::{HashMap, HashSet};
    // Collect every `{{coll.field}}` referenced anywhere in the tree
    // so we know which collections to fetch.
    let mut seen: HashSet<String> = HashSet::new();
    for row in rows.iter() {
        if let Some(t) = &row.text {
            collect_refs(t, &mut seen);
        }
        if let serde_json::Value::Object(m) = &row.attrs_json {
            for (_, v) in m {
                if let Some(s) = v.as_str() {
                    collect_refs(s, &mut seen);
                }
            }
        }
    }
    if seen.is_empty() {
        return Ok(());
    }
    // Resolve each referenced collection's first record once.
    let mut cache: HashMap<String, serde_json::Value> = HashMap::new();
    for coll_name in seen {
        let coll = lovely_db::find_collection_by_name(pg, app_id, &coll_name).await?;
        if let Some(c) = coll {
            if let Some(rec) = lovely_db::list_records(pg, c.id).await?.into_iter().next() {
                cache.insert(coll_name, rec.data_json);
            }
        }
    }
    if cache.is_empty() {
        return Ok(());
    }
    for row in rows.iter_mut() {
        if let Some(t) = row.text.take() {
            row.text = Some(interpolate_named(&t, &cache));
        }
        if let serde_json::Value::Object(m) = &mut row.attrs_json {
            for (_, v) in m.iter_mut() {
                if let Some(s) = v.as_str() {
                    *v = serde_json::Value::String(interpolate_named(s, &cache));
                }
            }
        }
    }
    Ok(())
}

fn collect_refs(s: &str, out: &mut std::collections::HashSet<String>) {
    let mut i = 0;
    let bytes = s.as_bytes();
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = s[i + 2..].find("}}") {
                let inner = s[i + 2..i + 2 + end].trim();
                if let Some((coll, _field)) = inner.split_once('.') {
                    out.insert(coll.trim().to_string());
                }
                i += 2 + end + 2;
                continue;
            }
        }
        i += 1;
    }
}

fn interpolate_named(
    s: &str,
    cache: &std::collections::HashMap<String, serde_json::Value>,
) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = s[i + 2..].find("}}") {
                let inner = s[i + 2..i + 2 + end].trim();
                if let Some((coll, field)) = inner.split_once('.') {
                    if let Some(rec) = cache.get(coll.trim()) {
                        if let Some(v) = rec.get(field.trim()).and_then(|v| v.as_str()) {
                            out.push_str(v);
                            i += 2 + end + 2;
                            continue;
                        } else if let Some(v) = rec.get(field.trim()) {
                            out.push_str(&v.to_string());
                            i += 2 + end + 2;
                            continue;
                        }
                    }
                }
                // Unresolved → leave the literal `{{...}}` in place.
                out.push_str(&s[i..i + 2 + end + 2]);
                i += 2 + end + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
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
pub(crate) async fn resolve_bindings(
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

pub async fn get_public_user_root(
    State(state): State<AppState>,
    MaybeUser(viewer): MaybeUser,
    Path(username): Path<String>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    render_public(&state, viewer, &username, None, "", jar).await
}

pub async fn get_public_user_page(
    State(state): State<AppState>,
    MaybeUser(viewer): MaybeUser,
    Path((username, slug)): Path<(String, String)>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let real_slug = decode_slug_segment(&slug);
    render_public(&state, viewer, &username, None, &real_slug, jar).await
}

/// `/{username}/{app_slug}/{page_slug}` — public render scoped to a
/// specific (non-default) app of the user. The default app keeps the
/// shorter `/{username}/{page_slug}` shape.
pub async fn get_public_user_app_page(
    State(state): State<AppState>,
    MaybeUser(viewer): MaybeUser,
    Path((username, app_slug, page_slug)): Path<(String, String, String)>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let real_slug = decode_slug_segment(&page_slug);
    render_public(&state, viewer, &username, Some(&app_slug), &real_slug, jar).await
}

/// `/{username}/{app_slug}` — public render of the named app's home
/// page (slug = "").
pub async fn get_public_user_app_root(
    State(state): State<AppState>,
    MaybeUser(viewer): MaybeUser,
    Path((username, app_slug)): Path<(String, String)>,
    jar: CookieJar,
) -> Result<Response, WebError> {
    render_public(&state, viewer, &username, Some(&app_slug), "", jar).await
}

async fn render_public(
    state: &AppState,
    viewer: Option<lovely_db::User>,
    username: &str,
    app_slug: Option<&str>,
    slug: &str,
    jar: CookieJar,
) -> Result<Response, WebError> {
    let (owner, app) = match app_slug {
        Some(name) => {
            let Some(owner) = lovely_db::find_user_by_username(&state.pg, username).await? else {
                return Err(WebError::NotFound);
            };
            let Some(app) =
                lovely_db::find_app_by_owner_and_slug(&state.pg, owner.id, name).await?
            else {
                return Err(WebError::NotFound);
            };
            (owner, app)
        }
        None => {
            let Some(pair) = find_default_app_for_username(&state.pg, username).await? else {
                return Err(WebError::NotFound);
            };
            pair
        }
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
            let markup = pages_views::user_profile(&owner, &apps, viewer.as_ref(), &token);
            return Ok((jar, markup).into_response());
        }
        return Err(WebError::NotFound);
    };

    // Owner always sees their page. Non-owners viewing an unpublished
    // page get bounced to the home route — the user explicitly asked
    // for a redirect over a 404 here. Unlisted pages stay 404 since
    // that's a "hidden" semantics, not "doesn't exist for them".
    if !is_owner {
        if page.published_at.is_none() {
            return Ok(Redirect::to("/").into_response());
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
                let markup = pages_views::password_gate(&page, username, slug, &token);
                return Ok((StatusCode::UNAUTHORIZED, jar, markup).into_response());
            }
        }
    }

    let mut rows = lovely_db::load_elements_for_page(&state.pg, page.id).await?;
    expand_repeaters(&state.pg, app.id, &mut rows).await?;
    resolve_bindings(&state.pg, app.id, &mut rows).await?;
    interpolate_collection_refs(&state.pg, app.id, &mut rows).await?;
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    auto_wire_forms(&mut rows, username, slug, &token);
    let tree = Tree::from_db_rows(&rows)?;
    let rendered = tree.render();
    let markup = pages_views::published_page(
        viewer.as_ref(),
        owner.id,
        &app.slug,
        &app.theme_json,
        &page,
        rendered,
        &token,
    );
    Ok((jar, markup).into_response())
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
    // The empty-slug Home page is structural — every app has one and
    // it can't be removed.
    if page.slug.is_empty() {
        return Err(WebError::Unprocessable(
            "The Home page can't be deleted.".into(),
        ));
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
    /// Disambiguator: when this is present, the publish checkbox state
    /// is canonical (off ↔ unchecked). When absent, publish is left
    /// alone so non-publish forms (title, description) don't clobber it.
    #[serde(default, rename = "_publish_form")]
    pub publish_form: Option<String>,
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
    // `_publish_form=1` marks the publish checkbox form. Without it
    // (e.g., the title+description autosave form), leave publish alone.
    let publish = if form.publish_form.is_some() {
        Some(form.publish.as_deref() == Some("on"))
    } else {
        form.publish.as_deref().map(|v| v == "on" || v == "true")
    };
    let updated = lovely_db::update_page(
        &state.pg,
        page.id,
        lovely_db::PagePatch {
            title: form.title.filter(|s| !s.trim().is_empty()),
            description: Some(form.description.filter(|s| !s.is_empty())),
            publish,
        },
    )
    .await?;

    // Publish-form submissions render OOB-swap fragments so the
    // visible pills (topbar + tree page row) reflect the new state
    // without a full reload. Other submissions just redirect.
    if form.publish_form.is_some() {
        let body = publish_pill_oob_fragment(updated.published_at.is_some());
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "HX-Trigger",
            axum::http::HeaderValue::from_static("preview-stale"),
        );
        return Ok((
            axum::http::StatusCode::OK,
            headers,
            axum::response::Html(body),
        )
            .into_response());
    }
    Ok(Redirect::to(&format!(
        "/apps/{}/pages/{}/edit",
        app.slug,
        slug_path_segment(&page.slug)
    ))
    .into_response())
}

/// HTML fragment with two `hx-swap-oob` spans matching the `id`s used
/// in the topbar and tree-page-row. Renders the right pill class for
/// the new published state.
fn publish_pill_oob_fragment(published: bool) -> String {
    let (class, label) = if published {
        ("pill pill-published", "published")
    } else {
        ("pill pill-draft", "draft")
    };
    format!(
        r#"<span id="topbar-publish-pill" hx-swap-oob="true" class="{class}">{label}</span>"# // Tree page row — id matches the maud template marker.
                                                                                              // The two-fragment string is concatenated so both swap.
    ) + &format!(r#"<span id="tree-page-pill" hx-swap-oob="true" class="{class}">{label}</span>"#)
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
