//! Full-screen 3-pane page builder layout.
//!
//! Body class `builder` opts the shell out of the centered `<main>` rule.
//! The grid is: top bar above three columns — tree, inline preview
//! canvas, inspector. Stacks under 64rem.

use crate::views::{builder_shell, labeled_checkbox, ShellCtx};
use lovely_db::{App, Collection, Page, User};
use lovely_tree::{ElementRow, ElementTag};
use maud::{html, Markup};
use uuid::Uuid;

/// What's currently selected in the inspector + tree.
///
/// `Page` selects the page itself — its inspector shows page-level
/// settings (title, description, head html, access, publish toggle).
/// `Element` selects a tree element.
#[derive(Clone, Copy)]
pub enum Selection {
    Page,
    Element(Uuid),
}

impl Selection {
    /// Resolve `?sel=` from the URL. The default selection is the
    /// page (top-of-tree view), matching the lovely Swift app.
    pub fn from_query(sel: Option<&str>, _root: Uuid) -> Self {
        match sel {
            None | Some("") | Some("page") | Some("root") => Selection::Page,
            Some(s) => match Uuid::parse_str(s) {
                Ok(id) => Selection::Element(id),
                Err(_) => Selection::Page,
            },
        }
    }

    pub fn param(&self) -> String {
        match self {
            Selection::Page => "page".to_string(),
            Selection::Element(id) => id.to_string(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Content,
    Data,
    Attrs,
    Style,
}

impl InspectorTab {
    pub fn from_query(tab: Option<&str>) -> Self {
        match tab {
            Some("data") => InspectorTab::Data,
            Some("attrs") => InspectorTab::Attrs,
            Some("style") => InspectorTab::Style,
            _ => InspectorTab::Content,
        }
    }
    pub fn slug(&self) -> &'static str {
        match self {
            InspectorTab::Content => "content",
            InspectorTab::Data => "data",
            InspectorTab::Attrs => "attrs",
            InspectorTab::Style => "style",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            InspectorTab::Content => "Content",
            InspectorTab::Data => "Data",
            InspectorTab::Attrs => "Attributes",
            InspectorTab::Style => "Style",
        }
    }
}

/// Which tabs are meaningful for the selected element. `#text` only
/// has Content (text payload). Regular elements never have Content
/// (text lives on `#text` children) — they get Attrs + Style + an
/// optional Data tab when the app has collections to bind/repeat.
fn tabs_for_row(row: &ElementRow, has_collections: bool) -> Vec<InspectorTab> {
    if row.is_text() {
        return vec![InspectorTab::Content];
    }
    let mut tabs = Vec::with_capacity(3);
    if has_collections {
        tabs.push(InspectorTab::Data);
    }
    tabs.push(InspectorTab::Attrs);
    tabs.push(InspectorTab::Style);
    tabs
}

pub struct BuilderCtx<'a> {
    pub user: &'a User,
    pub app: &'a App,
    pub page: &'a Page,
    pub elements: &'a [ElementRow],
    pub collections: &'a [Collection],
    pub selection: Selection,
    pub tab: InspectorTab,
    pub csrf_token: &'a str,
}

pub fn builder(ctx: BuilderCtx<'_>) -> Markup {
    use lovely_tree::Tree;
    let edit_segment = page_segment(&ctx.page.slug);
    // Public path is app-aware: default app uses the short
    // `/{user}/{slug}` shape; non-default apps include the app slug
    // (`/{user}/{app}/{slug}`). The empty home slug becomes `~home`
    // when it would otherwise leave a trailing path segment empty,
    // matching how editor URLs encode the home page.
    let public_path = match (ctx.app.is_default, ctx.page.slug.as_str()) {
        (true, "") => format!("/{}", ctx.user.username),
        (true, slug) => format!("/{}/{}", ctx.user.username, slug),
        (false, "") => format!("/{}/{}/~home", ctx.user.username, ctx.app.slug),
        (false, slug) => format!("/{}/{}/{}", ctx.user.username, ctx.app.slug, slug),
    };
    let canvas_url = format!("/apps/{}/pages/{}/canvas", ctx.app.slug, edit_segment);
    // Initial canvas content — rendered inline so first paint matches
    // the live preview without an extra round trip. Falls back to
    // empty markup if the tree is malformed.
    let initial_canvas: Markup = if ctx.elements.is_empty() {
        html! {}
    } else {
        Tree::from_db_rows(ctx.elements)
            .map(|t| t.render())
            .unwrap_or_else(|_| html! {})
    };
    let body = html! {
        div .builder-grid {
            (topbar(&ctx, &public_path))
            aside #tree .builder-tree
                hx-get=(format!("/apps/{}/pages/{}/tree?sel={}",
                    ctx.app.slug, edit_segment, ctx.selection.param()))
                hx-trigger="preview-stale from:body"
                hx-swap="innerHTML" {
                (tree_fragment(&ctx))
            }
            main .builder-canvas {
                div .canvas-backdrop
                    role="button"
                    tabindex="0"
                    hx-get=(format!("/apps/{}/pages/{}/inspector?sel=page",
                        ctx.app.slug, edit_segment))
                    hx-target="#inspector"
                    hx-swap="innerHTML"
                    hx-push-url=(format!("/apps/{}/pages/{}/edit?sel=page",
                        ctx.app.slug, edit_segment))
                    title="Click to edit page settings" {}
                div #preview-canvas .preview-canvas
                    hx-get=(canvas_url)
                    hx-trigger="preview-stale from:body"
                    hx-swap="innerHTML" {
                    (initial_canvas)
                }
            }
            aside #inspector .builder-inspector
                hx-trigger="preview-stale from:body"
                hx-get=(format!("/apps/{}/pages/{}/inspector?sel={}&tab={}",
                    ctx.app.slug, edit_segment,
                    ctx.selection.param(), ctx.tab.slug()))
                hx-swap="innerHTML" {
                (inspector_fragment(&ctx))
            }
        }
    };
    builder_shell(
        ShellCtx {
            title: &format!("Edit: {}", ctx.page.title),
            description: None,
            user: Some(ctx.user),
            csrf_token: ctx.csrf_token,
        },
        body,
    )
}

fn topbar(ctx: &BuilderCtx<'_>, public_path: &str) -> Markup {
    html! {
        header .builder-topbar {
            nav .breadcrumbs {
                a href="/apps" { "Apps" } " / "
                a href={"/apps/" (ctx.app.slug)} { (ctx.app.name) } " / "
                @if ctx.page.slug.is_empty() { "(home)" } @else { (ctx.page.slug) }
            }
            // div .topbar-history {
            //     form
            //         hx-post=(format!("/apps/{}/pages/{}/undo", ctx.app.slug, edit_segment))
            //         hx-swap="none"
            //         .inline-form {
            //         input type="hidden" name="_csrf" value=(ctx.csrf_token);
            //         button type="submit" title="Undo (Cmd-Z)" { "↶" }
            //     }
            //     form
            //         hx-post=(format!("/apps/{}/pages/{}/redo", ctx.app.slug, edit_segment))
            //         hx-swap="none"
            //         .inline-form {
            //         input type="hidden" name="_csrf" value=(ctx.csrf_token);
            //         button type="submit" title="Redo (Cmd-Shift-Z)" { "↷" }
            //     }
            // }
            div .spacer {}
            @if ctx.page.published_at.is_some() {
                span #topbar-publish-pill .pill .pill-published { "published" }
            } @else {
                span #topbar-publish-pill .pill .pill-draft { "draft" }
            }
            a href=(public_path) target="_blank" rel="noopener" { "View public ↗" }
        }
    }
}

pub fn tree_fragment(ctx: &BuilderCtx<'_>) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let page_selected = matches!(ctx.selection, Selection::Page);
    let elements_url = format!("/apps/{}/pages/{}/elements", ctx.app.slug, edit_segment);
    let mut top_level: Vec<&lovely_tree::ElementRow> = ctx
        .elements
        .iter()
        .filter(|r| r.parent_id.is_none())
        .collect();
    top_level.sort_by_key(|r| sibling_index(ctx.elements, r.id.into_inner()));
    html! {
        div .elements-sidebar__page-cell
            aria-current=[if page_selected { Some("true") } else { None }]
            role="button"
            tabindex="0"
            data-sel-id="page"
            hx-get=(format!("/apps/{}/pages/{}/inspector?sel=page",
                ctx.app.slug, edit_segment))
            hx-target="#inspector"
            hx-swap="innerHTML"
            hx-push-url=(format!("/apps/{}/pages/{}/edit?sel=page", ctx.app.slug, edit_segment))
            {
                span .tree-page-glyph { "▤" }
                " "
                (ctx.page.title)
                @if ctx.page.published_at.is_some() {
                    " " span #tree-page-pill .pill .pill-published { "published" }
                } @else {
                    " " span #tree-page-pill .pill .pill-draft { "draft" }
                }
            }
        ul .tree-list .tree-root {
            @for r in &top_level {
                (tree_node(ctx, r.id.into_inner(), current_selection(ctx), &edit_segment))
            }
            li .tree-empty {
                form hx-post=(elements_url) hx-swap="none" .inline-form {
                    input type="hidden" name="_csrf" value=(ctx.csrf_token);
                    input type="hidden" name="tag" value="div";
                    button type="submit" { "Add after" }
                }
            }
        }
    }
}

fn current_selection(ctx: &BuilderCtx<'_>) -> Option<Uuid> {
    match ctx.selection {
        Selection::Page => None,
        Selection::Element(id) => Some(id),
    }
}

fn tree_node(ctx: &BuilderCtx<'_>, id: Uuid, selected: Option<Uuid>, edit_segment: &str) -> Markup {
    let row = match ctx.elements.iter().find(|r| r.id.into_inner() == id) {
        Some(r) => r,
        None => return html! {},
    };
    let mut children: Vec<&lovely_tree::ElementRow> = ctx
        .elements
        .iter()
        .filter(|r| r.parent_id.map(|p| p.into_inner()) == Some(id))
        .collect();
    children.sort_by_key(|r| sibling_index(ctx.elements, r.id.into_inner()));

    let is_selected = selected == Some(id);
    let is_text = row.is_text();
    let label = if is_text {
        row.text.clone().unwrap_or_default()
    } else {
        row.tag.clone()
    };
    html! {
        li
            data-element-id=(id)
            aria-current=[if is_selected { Some("true") } else { None }] {
            div .tree-row {
                div .tree-row-button
                    role="button"
                    tabindex="0"
                    data-sel-id=(id)
                    hx-get=(format!("/apps/{}/pages/{}/inspector?sel={}",
                        ctx.app.slug, edit_segment, id))
                    hx-target="#inspector"
                    hx-swap="innerHTML"
                    hx-push-url=(format!("/apps/{}/pages/{}/edit?sel={}",
                        ctx.app.slug, edit_segment, id)) {
                    @if is_text {
                        span .tree-text-glyph { "T" }
                        span .tree-text-snippet {
                            (label.chars().take(28).collect::<String>())
                        }
                    } @else {
                        code { (label) }
                        @if let Some(t) = &row.text {
                            " " span .muted { (t.chars().take(20).collect::<String>()) }
                        }
                    }
                }
                (row_actions_menu(ctx, row, edit_segment))
            }
            @if !children.is_empty() {
                ul .tree-children data-parent-id=(id) {
                    @for child in &children {
                        (tree_node(ctx, child.id.into_inner(), selected, edit_segment))
                    }
                }
            } @else {
                ul .tree-children data-parent-id=(id) {}
            }
        }
    }
}

fn row_actions_menu(ctx: &BuilderCtx<'_>, row: &ElementRow, edit_segment: &str) -> Markup {
    let id = row.id;
    let app_slug = &ctx.app.slug;
    let csrf_token = ctx.csrf_token;
    let is_leaf = ElementTag::from_name(&row.tag)
        .map(|t| t.is_leaf())
        .unwrap_or(true);
    html! {
        details .tree-actions data-actions {
            summary title="Actions" { "⋯" }
            div .tree-actions-menu {
                (quick_action(app_slug, edit_segment, &id.to_string(), csrf_token,
                    "add-before", "div", "Add before"))
                (quick_action(app_slug, edit_segment, &id.to_string(), csrf_token,
                    "add-after", "div", "Add after"))
                @if !is_leaf {
                    (quick_child_action(app_slug, edit_segment, &id.to_string(), csrf_token,
                        "div", "Add child"))
                    (quick_child_action(app_slug, edit_segment, &id.to_string(), csrf_token,
                        ElementTag::TEXT_NAME, "Add text"))
                }
                form
                    hx-post=(format!("/apps/{app_slug}/pages/{edit_segment}/elements/{id}/duplicate"))
                    hx-swap="none"
                    .tree-action-form {
                    input type="hidden" name="_csrf" value=(csrf_token);
                    button type="submit" { "Duplicate" }
                }
                form
                    hx-post=(format!("/apps/{app_slug}/pages/{edit_segment}/elements/{id}/wrap"))
                    hx-swap="none"
                    .tree-action-form {
                    input type="hidden" name="_csrf" value=(csrf_token);
                    input type="hidden" name="tag" value="div";
                    button type="submit" { "Wrap in div" }
                }
                form
                    hx-post=(format!("/apps/{app_slug}/pages/{edit_segment}/elements/{id}/delete"))
                    hx-swap="none"
                    .tree-action-form {
                    input type="hidden" name="_csrf" value=(csrf_token);
                    button type="submit" .danger { "Delete" }
                }
            }
        }
    }
}

fn quick_action(
    app_slug: &str,
    edit_segment: &str,
    id: &str,
    csrf_token: &str,
    op: &str,
    tag: &str,
    label: &str,
) -> Markup {
    html! {
        form
            hx-post=(format!("/apps/{app_slug}/pages/{edit_segment}/elements/{id}/{op}"))
            hx-swap="none"
            .tree-action-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            input type="hidden" name="tag" value=(tag);
            button type="submit" { (label) }
        }
    }
}

