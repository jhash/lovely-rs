use crate::views::{public_shell, shell, ShellCtx};
use lovely_db::{App, Page, User};
use maud::{html, Markup, PreEscaped};

pub fn pages_new(user: &User, app: &App, csrf_token: &str, error: Option<&str>) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / New page"
        }
        h1 { "New page in " (app.name) }
        form method="post" action={"/apps/" (app.slug) "/pages"} .auth-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            label {
                "Slug (URL segment, leave empty for the home page)"
                input type="text" name="slug" pattern="[a-z0-9-]*" maxlength="80"
                      placeholder="about-us";
            }
            label {
                "Title"
                input type="text" name="title" required maxlength="200";
            }
            label {
                "Description (optional)"
                textarea name="description" rows="3" maxlength="500" {}
            }
            @if let Some(msg) = error { p .error { (msg) } }
            button type="submit" { "Create" }
        }
    };
    shell(
        ShellCtx {
            title: "New page",
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}

pub fn published_page(
    viewer: Option<&User>,
    owner_id: uuid::Uuid,
    app_slug: &str,
    page: &Page,
    rendered_tree: Markup,
    csrf_token: &str,
) -> Markup {
    let is_owner = viewer.map(|v| v.id == owner_id).unwrap_or(false);
    let edit_segment = if page.slug.is_empty() {
        "~home".to_string()
    } else {
        page.slug.clone()
    };
    let edit_href = format!("/apps/{app_slug}/pages/{edit_segment}/edit");
    // Public pages are 100% the user's. No app-injected h1/description.
    let body = html! {
        (PreEscaped(rendered_tree.into_string()))
    };
    public_shell(
        ShellCtx {
            title: &page.title,
            description: page.description.as_deref(),
            user: viewer,
            csrf_token,
        },
        Some(&edit_href),
        is_owner,
        body,
    )
}
