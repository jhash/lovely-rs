//! Full-screen 3-pane page builder layout.
//!
//! Body class `builder` opts the shell out of the centered `<main>` rule.
//! The grid is: top bar above three columns — tree, preview iframe,
//! inspector. Stacks under 64rem.

use crate::views::{builder_shell, ShellCtx};
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

#[derive(Clone, Copy)]
pub enum InspectorTab {
    Content,
    Attrs,
    Style,
}

impl InspectorTab {
    pub fn from_query(tab: Option<&str>) -> Self {
        match tab {
            Some("attrs") => InspectorTab::Attrs,
            Some("style") => InspectorTab::Style,
            _ => InspectorTab::Content,
        }
    }
    pub fn slug(&self) -> &'static str {
        match self {
            InspectorTab::Content => "content",
            InspectorTab::Attrs => "attrs",
            InspectorTab::Style => "style",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            InspectorTab::Content => "Content",
            InspectorTab::Attrs => "Attributes",
            InspectorTab::Style => "Style",
        }
    }
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
    let edit_segment = page_segment(&ctx.page.slug);
    let public_path = if ctx.page.slug.is_empty() {
        format!("/{}", ctx.user.username)
    } else {
        format!("/{}/{}", ctx.user.username, ctx.page.slug)
    };
    let preview_src = format!(
        "/apps/{}/pages/{}/preview",
        ctx.app.slug, edit_segment
    );
    let body = html! {
        div .builder-grid {
            (topbar(&ctx, &public_path, &edit_segment))
            aside #tree .builder-tree
                hx-get=(format!("/apps/{}/pages/{}/tree?sel={}",
                    ctx.app.slug, edit_segment, ctx.selection.param()))
                hx-trigger="preview-stale from:body"
                hx-swap="innerHTML" {
                (tree_fragment(&ctx))
            }
            main .builder-canvas {
                button .canvas-backdrop
                    type="button"
                    hx-get=(format!("/apps/{}/pages/{}/inspector?sel=page",
                        ctx.app.slug, edit_segment))
                    hx-target="#inspector"
                    hx-swap="innerHTML"
                    hx-push-url=(format!("/apps/{}/pages/{}/edit?sel=page",
                        ctx.app.slug, edit_segment))
                    title="Click to edit page settings" {}
                iframe #preview src=(preview_src) {}
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

fn topbar(ctx: &BuilderCtx<'_>, public_path: &str, edit_segment: &str) -> Markup {
    html! {
        header .builder-topbar {
            nav .breadcrumbs {
                a href="/apps" { "Apps" } " / "
                a href={"/apps/" (ctx.app.slug)} { (ctx.app.name) } " / "
                @if ctx.page.slug.is_empty() { "(home)" } @else { (ctx.page.slug) }
            }
            div .topbar-history {
                form
                    hx-post=(format!("/apps/{}/pages/{}/undo", ctx.app.slug, edit_segment))
                    hx-swap="none"
                    .inline-form {
                    input type="hidden" name="_csrf" value=(ctx.csrf_token);
                    button type="submit" title="Undo (Cmd-Z)" { "↶" }
                }
                form
                    hx-post=(format!("/apps/{}/pages/{}/redo", ctx.app.slug, edit_segment))
                    hx-swap="none"
                    .inline-form {
                    input type="hidden" name="_csrf" value=(ctx.csrf_token);
                    button type="submit" title="Redo (Cmd-Shift-Z)" { "↷" }
                }
            }
            div .spacer {}
            @if ctx.page.published_at.is_some() {
                span .pill .pill-published { "published" }
            } @else {
                span .pill .pill-draft { "draft" }
            }
            a href=(public_path) target="_blank" rel="noopener" { "View public ↗" }
        }
    }
}

pub fn tree_fragment(ctx: &BuilderCtx<'_>) -> Markup {
    let root_id = ctx.page.root_element.unwrap_or_default();
    let edit_segment = page_segment(&ctx.page.slug);
    let page_selected = matches!(ctx.selection, Selection::Page);
    html! {
        ul .tree-list .tree-root data-parent-id=(root_id) {
            li .tree-page-row
                aria-current=[if page_selected { Some("true") } else { None }] {
                button .tree-row-button
                    type="button"
                    data-sel-id="page"
                    hx-get=(format!("/apps/{}/pages/{}/inspector?sel=page",
                        ctx.app.slug, edit_segment))
                    hx-target="#inspector"
                    hx-swap="innerHTML"
                    hx-push-url=(format!("/apps/{}/pages/{}/edit?sel=page",
                        ctx.app.slug, edit_segment)) {
                    span .tree-page-glyph { "▤" }
                    " "
                    (ctx.page.title)
                    @if ctx.page.published_at.is_some() {
                        " " span .pill .pill-published { "published" }
                    } @else {
                        " " span .pill .pill-draft { "draft" }
                    }
                }
            }
            (tree_node(ctx, root_id, current_selection(ctx), &edit_segment, true))
        }
    }
}

fn current_selection(ctx: &BuilderCtx<'_>) -> Option<Uuid> {
    match ctx.selection {
        Selection::Page => None,
        Selection::Element(id) => Some(id),
    }
}

fn tree_node(
    ctx: &BuilderCtx<'_>,
    id: Uuid,
    selected: Option<Uuid>,
    edit_segment: &str,
    is_root: bool,
) -> Markup {
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
    let is_text = row.tag == "#text";
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
                button .tree-row-button
                    type="button"
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
                (row_actions_menu(ctx, row, edit_segment, is_root))
            }
            @if !children.is_empty() {
                ul .tree-children data-parent-id=(id) {
                    @for child in &children {
                        (tree_node(ctx, child.id.into_inner(), selected, edit_segment, false))
                    }
                }
            } @else {
                ul .tree-children data-parent-id=(id) {}
            }
        }
    }
}

