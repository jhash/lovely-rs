//! Full-screen 3-pane page builder layout.
//!
//! Body class `builder` opts the shell out of the centered `<main>` rule.
//! The grid is: top bar (40px) above three columns — tree (≈280px),
//! preview iframe (1fr), inspector (≈320px). Stacks under 80rem.

use crate::views::{builder_shell, ShellCtx};
use lovely_db::{App, Collection, Page, User};
use lovely_tree::{ElementRow, ElementTag};
use maud::{html, Markup};
use uuid::Uuid;

/// What's currently selected in the inspector + tree. `None` means root.
#[derive(Clone, Copy)]
pub enum Selection {
    Root,
    Element(Uuid),
}

impl Selection {
    pub fn from_query(sel: Option<&str>, root: Uuid) -> Self {
        match sel {
            None | Some("") | Some("root") => Selection::Root,
            Some(s) => match Uuid::parse_str(s) {
                Ok(id) if id == root => Selection::Root,
                Ok(id) => Selection::Element(id),
                Err(_) => Selection::Root,
            },
        }
    }

    pub fn id(&self, root: Uuid) -> Uuid {
        match self {
            Selection::Root => root,
            Selection::Element(id) => *id,
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
                    ctx.app.slug, edit_segment, sel_param(ctx.selection, ctx.page.root_element)))
                hx-trigger="preview-stale from:body"
                hx-swap="innerHTML" {
                (tree_fragment(&ctx))
            }
            main .builder-canvas {
                iframe #preview src=(preview_src) {}
            }
            aside #inspector .builder-inspector
                hx-trigger="preview-stale from:body"
                hx-get=(format!("/apps/{}/pages/{}/inspector?sel={}&tab={}",
                    ctx.app.slug, edit_segment,
                    sel_param(ctx.selection, ctx.page.root_element), ctx.tab.slug()))
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
            form method="post"
                 action={"/apps/" (ctx.app.slug) "/pages/" (edit_segment)}
                 .topbar-publish {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                input type="hidden" name="title" value=(ctx.page.title);
                label .checkbox {
                    input type="checkbox" name="publish" value="on"
                        checked[ctx.page.published_at.is_some()]
                        onchange="this.form.submit()";
                    " Published"
                }
            }
            details .topbar-settings {
                summary { "Settings" }
                div .topbar-settings-panel {
                    h4 { "Custom <head> HTML" }
                    p .muted { "Sanitized: no <script> or on* attrs." }
                    form method="post"
                         action={"/apps/" (ctx.app.slug) "/pages/" (edit_segment) "/head"}
                         .auth-form {
                        input type="hidden" name="_csrf" value=(ctx.csrf_token);
                        textarea name="head_html" rows="4" {
                            (ctx.page.head_html)
                        }
                        button type="submit" { "Save" }
                    }
                    h4 { "Access" }
                    form method="post"
                         action={"/apps/" (ctx.app.slug) "/pages/" (edit_segment) "/access"}
                         .auth-form {
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
                        button type="submit" { "Save" }
                    }
                }
            }
            a href=(public_path) target="_blank" rel="noopener" { "View public ↗" }
        }
    }
}

pub fn tree_fragment(ctx: &BuilderCtx<'_>) -> Markup {
    let root_id = ctx.page.root_element.unwrap_or_default();
    let selected = ctx.selection.id(root_id);
    let edit_segment = page_segment(&ctx.page.slug);
    html! {
        ul .tree-list .tree-root data-parent-id=(root_id) {
            (tree_node(ctx, root_id, selected, &edit_segment, true))
        }
    }
}