fn quick_child_action(
    app_slug: &str,
    edit_segment: &str,
    parent_id: &str,
    csrf_token: &str,
    tag: &str,
    label: &str,
) -> Markup {
    html! {
        form
            hx-post=(format!("/apps/{app_slug}/pages/{edit_segment}/elements"))
            hx-swap="none"
            .tree-action-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            input type="hidden" name="parent_id" value=(parent_id);
            input type="hidden" name="tag" value=(tag);
            button type="submit" { (label) }
        }
    }
}

pub fn inspector_fragment(ctx: &BuilderCtx<'_>) -> Markup {
    match ctx.selection {
        Selection::Page => page_inspector(ctx),
        Selection::Element(id) => element_inspector(ctx, id),
    }
}

fn page_inspector(ctx: &BuilderCtx<'_>) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let app_slug = &ctx.app.slug;
    let publish_url = format!("/apps/{app_slug}/pages/{edit_segment}");
    let head_url = format!("/apps/{app_slug}/pages/{edit_segment}/head");
    let access_url = format!("/apps/{app_slug}/pages/{edit_segment}/access");
    html! {
        header .inspector-header {
            span .tree-page-glyph { "▤" }
            " "
            strong { "Page" }
            small .muted .inspector-id { (ctx.page.slug) }
        }
        section .inspector-section {
            h4 { "Title & description" }
            form
                method="post" action=(publish_url)
                .inspector-form
                hx-post=(publish_url) hx-swap="none"
                hx-trigger="input changed delay:400ms, change" {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                @if ctx.page.published_at.is_some() {
                    input type="hidden" name="publish" value="on";
                }
                label {
                    "Title"
                    input type="text" name="title" value=(ctx.page.title);
                }
                label {
                    "Description"
                    textarea name="description" rows="2" {
                        (ctx.page.description.clone().unwrap_or_default())
                    }
                }
            }
        }
        section .inspector-section {
            h4 { "Publish" }
            form
                hx-post=(publish_url)
                hx-trigger="change"
                hx-swap="none"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                input type="hidden" name="_publish_form" value="1";
                (labeled_checkbox("publish", "Published", ctx.page.published_at.is_some()))
            }
        }
        section .inspector-section {
            h4 { "Custom <head>" }
            p .muted { "Sanitized: no <script> or on* attrs." }
            form
                hx-post=(head_url) hx-swap="none"
                hx-trigger="input changed delay:600ms, change"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                textarea name="head_html" rows="4" { (ctx.page.head_html) }
            }
        }
        section .inspector-section {
            h4 { "Access" }
            form hx-post=(access_url) hx-swap="none"
                hx-trigger="change, input changed delay:600ms"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                label {
                    "Password (leave blank to remove)"
                    input type="password" name="password" placeholder=
                        @if ctx.page.password_hash.is_some() { "currently set" } @else { "no password" };
                }
                (labeled_checkbox("unlisted", "Unlisted (404 unless owner)", ctx.page.unlisted))
            }
        }
        (page_add_element_section(ctx))
    }
}

