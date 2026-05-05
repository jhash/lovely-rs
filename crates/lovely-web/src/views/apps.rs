use crate::views::{shell, ShellCtx};
use lovely_db::{App, Collection, Page, User};
use maud::{html, Markup};

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum AppTab {
    Pages,
    Data,
    Settings,
}

/// Sub-nav rendered on every /apps/{slug}* page so users can jump
/// between Pages, Data, and Settings without re-thinking the URL.
pub fn app_subnav(app: &App, active: AppTab) -> Markup {
    let tab = |label: &str, href: String, kind: AppTab| {
        let is_active = active == kind;
        html! {
            a href=(href)
                aria-current=[if is_active { Some("page") } else { None }]
                class=[if is_active { Some("active") } else { None }] { (label) }
        }
    };
    html! {
        nav .app-subnav aria-label="App sections" {
            div .app-subnav-inner {
                (tab("Pages", format!("/apps/{}", app.slug), AppTab::Pages))
                (tab("Data", format!("/apps/{}/data", app.slug), AppTab::Data))
                (tab("Settings", format!("/apps/{}/settings", app.slug), AppTab::Settings))
            }
        }
    }
}

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

/// Dashboard = pages summary + data summary, nothing else. App-level
/// rename/theme/delete live on /apps/{slug}/settings.
pub fn app_dashboard(
    user: &User,
    app: &App,
    pages: &[Page],
    collections: &[Collection],
    csrf_token: &str,
) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / " (app.name)
        }
        h1 { (app.name) }
        @if let Some(d) = &app.description { p .muted { (d) } }
        (app_subnav(app, AppTab::Pages))

        (pages_summary_section(user, app, pages))
        (data_summary_section(app, collections))
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

/// /apps/{slug}/pages — same Pages list as the dashboard, on its own
/// page (parity with /data).
pub fn app_pages_index(user: &User, app: &App, pages: &[Page], csrf_token: &str) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / Pages"
        }
        (app_subnav(app, AppTab::Pages))
        (pages_summary_section(user, app, pages))
    };
    shell(
        ShellCtx {
            title: &format!("Pages — {}", app.name),
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}

fn pages_summary_section(user: &User, app: &App, pages: &[Page]) -> Markup {
    html! {
        section .summary-section {
            div .section-header {
                h2 {
                    a href={"/apps/" (app.slug) "/pages"} { "Pages" }
                }
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
    }
}

fn data_summary_section(app: &App, collections: &[Collection]) -> Markup {
    html! {
        section .summary-section {
            div .section-header {
                h2 {
                    a href={"/apps/" (app.slug) "/data"} { "Data" }
                }
                a href={"/apps/" (app.slug) "/data/new"} .button { "New collection" }
            }
            @if collections.is_empty() {
                p .muted { "No collections yet." }
            } @else {
                ul .page-list {
                    @for c in collections {
                        li {
                            a href={"/apps/" (app.slug) "/data/" (c.name)} {
                                code { (c.name) }
                            }
                            " "
                            span .muted { "(" (c.fields().join(", ")) ")" }
                        }
                    }
                }
            }
        }
    }
}

/// /apps/{slug}/settings — rename, delete, theme.
pub fn app_settings(user: &User, app: &App, csrf_token: &str) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / Settings"
        }
        (app_subnav(app, AppTab::Settings))
        h1 { "Settings — " (app.name) }

        section .app-settings {
            h2 { "Identity" }
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
        }

        section .app-settings {
            h2 { "Theme" }
            p .muted { "Variables become CSS custom properties on every public page in this app: --lovely-primary, --lovely-background, --lovely-ink, --lovely-font." }
            form method="post" action={"/apps/" (app.slug) "/theme"} .auth-form {
                input type="hidden" name="_csrf" value=(csrf_token);
                @let theme = app.theme_json.as_object();
                @let get = |k: &str| theme.and_then(|m| m.get(k)).and_then(|v| v.as_str()).unwrap_or("");
                label {
                    "Primary color"
                    input type="text" name="primary" value=(get("primary")) placeholder="#c026d3";
                }
                label {
                    "Background color"
                    input type="text" name="background" value=(get("background")) placeholder="#ffffff";
                }
                label {
                    "Ink color"
                    input type="text" name="ink" value=(get("ink")) placeholder="#000000";
                }
                label {
                    "Font family"
                    input type="text" name="font" value=(get("font")) placeholder="Lora, serif";
                }
                button type="submit" { "Save theme" }
            }
        }

        @if !app.is_default {
            section .app-settings {
                h2 { "Danger zone" }
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
            title: &format!("Settings — {}", app.name),
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}
