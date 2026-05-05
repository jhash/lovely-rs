//! Full-screen 3-pane page builder layout.
//!
//! Body class `builder` opts the shell out of the centered `<main>` rule.
//! The grid is: top bar (40px) above three columns — tree (≈280px),
//! preview iframe (1fr), inspector (≈320px). Stacks under 80rem.

use crate::views::{builder_shell, ShellCtx};
use lovely_db::{App, Page, User};
use lovely_tree::ElementRow;
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
    // Children sorted by sibling chain.
    let mut children: Vec<&lovely_tree::ElementRow> = ctx
        .elements
        .iter()
        .filter(|r| r.parent_id.map(|p| p.into_inner()) == Some(id))
        .collect();
    children.sort_by_key(|r| sibling_index(ctx.elements, r.id.into_inner()));

    let is_selected = id == selected;
    html! {
        li
            data-element-id=(id)
            aria-current=[if is_selected { Some("true") } else { None }] {
            div .tree-row
                hx-get=(format!("/apps/{}/pages/{}/inspector?sel={}",
                    ctx.app.slug, edit_segment, id))
                hx-target="#inspector"
                hx-swap="innerHTML"
                hx-push-url=(format!("/apps/{}/pages/{}/edit?sel={}",
                    ctx.app.slug, edit_segment, id)) {
                code { (row.tag) }
                @if is_root { " " span .pill { "root" } }
                @if let Some(t) = &row.text {
                    " " span .muted { (t.chars().take(24).collect::<String>()) }
                }
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
                code { (row.tag) }
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

fn content_tab(ctx: &BuilderCtx<'_>, row: &ElementRow) -> Markup {
    let edit_segment = page_segment(&ctx.page.slug);
    html! {
        form
            hx-patch=(format!("/apps/{}/pages/{}/elements/{}",
                ctx.app.slug, edit_segment, row.id))
            hx-swap="none"
            .inspector-form {
            input type="hidden" name="_csrf" value=(ctx.csrf_token);
            label {
                "Text content"
                input type="text" name="text" value=(row.text.clone().unwrap_or_default());
            }
            button type="submit" { "Save" }
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
