use crate::WebError;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use rand::RngCore;

pub const CSRF_COOKIE: &str = "csrf_token";
pub const CSRF_HEADER: &str = "x-csrf-token";
pub const CSRF_FORM_FIELD: &str = "_csrf";

#[derive(Clone, Debug)]
pub struct CsrfToken(pub String);

impl CsrfToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push(hex_nibble(b >> 4));
        out.push(hex_nibble(b & 0x0F));
    }
    out
}

fn hex_nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => unreachable!(),
    }
}

pub fn ensure_cookie<'a>(jar: CookieJar, base_url: &str) -> (CookieJar, String) {
    if let Some(c) = jar.get(CSRF_COOKIE) {
        return (jar.clone(), c.value().to_string());
    }
    let token = generate_token();
    let secure = base_url.starts_with("https://");
    let cookie = Cookie::build((CSRF_COOKIE, token.clone()))
        .path("/")
        .http_only(false) // tree.js reads it for the htmx header
        .same_site(SameSite::Lax)
        .secure(secure);
    (jar.add(cookie), token)
}

impl<S: Send + Sync> FromRequestParts<S> for CsrfToken {
    type Rejection = WebError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(CSRF_COOKIE)
            .map(|c| c.value().to_string())
            .ok_or(WebError::Csrf)?;
        Ok(CsrfToken(token))
    }
}

/// Verify that the cookie token matches the value supplied via header
/// (`X-CSRF-Token`) or form field (`_csrf`). On failure returns
/// [`WebError::Csrf`] which becomes a 403.
pub fn verify_token(cookie_token: &str, header_or_form: Option<&str>) -> Result<(), WebError> {
    let supplied = header_or_form.ok_or(WebError::Csrf)?;
    if !constant_time_eq(cookie_token.as_bytes(), supplied.as_bytes()) {
        return Err(WebError::Csrf);
    }
    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verify_matches() {
        let t = generate_token();
        assert!(verify_token(&t, Some(&t)).is_ok());
    }
    #[test]
    fn verify_rejects_mismatch() {
        let t = generate_token();
        assert!(verify_token(&t, Some("other")).is_err());
        assert!(verify_token(&t, None).is_err());
    }
    #[test]
    fn token_is_64_hex_chars() {
        let t = generate_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