/// "Add element" buttons rendered when the page itself is selected.
/// Always inserts at the top level (parent_id omitted) — there is no
/// privileged "root" element anymore, just rows whose parent is NULL.
fn page_add_element_section(ctx: &BuilderCtx<'_>) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let elements_url = format!("/apps/{}/pages/{}/elements", ctx.app.slug, edit_segment);
    html! {
        div .inspector-add {
            h3 { "Add element" }
            div .inspector-add-buttons {
                form hx-post=(elements_url) hx-swap="none" .inline-form {
                    input type="hidden" name="_csrf" value=(ctx.csrf_token);
                    input type="hidden" name="tag" value="div";
                    button type="submit" { "Add div" }
                }
                form hx-post=(elements_url) hx-swap="none" .inline-form {
                    input type="hidden" name="_csrf" value=(ctx.csrf_token);
                    input type="hidden" name="tag" value=(ElementTag::TEXT_NAME);
                    button type="submit" {
                        span .tree-text-glyph { "T" } " Add text"
                    }
                }
            }
        }
    }
}

fn element_inspector(ctx: &BuilderCtx<'_>, id: Uuid) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let row = ctx.elements.iter().find(|r| r.id.into_inner() == id);
    html! {
        @if let Some(row) = row {
            header .inspector-header {
                form
                    hx-patch=(format!("/apps/{}/pages/{}/elements/{}",
                        ctx.app.slug, edit_segment, row.id))
                    hx-swap="none"
                    .inspector-tag-form {
                    input type="hidden" name="_csrf" value=(ctx.csrf_token);
                    select name="tag" onchange="this.form.requestSubmit()" {
                        @for tag in ElementTag::ALL {
                            option value=(tag.name())
                                selected[tag.name() == row.tag] { (tag.name()) }
                        }
                    }
                }
            }
            @let available = tabs_for_row(row, !ctx.collections.is_empty());
            @let active = if available.contains(&ctx.tab) { ctx.tab } else { available[0] };
            nav .inspector-tabs {
                @for t in &available {
                    a
                        data-tab=(t.slug())
                        aria-current=[if t.slug() == active.slug() { Some("true") } else { None }]
                        hx-get=(format!("/apps/{}/pages/{}/inspector?sel={}&tab={}",
                            ctx.app.slug, edit_segment, row.id, t.slug()))
                        hx-target="#inspector"
                        hx-swap="innerHTML" { (t.label()) }
                }
            }
            div .inspector-body {
                @match active {
                    InspectorTab::Content => (content_tab(ctx, row)),
                    InspectorTab::Data => (data_tab(ctx, row)),
                    InspectorTab::Attrs => (attrs_tab(ctx, row)),
                    InspectorTab::Style => (style_tab(ctx, row)),
                }
            }
            (add_child_form(ctx, row))
            form
                hx-post=(format!("/apps/{}/pages/{}/elements/{}/delete",
                    ctx.app.slug, edit_segment, row.id))
                hx-target="#tree"
                hx-swap="innerHTML"
                .inspector-delete {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                button type="submit" .danger { "Delete element" }
            }
        } @else {
            p .muted { "No element selected." }
        }
    }
}

