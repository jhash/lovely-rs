use crate::views::{shell, ShellCtx};
use lovely_db::{App, Page, User};
use maud::{html, Markup};

pub fn apps_index(user: &User, apps: &[App], csrf_token: &str) -> Markup {
    let body = html! {
        h1 { "Your apps" }
        @if apps.is_empty() {
            p .muted { "No apps yet. (One will be created automatically when you register.)" }
        } @else {
            ul .app-list {
                @for app in apps {
                    li {
                        a href={"/apps/" (app.slug)} { (app.name) }
                        @if app.is_default { " " span .muted { "(default)" } }
                    }
                }
            }
        }
    };
    shell(
        ShellCtx {
            title: "Apps",
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}

pub fn app_dashboard(user: &User, app: &App, pages: &[Page], csrf_token: &str) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / " (app.name)
        }
        h1 { (app.name) }
        @if let Some(d) = &app.description { p .muted { (d) } }
        section {
            h2 { "Pages" }
            p {
                a href={"/apps/" (app.slug) "/pages/new"} .button { "New page" }
            }
            @if pages.is_empty() {
                p .muted { "No pages yet." }
            } @else {
                ul .page-list {
                    @for page in pages {
                        li {
                            @let edit_segment = if page.slug.is_empty() { "~home" } else { page.slug.as_str() };
                            a href={"/apps/" (app.slug) "/pages/" (edit_segment) "/edit"} { (page.title) }
                            " "
                            @if page.slug.is_empty() {
                                span .muted { "(home)" }
                            } @else {
                                code .muted { "/" (user.username) "/" (page.slug) }
                            }
                            @if page.published_at.is_some() {
                                " " span .pill .pill-published { "published" }
                            } @else {
                                " " span .pill .pill-draft { "draft" }
                            }
                        }
                    }
                }
            }
        }
    };
    shell(
        ShellCtx {
            title: &app.name,
            description: app.description.as_deref(),
            user: Some(user),
            csrf_token,
        },
        body,
    )
}
