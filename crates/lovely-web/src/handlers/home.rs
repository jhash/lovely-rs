use crate::views::{shell, ShellCtx};
use axum::response::IntoResponse;
use maud::html;

pub async fn home() -> impl IntoResponse {
    let csrf = ""; // CSRF middleware will populate this in a later step.
    let body = html! {
        h1 { "lovely" }
        p { "A dynamic site builder." }
        p { a href="/auth/login" { "Sign in" } " or " a href="/auth/register" { "Register" } "." }
    };
    axum::response::Html(
        shell(
            ShellCtx {
                title: "lovely",
                description: Some("A dynamic site builder"),
                user: None,
                csrf_token: csrf,
            },
            body,
        )
        .into_string(),
    )
}