fn add_child_form(ctx: &BuilderCtx<'_>, row: &ElementRow) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let is_leaf = ElementTag::from_name(&row.tag)
        .map(|t| t.is_leaf())
        .unwrap_or(true);
    // Root and non-root use the same action set now. Add-before /
    // add-after work for the root because post_add_before/after carry
    // the new element under the same parent (None for root) and update
    // the page's root pointer when needed downstream.
    html! {
        div .inspector-add {
            h3 { "Add element" }
            div .inspector-add-buttons {
                @if !is_leaf {
                    form
                        hx-post=(format!("/apps/{}/pages/{}/elements", ctx.app.slug, edit_segment))
                        hx-swap="none"
                        .inline-form {
                        input type="hidden" name="_csrf" value=(ctx.csrf_token);
                        input type="hidden" name="parent_id" value=(row.id);
                        input type="hidden" name="tag" value="div";
                        button type="submit" { "Add child" }
                    }
                    form
                        hx-post=(format!("/apps/{}/pages/{}/elements", ctx.app.slug, edit_segment))
                        hx-swap="none"
                        .inline-form {
                        input type="hidden" name="_csrf" value=(ctx.csrf_token);
                        input type="hidden" name="parent_id" value=(row.id);
                        input type="hidden" name="tag" value=(ElementTag::TEXT_NAME);
                        button type="submit" {
                            span .tree-text-glyph { "T" } " Add text"
                        }
                    }
                }
                form
                    hx-post=(format!("/apps/{}/pages/{}/elements/{}/add-before",
                        ctx.app.slug, edit_segment, row.id))
                    hx-swap="none"
                    .inline-form {
                    input type="hidden" name="_csrf" value=(ctx.csrf_token);
                    input type="hidden" name="tag" value="div";
                    button type="submit" { "Add before" }
                }
                form
                    hx-post=(format!("/apps/{}/pages/{}/elements/{}/add-after",
                        ctx.app.slug, edit_segment, row.id))
                    hx-swap="none"
                    .inline-form {
                    input type="hidden" name="_csrf" value=(ctx.csrf_token);
                    input type="hidden" name="tag" value="div";
                    button type="submit" { "Add after" }
                }
            }
        }
    }
}