fn tree_node(
    ctx: &BuilderCtx<'_>,
    id: Uuid,
    selected: Uuid,
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

    let is_selected = id == selected;
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
                        @if is_root { " " span .pill { "root" } }
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
    html! {
        details .tree-actions data-actions {
            summary title="Actions" { "⋯" }
            div .tree-actions-menu {
                @if !is_root {
                    (sibling_action_form(app_slug, edit_segment, &id.to_string(), csrf_token, "add-before", "Add before"))
                    (sibling_action_form(app_slug, edit_segment, &id.to_string(), csrf_token, "add-after", "Add after"))
                }
                (child_action_form(app_slug, edit_segment, &id.to_string(), csrf_token, "Add child"))
                @if !is_root {
                    form
                        hx-post=(format!("/apps/{app_slug}/pages/{edit_segment}/elements/{id}/duplicate"))
                        hx-swap="none"
                        .tree-action-form {
                        input type="hidden" name="_csrf" value=(csrf_token);
                        button type="submit" { "Duplicate" }
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

fn sibling_action_form(
    app_slug: &str,
    edit_segment: &str,
    id: &str,
    csrf_token: &str,
    op: &str,
    label: &str,
) -> Markup {
    html! {
        form
            hx-post=(format!("/apps/{app_slug}/pages/{edit_segment}/elements/{id}/{op}"))
            hx-swap="none"
            .tree-action-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            select name="tag" {
                @for tag in ElementTag::ALL {
                    option value=(tag.name()) selected[tag.name() == "div"] { (tag.name()) }
                }
            }
            button type="submit" { (label) }
        }
    }
}

fn child_action_form(
    app_slug: &str,
    edit_segment: &str,
    parent_id: &str,
    csrf_token: &str,
    label: &str,
) -> Markup {
    html! {
        form
            hx-post=(format!("/apps/{app_slug}/pages/{edit_segment}/elements"))
            hx-swap="none"
            .tree-action-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            input type="hidden" name="parent_id" value=(parent_id);
            select name="tag" {
                @for tag in ElementTag::ALL {
                    option value=(tag.name()) selected[tag.name() == "div"] { (tag.name()) }
                }
            }
            button type="submit" { (label) }
        }
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
    // Build sibling list under the same parent, ordered by chain.
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

pub fn inspector_fragment(ctx: &BuilderCtx<'_>) -> Markup {
    let root = ctx.page.root_element.unwrap_or_default();
    let sel_id = ctx.selection.id(root);
    let edit_segment = page_segment(&ctx.page.slug);
    let row = ctx
        .elements
        .iter()
        .find(|r| r.id.into_inner() == sel_id);
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
                @if ctx.page.root_element == Some(row.id.into_inner()) {
                    " " span .pill { "root" }
                }
                small .muted .inspector-id { (row.id) }
            }
            nav .inspector-tabs {
                @for t in [InspectorTab::Content, InspectorTab::Attrs, InspectorTab::Style] {
                    a
                        data-tab=(t.slug())
                        aria-current=[if t.slug() == ctx.tab.slug() { Some("true") } else { None }]
                        hx-get=(format!("/apps/{}/pages/{}/inspector?sel={}&tab={}",
                            ctx.app.slug, edit_segment, sel_id, t.slug()))
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
    let is_text = row.tag == "#text";
    html! {
        div .inspector-add {
            h3 { "Add element" }
            p .muted {
                "Inline text is its own #text element. "
                "Pick #text from the tag list or use the quick "
                strong { "T" }
                " button."
            }
            form
                hx-post=(format!("/apps/{}/pages/{}/elements",
                    ctx.app.slug, edit_segment))
                hx-swap="none"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                input type="hidden" name="parent_id" value=(row.id);
                label {
                    "Tag"
                    select name="tag" required {
                        @for tag in ElementTag::ALL {
                            option value=(tag.name())
                                selected[tag.name() == "div"] { (tag.name()) }
                        }
                    }
                }
                div .inspector-add-buttons {
                    @if !is_text {
                        button type="submit" { "Add child" }
                    }
                    @if !is_root {
                        button type="submit"
                            formaction=(format!("/apps/{}/pages/{}/elements/{}/add-before",
                                ctx.app.slug, edit_segment, row.id))
                            hx-post=(format!("/apps/{}/pages/{}/elements/{}/add-before",
                                ctx.app.slug, edit_segment, row.id)) { "Add before" }
                        button type="submit"
                            formaction=(format!("/apps/{}/pages/{}/elements/{}/add-after",
                                ctx.app.slug, edit_segment, row.id))
                            hx-post=(format!("/apps/{}/pages/{}/elements/{}/add-after",
                                ctx.app.slug, edit_segment, row.id)) { "Add after" }
                    }
                }
            }
            @if !is_text {
                div .inspector-add-text-quick {
                    form
                        hx-post=(format!("/apps/{}/pages/{}/elements",
                            ctx.app.slug, edit_segment))
                        hx-swap="none"
                        .inline-form {
                        input type="hidden" name="_csrf" value=(ctx.csrf_token);
                        input type="hidden" name="parent_id" value=(row.id);
                        input type="hidden" name="tag" value="#text";
                        button type="submit" title="Add an inline #text child" {
                            span .tree-text-glyph { "T" }
                            " Add text child"
                        }
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
    let (bind_coll, bind_field) = match bind.split_once('.') {
        Some((c, f)) => (c, f),
        None => (bind, ""),
    };
    let is_text = row.tag == "#text";
    html! {
        @if is_text {
            form
                hx-patch=(format!("/apps/{}/pages/{}/elements/{}",
                    ctx.app.slug, edit_segment, row.id))
                hx-swap="none"
                .inspector-form {
                input type="hidden" name="_csrf" value=(ctx.csrf_token);
                label {
                    "Text content"
                    textarea name="text" rows="3" autofocus { (row.text.clone().unwrap_or_default()) }
                }
                button type="submit" { "Save" }
            }
        } @else {
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
            div .data-binding {
                h3 { "Bind to data" }
                form
                    hx-patch=(format!("/apps/{}/pages/{}/elements/{}",
                        ctx.app.slug, edit_segment, row.id))
                    hx-swap="none"
                    .inspector-form {
                    input type="hidden" name="_csrf" value=(ctx.csrf_token);
                    label {
                        "Collection"
                        select name="binding_collection" {
                            option value="" { "(none)" }
                            @for c in ctx.collections {
                                option value=(c.name) selected[c.name == bind_coll] { (c.name) }
                            }
                        }
                    }
                    label {
                        "Field"
                        input type="text" name="binding_field" value=(bind_field) placeholder="title";
                    }
                    button type="submit" { "Save binding" }
                }
                @if !bind.is_empty() {
                    p .muted { "Currently bound to " code { (bind) } }
                }
            }
        } @else {
            p .muted .data-binding-empty {
                "No collections in this app. "
                a href={"/apps/" (ctx.app.slug) "/data"} { "Create one" }
                " to bind data."
            }
        }
    }
}

fn attrs_tab(ctx: &BuilderCtx<'_>, row: &ElementRow) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    let attrs: Vec<(String, String)> = row
        .attrs_json
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    html! {
        ul .attrs-list {
            @for (name, value) in &attrs {
                li {
                    code { (name) }
                    " "
                    span .muted { (value) }
                }
            }
            @if attrs.is_empty() {
                li .muted { "No attributes set." }
            }
        }
        form
            hx-patch=(format!("/apps/{}/pages/{}/elements/{}",
                ctx.app.slug, edit_segment, row.id))
            hx-swap="none"
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
            button type="submit" { "Set attribute" }
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
    html! {
        form
            hx-patch=(format!("/apps/{}/pages/{}/elements/{}",
                ctx.app.slug, edit_segment, row.id))
            hx-swap="none"
            .inspector-form {
            input type="hidden" name="_csrf" value=(ctx.csrf_token);
            input type="hidden" name="attr_name" value="style";
            label {
                "Inline style (CSS)"
                textarea name="attr_value" rows="6" placeholder="padding: 1rem; color: #c026d3;" {
                    (current_style)
                }
            }
            button type="submit" { "Save style" }
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

fn sel_param(sel: Selection, root: Option<Uuid>) -> String {
    match sel {
        Selection::Root => root.map(|r| r.to_string()).unwrap_or_else(|| "root".into()),
        Selection::Element(id) => id.to_string(),
    }
}
