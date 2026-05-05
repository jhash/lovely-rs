use lovely_db::User;
use maud::{html, Markup, DOCTYPE};

pub struct ShellCtx<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub user: Option<&'a User>,
    pub csrf_token: &'a str,
}

pub fn shell(ctx: ShellCtx<'_>, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            (head_common(&ctx))
            body {
                (top_nav(ctx.user))
                main { (body) }
            }
        }
    }
}

/// Builder shell — full-screen, edge-to-edge, no centered `<main>`. The
/// nav stays so the editor still feels like part of the app.
pub fn builder_shell(ctx: ShellCtx<'_>, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            (head_common(&ctx))
            body class="builder" {
                (top_nav(ctx.user))
                (body)
            }
        }
    }
}

/// Public shell — no editor chrome at all. The user's page is the page.
/// An owner viewing their own page gets a small floating "edit" badge so
/// they can hop back into the builder.
pub fn public_shell(
    ctx: ShellCtx<'_>,
    edit_href: Option<&str>,
    is_owner: bool,
    body: Markup,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            (head_common(&ctx))
            body class="public" {
                (body)
                @if is_owner {
                    @if let Some(href) = edit_href {
                        a .owner-edit-badge href=(href) { "Edit" }
                    }
                }
            }
        }
    }
}

fn head_common(ctx: &ShellCtx<'_>) -> Markup {
    // ASSET_VERSION busts browser caches whenever this binary is rebuilt.
    // It's the build timestamp baked in at compile time.
    let v = env!("ASSET_VERSION");
    html! {
        head {
            meta charset="utf-8";
            meta name="viewport" content="width=device-width, initial-scale=1";
            title { (ctx.title) }
            @if let Some(d) = ctx.description {
                meta name="description" content=(d);
            }
            meta name="csrf-token" content=(ctx.csrf_token);
            link rel="stylesheet" href={"/static/style.css?v=" (v)};
            script src="https://unpkg.com/htmx.org@2.0.4" defer {}
            script src="https://cdn.jsdelivr.net/npm/sortablejs@1.15.3/Sortable.min.js" defer {}
            script src={"/static/tree.js?v=" (v)} defer {}
        }
    }
}

fn top_nav(user: Option<&User>) -> Markup {
    html! {
        nav .top-nav {
            div .container {
                a .brand href="/" { "lovely" }
                div .spacer {}
                @if let Some(u) = user {
                    a href="/apps" { "Apps" }
                    a href={"/" (u.username)} { "/" (u.username) }
                    form method="post" action="/auth/logout" .inline-form {
                        button type="submit" { "Sign out (" (u.username) ")" }
                    }
                } @else {
                    a href="/auth/login" { "Sign in" }
                    a href="/auth/register" { "Register" }
                }
            }
        }
    }
}
