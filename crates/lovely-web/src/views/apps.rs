use crate::views::{shell, ShellCtx};
use lovely_db::{App, Page, User};
use maud::{html, Markup};

pub fn apps_index(user: &User, apps: &[App], csrf_token: &str) -> Markup {
    let body = html! {
        div .page-header {
            h1 { "Your apps" }
            a href="/apps/new" .button { "New app" }
        }
        @if apps.is_empty() {
            p .muted { "No apps yet." }
        } @else {
            ul .app-list {
                @for app in apps {
                    li {
                        a href={"/apps/" (app.slug)} { (app.name) }
                        @if app.is_default { " " span .pill { "default" } }
                        @if let Some(d) = &app.description { " " span .muted { (d) } }
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

pub fn apps_new(user: &User, csrf_token: &str, error: Option<&str>) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / New app"
        }
        h1 { "New app" }
        form method="post" action="/apps" .auth-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            label {
                "Slug (URL segment)"
                input type="text" name="slug" pattern="[a-z0-9-]+" maxlength="40"
                      required placeholder="my-blog";
            }
            label {
                "Name"
                input type="text" name="name" required maxlength="120";
            }
            label {
                "Description (optional)"
                textarea name="description" rows="2" maxlength="500" {}
            }
            @if let Some(msg) = error { p .error { (msg) } }
            button type="submit" { "Create app" }
        }
    };
    shell(
        ShellCtx {
            title: "New app",
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
        div .page-header {
            h1 { (app.name) }
            div .header-actions {
                a href={"/apps/" (app.slug) "/pages/new"} .button { "New page" }
                a href={"/apps/" (app.slug) "/data"} .button { "Data" }
            }
        }
        @if let Some(d) = &app.description { p .muted { (d) } }
        section {
            h2 { "Pages" }
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
        section .app-settings {
            h2 { "App settings" }
            form method="post" action={"/apps/" (app.slug) "/rename"} .auth-form {
                input type="hidden" name="_csrf" value=(csrf_token);
                label {
                    "Name"
                    input type="text" name="name" value=(app.name) required;
                }
                label {
                    "Slug"
                    input type="text" name="slug" value=(app.slug) required pattern="[a-z0-9-]+" maxlength="40";
                }
                label {
                    "Description"
                    textarea name="description" rows="2" {
                        (app.description.clone().unwrap_or_default())
                    }
                }
                button type="submit" { "Save" }
            }
            @if !app.is_default {
                form method="post" action={"/apps/" (app.slug) "/delete"}
                     .delete-form
                     onsubmit="return confirm('Delete this app and all its pages?')" {
                    input type="hidden" name="_csrf" value=(csrf_token);
                    button type="submit" .danger { "Delete app" }
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
