# Plan: GitHub & Google OAuth

Goal: add `/auth/github` → GitHub → `/auth/github/callback` and the same for Google.
On success: upsert user + identity via existing `upsert_oauth_identity`, set `lovely_session` cookie, redirect to `/apps`.

## What's already in place

- `oauth_identities` table + `upsert_oauth_identity` in `lovely-db` — ready to use, no DB changes needed
- `LOVELY_GITHUB_CLIENT_ID/SECRET` and `LOVELY_GOOGLE_CLIENT_ID/SECRET` read from env, already in `AppState` config in `main.rs` and in the stack template
- `lovely_session` cookie + `SESSION_COOKIE` constant in `auth/extractor.rs` — same cookie the username flow sets
- OAuth buttons already in the login view pointing at `/auth/github` and `/auth/google`

## Reference implementations

- **Go** (`go-jake/authentication.go`): uses `markbates/goth` which abstracts the PKCE dance. Pattern: `BeginAuthHandler` on `/auth/{provider}`, `CompleteUserAuth` on callback, then `findOrCreateUser` + JWT cookie.
- **Swift** (`lovely/Packages/swoth/`): rolled manually — `SwothClient.beginAuth` generates state + redirect URL; `completeAuth` verifies state, exchanges code, fetches user. Providers are structs that implement a `Provider` protocol.

For Rust, roll it the same way swoth does — no macro-heavy crate needed.

## Crates to add

```toml
# lovely-web/Cargo.toml
reqwest = { version = "0.12", features = ["json"] }
```

`reqwest` for the server-side token exchange and user-info calls.  
No dedicated OAuth crate needed — the flow is simple enough to write directly (as swoth demonstrates).

## New files

```
crates/lovely-web/src/handlers/auth_oauth.rs
crates/lovely-web/src/oauth/mod.rs        # provider trait + GitHub + Google impls
```

## Step-by-step

### 1. Add `OAuthConfig` to `AppState`

In `crates/lovely-web/src/state.rs`, add:

```rust
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: SecretString,
    pub callback_url: String,
}

pub struct AppState {
    // ... existing fields ...
    pub github_oauth: Option<OAuthConfig>,
    pub google_oauth:  Option<OAuthConfig>,
}
```

Wire from `main.rs` — already reads the env vars, just pass them into `AppState::new`.

### 2. Provider trait + implementations

`crates/lovely-web/src/oauth/mod.rs`:

```rust
pub struct OAuthUser {
    pub provider_user_id: String,
    pub email: Option<String>,
    pub username: String,        // used as fallback_username if new user
    pub raw_profile: serde_json::Value,
}

pub trait OAuthProvider {
    fn authorization_url(&self, state: &str) -> String;
    // called server-side after callback
    async fn exchange_code(&self, code: &str) -> anyhow::Result<String>; // returns access_token
    async fn fetch_user(&self, access_token: &str) -> anyhow::Result<OAuthUser>;
}
```

**GitHub** (`oauth/github.rs`):
- Auth URL: `https://github.com/login/oauth/authorize?client_id=...&redirect_uri=...&scope=user:email+read:user&state=...`
- Token endpoint: POST `https://github.com/login/oauth/access_token` (Accept: application/json)
- User endpoint: GET `https://api.github.com/user` — if `email` is null, GET `https://api.github.com/user/emails` and pick the primary+verified one (same as swoth's `fetchPrimaryEmail`)
- `username` = `login` field

**Google** (`oauth/google.rs`):
- Auth URL: `https://accounts.google.com/o/oauth2/v2/auth?...&scope=openid+email+profile`
- Token endpoint: POST `https://oauth2.googleapis.com/token`
- User endpoint: GET `https://www.googleapis.com/oauth2/v3/userinfo` — has `sub`, `email`, `name`
- `username` = slugify `name` or `email` prefix

See swoth's `GoogleProvider.swift` for exact field names in the token and userinfo responses.

### 3. CSRF state via existing cookie pattern

Follow `csrf.rs` — generate a random hex state, store in a short-lived `oauth_state` cookie (SameSite=Lax), verify on callback. Delete the cookie after verification.

```rust
const OAUTH_STATE_COOKIE: &str = "lovely_oauth_state";
```

### 4. Handlers (`auth_oauth.rs`)

```rust
// GET /auth/github
pub async fn begin_github(State(state): State<AppState>, jar: CookieJar) -> Response {
    let cfg = state.github_oauth.as_ref() /* return 404 if None */;
    let oauth_state = generate_state();
    let url = github_provider(cfg).authorization_url(&oauth_state);
    let cookie = Cookie::build((OAUTH_STATE_COOKIE, oauth_state))
        .same_site(SameSite::Lax).http_only(true).path("/").build();
    (jar.add(cookie), Redirect::to(&url)).into_response()
}

// GET /auth/github/callback?code=...&state=...
pub async fn callback_github(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(params): Query<CallbackParams>,
) -> Result<Response, WebError> {
    // 1. verify state cookie
    // 2. exchange code for access_token
    // 3. fetch OAuthUser
    // 4. upsert_oauth_identity(pool, UpsertOAuth { ... })
    // 5. set lovely_session cookie (same as post_login does)
    // 6. Redirect::to("/apps")
}
```

Repeat for Google.

### 5. Wire routes in `router.rs`

```rust
.route("/auth/github",           get(handlers::auth_oauth::begin_github))
.route("/auth/github/callback",  get(handlers::auth_oauth::callback_github))
.route("/auth/google",           get(handlers::auth_oauth::begin_google))
.route("/auth/google/callback",  get(handlers::auth_oauth::callback_google))
```

### 6. Update OAuth app settings

- **GitHub**: https://github.com/settings/developers — set callback to `https://lovely.jakehash.com/auth/github/callback`
- **Google**: https://console.cloud.google.com/apis/credentials — add `https://lovely.jakehash.com/auth/google/callback` as authorized redirect URI

## Session cookie (how post_login does it — mirror exactly)

Look at `auth_username.rs::post_login` for how it builds and sets `lovely_session`. Do the same in the OAuth callback handlers so the session extractor works without changes.

## Error handling

If `github_oauth` / `google_oauth` is `None` (env vars not set), return `StatusCode::NOT_FOUND` from both routes so local dev without credentials fails cleanly instead of panicking.