fn content_tab(_ctx: &BuilderCtx<'_>, row: &ElementRow) -> Markup {
    // Content tab is only ever shown for `#text` nodes; non-text
    // elements use `#text` children and route data binding to the
    // dedicated Data tab.
    let edit_segment = page_segment(&_ctx.page.slug);
    let patch_url = format!(
        "/apps/{}/pages/{}/elements/{}",
        _ctx.app.slug, edit_segment, row.id
    );
    html! {
        form
            hx-patch=(patch_url)
            hx-swap="none"
            hx-trigger="input changed delay:400ms, change"
            .inspector-form {
            input type="hidden" name="_csrf" value=(_ctx.csrf_token);
            label {
                "Text content"
                textarea name="text" rows="3" autofocus { (row.text.clone().unwrap_or_default()) }
            }
        }
    }
}

/// "Data" tab for non-text elements.
///
/// Three sub-sections, each conditional on element type:
///   - `<form>` → "Collect submissions" picker (sets data-lovely-collection)
///   - `<input>/<textarea>/<select>` → "Collect value" field picker (sets
///     data-lovely-field; only meaningful when an ancestor `<form>` has
///     a collection chosen)
///   - everything → Read binding section
///   - non-leaves → Repeat per record section
fn data_tab(ctx: &BuilderCtx<'_>, row: &ElementRow) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let bind = row
        .attrs_json
        .get("data-lovely-bind")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let (bind_coll, bind_field) = split_ref(bind);
    let repeat = row
        .attrs_json
        .get("data-lovely-repeat")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let is_form = row.tag == "form";
    let is_input_like = matches!(row.tag.as_str(), "input" | "textarea" | "select");
    let is_leaf = ElementTag::from_name(&row.tag)
        .map(|t| t.is_leaf())
        .unwrap_or(true);
    let patch_url = format!(
        "/apps/{}/pages/{}/elements/{}",
        ctx.app.slug, edit_segment, row.id
    );
    html! {
        @if is_form {
            (form_collection_section(ctx, &patch_url, row))
        }
        @if is_input_like {
            (field_picker_section(ctx, &patch_url, row))
        }
        (data_ref_section(
            ctx, &patch_url,
            DataDirection::Read,
            bind, bind_coll, bind_field,
        ))
        @if !is_leaf {
            (repeat_section(ctx, &patch_url, repeat))
        }
    }
}

