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
    theme_json: &serde_json::Value,
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
    let theme_css = theme_to_css(theme_json);
    let extra_head = if page.head_html.is_empty() {
        None
    } else {
        Some(page.head_html.as_str())
    };
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
        theme_css,
        extra_head,
        body,
    )
}

pub fn user_profile(
    owner: &User,
    apps: &[lovely_db::App],
    viewer: Option<&User>,
    csrf_token: &str,
) -> Markup {
    use crate::views::public_shell;
    let is_owner = viewer.map(|v| v.id == owner.id).unwrap_or(false);
    let is_public = owner.public_published_at.is_some();
    let body = html! {
        article .published-page {
            header .profile-header {
                h1 { "@" (owner.username) }
                @if is_public {
                    span .pill .pill-published { "published" }
                } @else if is_owner {
                    span .pill .pill-draft { "draft" }
                }
            }
            @if apps.is_empty() {
                p .muted { "No apps yet." }
            } @else {
                ul .app-list {
                    @for a in apps {
                        li {
                            a href={"/" (owner.username) "/" (a.slug)} { (a.name) }
                            " "
                            @if a.published_at.is_some() {
                                span .pill .pill-published { "published" }
                            } @else if is_owner {
                                span .pill .pill-draft { "draft" }
                            }
                            @if let Some(d) = &a.description { " " span .muted { (d) } }
                        }
                    }
                }
            }
        }
    };
    public_shell(
        ShellCtx {
            title: &owner.username,
            description: None,
            user: viewer,
            csrf_token,
        },
        if is_owner { Some("/apps") } else { None },
        is_owner,
        None,
        None,
        body,
    )
}

pub fn password_gate(page: &Page, username: &str, slug: &str, csrf_token: &str) -> Markup {
    let body = html! {
        article .password-gate {
            h1 { "Password required" }
            p .muted { "This page is protected. Enter the password to continue." }
            form method="post" action={"/p/" (username) "/" (slug) "/_unlock"} .auth-form {
                input type="hidden" name="_csrf" value=(csrf_token);
                label {
                    "Password"
                    input type="password" name="password" required autofocus;
                }
                button type="submit" { "Unlock" }
            }
        }
    };
    shell(
        ShellCtx {
            title: &format!("{} — Locked", page.title),
            description: None,
            user: None,
            csrf_token,
        },
        body,
    )
}

fn theme_to_css(theme: &serde_json::Value) -> Option<String> {
    let map = theme.as_object()?;
    if map.is_empty() {
        return None;
    }
    let mut s = String::from(":root {");
    for (k, v) in map {
        if let Some(value) = v.as_str() {
            s.push_str(&format!(" --lovely-{k}: {value};"));
        }
    }
    s.push('}');
    Some(s)
}
