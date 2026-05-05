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
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (ctx.title) }
                @if let Some(d) = ctx.description {
                    meta name="description" content=(d);
                }
                meta name="csrf-token" content=(ctx.csrf_token);
                link rel="stylesheet" href="/static/style.css";
                script src="https://unpkg.com/htmx.org@2.0.4" defer {}
                script src="/static/tree.js" defer {}
            }
            body {
                (top_nav(ctx.user))
                main { (body) }
            }
        }
    }
}

fn top_nav(user: Option<&User>) -> Markup {
    html! {
        nav .top-nav {
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
