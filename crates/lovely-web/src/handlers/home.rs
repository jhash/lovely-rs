use crate::auth::{csrf, MaybeUser};
use crate::state::AppState;
use crate::views::{shell, ShellCtx};
use axum::extract::State;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::CookieJar;
use maud::html;

pub async fn home(
    State(state): State<AppState>,
    MaybeUser(user): MaybeUser,
    jar: CookieJar,
) -> Response {
    if user.is_some() {
        return Redirect::to("/apps").into_response();
    }
    let (jar, token) = csrf::ensure_cookie(jar, &state.base_url);
    let body = html! {
        section .hero {
            h1 { "lovely" }
            p .lead { "A dynamic site builder." }
            p {
                a href="/auth/login" .button { "Sign in" }
                " "
                a href="/auth/register" { "Register" }
            }
        }
    };
    let markup = shell(
        ShellCtx {
            title: "lovely",
            description: Some("A dynamic site builder"),
            user: None,
            csrf_token: &token,
        },
        body,
    );
    (jar, markup).into_response()
}