/// Form-element-only section: pick the collection that this form
/// writes records into. Stored as `data-lovely-collection` on the form.
/// Once set, descendant inputs can pick a Field to map into.
fn form_collection_section(ctx: &BuilderCtx<'_>, patch_url: &str, row: &ElementRow) -> Markup {
    let current = row
        .attrs_json
        .get("data-lovely-collection")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    html! {
        div .data-binding {
            h3 { "Collect submissions" }
            p .muted {
                "Pick the collection this form writes to. The form will be "
                "wired to POST a new record on submit, with descendant "
                "inputs mapped to the collection's fields."
            }
            form
                hx-patch=(patch_url)
                hx-swap="none"
                hx-trigger="change"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                label {
                    "Collection"
                    select name="collect_collection" {
                        option value="" { "(none)" }
                        @for c in ctx.collections {
                            option value=(c.name) selected[c.name == current] { (c.name) }
                        }
                    }
                }
            }
            @if !current.is_empty() {
                p .muted {
                    "Submissions go to "
                    code { (current) }
                    "."
                }
            }
        }
    }
}

/// Input-element-only section: pick which field of the form's
/// collection this input writes into. If no ancestor `<form>` has a
/// collection set yet, this section shows guidance instead of pickers.
fn field_picker_section(ctx: &BuilderCtx<'_>, patch_url: &str, row: &ElementRow) -> Markup {
    let current_field = row
        .attrs_json
        .get("data-lovely-field")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let form_collection = nearest_form_collection(ctx.elements, row.id.into_inner());
    html! {
        div .data-binding {
            h3 { "Collect value" }
            @match form_collection.as_deref() {
                None => {
                    p .muted {
                        "Wrap this input in a "
                        code { "<form>" }
                        " element, then choose a collection on the form, "
                        "to start collecting submissions."
                    }
                }
                Some(coll_name) => {
                    p .muted {
                        "Submissions go to "
                        code { (coll_name) }
                        ". Pick which field this input writes into."
                    }
                    @let active_coll = ctx.collections.iter().find(|c| c.name == coll_name);
                    @let fields: Vec<String> = active_coll
                        .map(|c| c.fields()).unwrap_or_default();
                    form
                        hx-patch=(patch_url)
                        hx-swap="none"
                        hx-trigger="change"
                        .inspector-form {
                        input type="hidden" name="_csrf" value=(ctx.csrf_token);
                        label {
                            "Field"
                            select name="collect_field" {
                                option value="" { "(none)" }
                                @for f in &fields {
                                    option value=(f) selected[f == current_field] { (f) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Walk up the parent chain from `start_id` looking for a `<form>`
/// element whose `data-lovely-collection` attr is set. Returns the
/// collection name when found.
fn nearest_form_collection(rows: &[ElementRow], start_id: Uuid) -> Option<String> {
    use std::collections::HashMap;
    let by_id: HashMap<Uuid, &ElementRow> = rows.iter().map(|r| (r.id.into_inner(), r)).collect();
    let mut cur = by_id.get(&start_id).and_then(|r| r.parent_id);
    while let Some(pid) = cur {
        let pid_inner = pid.into_inner();
        let row = by_id.get(&pid_inner)?;
        if row.tag == "form" {
            return row
                .attrs_json
                .get("data-lovely-collection")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
        }
        cur = row.parent_id;
    }
    None
}

/// "Repeat per record" inspector section. Pick a collection and the
/// element's first child becomes a template instantiated once per
/// record. Auto-saves on change. Disconnect button drops the
/// `data-lovely-repeat` attribute.
fn repeat_section(ctx: &BuilderCtx<'_>, patch_url: &str, current: &str) -> Markup {
    html! {
        div .data-binding {
            h3 { "Repeat per record" }
            p .muted {
                "Pick a collection and this element's first child becomes a "
                "template — one copy will render for each record. Use "
                code { "{{field}}" }
                " inside the template to substitute record values."
            }
            form
                hx-patch=(patch_url)
                hx-swap="none"
                hx-trigger="change"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                label {
                    "Collection"
                    select name="repeat_collection" {
                        option value="" { "(none)" }
                        @for c in ctx.collections {
                            option value=(c.name) selected[c.name == current] { (c.name) }
                        }
                    }
                }
            }
            @if !current.is_empty() {
                div .binding-current {
                    p .muted { "Repeating over " code { (current) } }
                    form
                        hx-patch=(patch_url)
                        hx-swap="none"
                        .inline-form {
                        input type="hidden" name="_csrf" value=(ctx.csrf_token);
                        input type="hidden" name="repeat_collection" value="";
                        button type="submit" .danger { "Stop repeating" }
                    }
                }
            }
        }
    }
}

fn split_ref(value: &str) -> (&str, &str) {
    match value.split_once('.') {
        Some((c, f)) => (c, f),
        None => (value, ""),
    }
}

#[derive(Copy, Clone)]
enum DataDirection {
    Read,
}

fn data_ref_section(
    ctx: &BuilderCtx<'_>,
    patch_url: &str,
    _dir: DataDirection,
    current: &str,
    current_coll: &str,
    current_field: &str,
) -> Markup {
    let active_coll = ctx.collections.iter().find(|c| c.name == current_coll);
    let coll_fields: Vec<String> = active_coll.map(|c| c.fields()).unwrap_or_default();
    html! {
        div .data-binding {
            h3 { "Display value (read from data)" }
            p .muted {
                "Pick a collection to make its records available to this "
                "element. The optional field directly populates this "
                "element's text — leave it blank to use "
                code { "{{collection.field}}" }
                " interpolation in #text children, repeat templates, or "
                "dynamic attributes instead."
            }
            form
                hx-patch=(patch_url)
                hx-swap="none"
                hx-trigger="change"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                label {
                    "Collection"
                    select name="binding_collection" {
                        option value="" { "(none)" }
                        @for c in ctx.collections {
                            option value=(c.name) selected[c.name == current_coll] { (c.name) }
                        }
                    }
                }
                label {
                    "Field (optional — direct value display)"
                    select name="binding_field" disabled[active_coll.is_none()] {
                        option value="" { "(none)" }
                        @for f in &coll_fields {
                            option value=(f) selected[f == current_field] { (f) }
                        }
                    }
                }
            }
            @if !current.is_empty() {
                div .binding-current {
                    p .muted { "Connected to " code { (current) } }
                    form
                        hx-patch=(patch_url)
                        hx-swap="none"
                        .inline-form {
                        input type="hidden" name="_csrf" value=(ctx.csrf_token);
                        input type="hidden" name="binding_collection" value="";
                        input type="hidden" name="binding_field" value="";
                        button type="submit" .danger { "Disconnect" }
                    }
                }
            }
        }
    }
}

/// Tag-aware dedicated attribute fields. Renders the most-likely
/// inputs for the selected element's tag (href on <a>, src/alt on
/// <img>, etc.) above the freeform attribute editor.
fn dedicated_attrs_for_tag(tag: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    // (attr_name, html_input_type, placeholder)
    //
    // URL-bearing attrs (href, src, action) use a plain text input — the
    // HTML5 "url" type rejects relative paths and fragments, which the
    // browser then silently refuses to submit. The renderer escapes
    // values, so accepting relative URLs here is safe.
    match tag {
        "a" => &[
            ("href", "text", "https://example.com or /about"),
            ("target", "text", "_blank"),
            ("rel", "text", "noopener"),
        ],
        "img" => &[
            ("src", "text", "/static/photo.jpg"),
            ("alt", "text", "Description"),
            ("width", "number", ""),
            ("height", "number", ""),
        ],
        "input" => &[
            ("type", "text", "text"),
            ("name", "text", ""),
            ("placeholder", "text", ""),
            ("value", "text", ""),
        ],
        "textarea" => &[
            ("name", "text", ""),
            ("placeholder", "text", ""),
            ("rows", "number", ""),
        ],
        "select" => &[("name", "text", "")],
        "form" => &[("action", "text", "/path"), ("method", "text", "post")],
        "button" => &[("type", "text", "button"), ("name", "text", "")],
        "label" => &[("for", "text", "")],
        "iframe" => &[("src", "text", ""), ("title", "text", "")],
        _ => &[],
    }
}

fn attrs_tab(ctx: &BuilderCtx<'_>, row: &ElementRow) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let patch_url = format!(
        "/apps/{}/pages/{}/elements/{}",
        ctx.app.slug, edit_segment, row.id
    );
    let attrs: Vec<(String, String)> = row
        .attrs_json
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| !k.starts_with("data-lovely-"))
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let dedicated = dedicated_attrs_for_tag(&row.tag);
    html! {
        @if !dedicated.is_empty() {
            section .inspector-section {
                h4 { "Tag attributes" }
                @for (name, input_type, placeholder) in dedicated {
                    @let current = attrs.iter().find(|(n, _)| n == name)
                        .map(|(_, v)| v.clone()).unwrap_or_default();
                    form
                        hx-patch=(patch_url)
                        hx-swap="none"
                        hx-trigger="input changed delay:400ms, change"
                        .inspector-form
                        data-attr=(name) {
                        input type="hidden" name="_csrf" value=(ctx.csrf_token);
                        input type="hidden" name="attr_name" value=(name);
                        label {
                            (name)
                            input type=(*input_type) name="attr_value"
                                value=(current) placeholder=(*placeholder);
                        }
                    }
                }
            }
        } @else {
            p .muted { "No tag-specific attributes for " code { (row.tag) } "." }
        }
    }
}

fn style_tab(ctx: &BuilderCtx<'_>, row: &ElementRow) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let current_style = row
        .attrs_json
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let patch_url = format!(
        "/apps/{}/pages/{}/elements/{}",
        ctx.app.slug, edit_segment, row.id
    );
    html! {
        form
            hx-patch=(patch_url)
            hx-swap="none"
            hx-trigger="input changed delay:400ms, change"
            .inspector-form {
            input type="hidden" name="_csrf" value=(ctx.csrf_token);
            input type="hidden" name="attr_name" value="style";
            label {
                "Inline style (CSS)"
                textarea name="attr_value" rows="6"
                    placeholder="padding: 1rem; color: var(--lovely-primary);" {
                    (current_style)
                }
            }
        }
    }
}

fn page_segment(slug: &str) -> String {
    if slug.is_empty() {
        "~home".into()
    } else {
        slug.into()
    }
}

/// Walks the prev_sibling chain starting from rows with prev_sibling = NULL
/// and returns a 0-based index for the given id within its parent. Used
/// only for stable display ordering in the tree sidebar.
fn sibling_index(rows: &[lovely_tree::ElementRow], target: Uuid) -> usize {
    let target_row = match rows.iter().find(|r| r.id.into_inner() == target) {
        Some(r) => r,
        None => return 0,
    };
    let parent = target_row.parent_id.map(|p| p.into_inner());
    let siblings: Vec<&lovely_tree::ElementRow> = rows
        .iter()
        .filter(|r| r.parent_id.map(|p| p.into_inner()) == parent)
        .collect();
    let mut chain: Vec<Uuid> = Vec::new();
    if let Some(first) = siblings.iter().find(|r| r.prev_sibling.is_none()) {
        chain.push(first.id.into_inner());
        loop {
            let last = *chain.last().unwrap();
            match siblings
                .iter()
                .find(|r| r.prev_sibling.map(|p| p.into_inner()) == Some(last))
            {
                Some(next) => chain.push(next.id.into_inner()),
                None => break,
            }
        }
    }
    chain.iter().position(|&x| x == target).unwrap_or(0)
}
