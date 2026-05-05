use crate::views::{shell, ShellCtx};
use lovely_db::{Page, User};
use maud::{html, Markup, PreEscaped};

pub fn pages_index(user: &User, pages: &[Page], csrf_token: &str) -> Markup {
    let body = html! {
        h1 { "Your pages" }
        a href="/pages/new" .button { "New page" }
        @if pages.is_empty() {
            p .muted { "No pages yet." }
        } @else {
            ul .page-list {
                @for p in pages {
                    li {
                        a href={"/pages/" (p.slug)} { (p.title) }
                        span .muted { " " (p.slug) }
                    }
                }
            }
        }
    };
    shell(
        ShellCtx {
            title: "Pages",
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}

pub fn pages_new(user: &User, csrf_token: &str, error: Option<&str>) -> Markup {
    let body = html! {
        h1 { "New page" }
        form method="post" action="/pages" .auth-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            label { "Slug (URL)" input type="text" name="slug" pattern="[a-z0-9-]+" required minlength="1" maxlength="80"; }
            label { "Title" input type="text" name="title" required maxlength="200"; }
            label { "Description (optional)" textarea name="description" rows="3" maxlength="500" {} }
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

pub fn published_page(page: &Page, rendered_tree: maud::Markup, csrf_token: &str) -> Markup {
    let body = html! {
        article .published-page {
            h1 { (page.title) }
            @if let Some(d) = &page.description { p .lead { (d) } }
            (PreEscaped(rendered_tree.into_string()))
        }
    };
    shell(
        ShellCtx {
            title: &page.title,
            description: page.description.as_deref(),
            user: None,
            csrf_token,
        },
        body,
    )
}
