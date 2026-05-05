use crate::state::AppState;
use crate::WebError;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::CookieJar;
use lovely_db::User;

pub const SESSION_COOKIE: &str = "lovely_session";

#[derive(Clone, Debug)]
pub struct AuthUser(pub User);

impl AuthUser {
    pub fn user(&self) -> &User {
        &self.0
    }
}

#[derive(Clone, Debug, Default)]
pub struct MaybeUser(pub Option<User>);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match resolve_user(parts, state).await? {
            Some(u) => Ok(AuthUser(u)),
            None => Err(WebError::Unauthorized),
        }
    }
}

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = WebError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(MaybeUser(resolve_user(parts, state).await?))
    }
}

async fn resolve_user(parts: &Parts, state: &AppState) -> Result<Option<User>, WebError> {
    let jar = CookieJar::from_headers(&parts.headers);
    let Some(session_cookie) = jar.get(SESSION_COOKIE) else {
        return Ok(None);
    };
    let session = match lovely_db::find_session(&state.pg, session_cookie.value()).await? {
        Some(s) => s,
        None => return Ok(None),
    };
    let user = lovely_db::find_user_by_id(&state.pg, session.user_id).await?;
    Ok(user)
}