fn row_actions_menu(
    ctx: &BuilderCtx<'_>,
    row: &ElementRow,
    edit_segment: &str,
    is_root: bool,
) -> Markup {
    let id = row.id;
    let app_slug = &ctx.app.slug;
    let csrf_token = ctx.csrf_token;
    let is_leaf = ElementTag::from_name(&row.tag).map(|t| t.is_leaf()).unwrap_or(true);
    html! {
        details .tree-actions data-actions {
            summary title="Actions" { "⋯" }
            div .tree-actions-menu {
                @if !is_root {
                    (quick_action(app_slug, edit_segment, &id.to_string(), csrf_token,
                        "add-before", "div", "Add before"))
                    (quick_action(app_slug, edit_segment, &id.to_string(), csrf_token,
                        "add-after", "div", "Add after"))
                }
                @if !is_leaf {
                    (quick_child_action(app_slug, edit_segment, &id.to_string(), csrf_token,
                        "div", "Add child"))
                    (quick_child_action(app_slug, edit_segment, &id.to_string(), csrf_token,
                        "#text", "Add text child"))
                }
                @if !is_root {
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
                hx-post=(publish_url) hx-swap="none"
                hx-trigger="change"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                input type="hidden" name="title" value=(ctx.page.title);
                label .checkbox {
                    input type="checkbox" name="publish" value="on"
                        checked[ctx.page.published_at.is_some()];
                    " Published"
                }
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
                label .checkbox {
                    input type="checkbox" name="unlisted" value="on" checked[ctx.page.unlisted];
                    " Unlisted (404 unless owner)"
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
                small .muted .inspector-id { (row.id) }
            }
            nav .inspector-tabs {
                @for t in [InspectorTab::Content, InspectorTab::Attrs, InspectorTab::Style] {
                    a
                        data-tab=(t.slug())
                        aria-current=[if t.slug() == ctx.tab.slug() { Some("true") } else { None }]
                        hx-get=(format!("/apps/{}/pages/{}/inspector?sel={}&tab={}",
                            ctx.app.slug, edit_segment, row.id, t.slug()))
                        hx-target="#inspector"
                        hx-swap="innerHTML" { (t.label()) }
                }
            }
            div .inspector-body {
                @match ctx.tab {
                    InspectorTab::Content => (content_tab(ctx, row)),
                    InspectorTab::Attrs => (attrs_tab(ctx, row)),
                    InspectorTab::Style => (style_tab(ctx, row)),
                }
            }
            (add_child_form(ctx, row))
            @if ctx.page.root_element != Some(row.id.into_inner()) {
                form
                    hx-post=(format!("/apps/{}/pages/{}/elements/{}/delete",
                        ctx.app.slug, edit_segment, row.id))
                    hx-target="#tree"
                    hx-swap="innerHTML"
                    .inspector-delete {
                    input type="hidden" name="_csrf" value=(ctx.csrf_token);
                    button type="submit" .danger { "Delete element" }
                }
            }
        } @else {
            p .muted { "No element selected." }
        }
    }
}

fn add_child_form(ctx: &BuilderCtx<'_>, row: &ElementRow) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let is_root = ctx.page.root_element == Some(row.id.into_inner());
    let is_leaf = ElementTag::from_name(&row.tag).map(|t| t.is_leaf()).unwrap_or(true);
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
                        input type="hidden" name="tag" value="#text";
                        button type="submit" {
                            span .tree-text-glyph { "T" } " Add text child"
                        }
                    }
                }
                @if !is_root {
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
}

fn content_tab(ctx: &BuilderCtx<'_>, row: &ElementRow) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let bind = row
        .attrs_json
        .get("data-lovely-bind")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let (bind_coll, bind_field) = split_ref(bind);
    let source = row
        .attrs_json
        .get("data-lovely-source")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let (src_coll, src_field) = split_ref(source);
    let is_text = row.tag == "#text";
    let is_input_like = matches!(row.tag.as_str(), "input" | "textarea" | "select");
    let patch_url = format!(
        "/apps/{}/pages/{}/elements/{}",
        ctx.app.slug, edit_segment, row.id
    );
    html! {
        @if is_text {
            form
                hx-patch=(patch_url)
                hx-swap="none"
                hx-trigger="input changed delay:400ms, change"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                label {
                    "Text content"
                    textarea name="text" rows="3" autofocus { (row.text.clone().unwrap_or_default()) }
                }
            }
        } @else if !is_input_like {
            p .muted {
                "Text content lives on its own "
                code { "#text" }
                " child element so you can mix inline elements (links, "
                code { "strong" }
                ", etc.) into a paragraph. Use "
                strong { "Add text child" }
                " below."
            }
        }
        @if !ctx.collections.is_empty() {
            (data_ref_section(
                ctx, &patch_url,
                DataDirection::Read,
                bind, bind_coll, bind_field,
            ))
            @if is_input_like {
                (data_ref_section(
                    ctx, &patch_url,
                    DataDirection::Write,
                    source, src_coll, src_field,
                ))
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
    Write,
}

impl DataDirection {
    fn heading(self) -> &'static str {
        match self {
            DataDirection::Read => "Display value (read from data)",
            DataDirection::Write => "Collect value (write into data)",
        }
    }
    fn helper(self) -> &'static str {
        match self {
            DataDirection::Read => {
                "Element shows the field's value from the latest record."
            }
            DataDirection::Write => {
                "On form submit, this input writes its value into a new record."
            }
        }
    }
    fn coll_field(self) -> (&'static str, &'static str) {
        match self {
            DataDirection::Read => ("binding_collection", "binding_field"),
            DataDirection::Write => ("source_collection", "source_field"),
        }
    }
}

fn data_ref_section(
    ctx: &BuilderCtx<'_>,
    patch_url: &str,
    dir: DataDirection,
    current: &str,
    current_coll: &str,
    current_field: &str,
) -> Markup {
    let (coll_name, field_name) = dir.coll_field();
    let active_coll = ctx.collections.iter().find(|c| c.name == current_coll);
    let coll_fields: Vec<String> = active_coll.map(|c| c.fields()).unwrap_or_default();
    html! {
        div .data-binding {
            h3 { (dir.heading()) }
            p .muted { (dir.helper()) }
            form
                hx-patch=(patch_url)
                hx-swap="none"
                hx-trigger="change"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                label {
                    "Collection"
                    select name=(coll_name) {
                        option value="" { "(none)" }
                        @for c in ctx.collections {
                            option value=(c.name) selected[c.name == current_coll] { (c.name) }
                        }
                    }
                }
                label {
                    "Field"
                    select name=(field_name)
                        disabled[active_coll.is_none()] {
                        option value="" { "(pick a field)" }
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
                        input type="hidden" name=(coll_name) value="";
                        input type="hidden" name=(field_name) value="";
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
    match tag {
        "a" => &[
            ("href", "url", "https://example.com"),
            ("target", "text", "_blank"),
            ("rel", "text", "noopener"),
        ],
        "img" => &[
            ("src", "url", "https://example.com/img.png"),
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
        "form" => &[
            ("action", "url", "/path"),
            ("method", "text", "post"),
        ],
        "button" => &[
            ("type", "text", "button"),
            ("name", "text", ""),
        ],
        "label" => &[("for", "text", "")],
        "iframe" => &[
            ("src", "url", ""),
            ("title", "text", ""),
        ],
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
        }
        section .inspector-section {
            h4 { "Other attributes" }
            ul .attrs-list {
                @for (name, value) in &attrs {
                    @if !dedicated.iter().any(|(n, _, _)| n == name) {
                        li {
                            code { (name) }
                            " "
                            span .muted { (value) }
                        }
                    }
                }
                @if attrs.iter().filter(|(n, _)| !dedicated.iter().any(|(d, _, _)| d == n)).count() == 0 {
                    li .muted { "No other attributes." }
                }
            }
            form
                hx-patch=(patch_url)
                hx-swap="none"
                hx-trigger="change, input changed delay:600ms"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                label {
                    "Attribute name"
                    input type="text" name="attr_name" placeholder="class";
                }
                label {
                    "Value"
                    input type="text" name="attr_value" placeholder="hero";
                }
            }
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
