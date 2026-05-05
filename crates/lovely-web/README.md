# lovely-web

The HTTP layer: axum 0.8 router, maud views, htmx wiring, auth (username/password + OAuth scaffolding + TOTP scaffolding), CSRF, sessions.

Handlers take `State<AppState>` plus extractors (`AuthUser`, `MaybeUser`, `CsrfToken`, `Form<…>`). They return `Result<Response, WebError>`. `WebError`'s `IntoResponse` impl is htmx-aware: `Unauthorized` becomes `303 + HX-Redirect: /auth/login` for both htmx and plain browser requests.

Views in `views/` build maud trees that go through `views::shell` for the page chrome (top nav, csrf-token meta, /static/style.css link, htmx + tree.js scripts).

The page tree itself is rendered via `lovely_tree::Tree::render` — that crate is wired in with `features = ["render"]`.

## Tests

Unit tests for password hashing and CSRF token verification run in `cargo test`. End-to-end tests in `tests/` boot a full server via `lovely-test-support::TestApp`; they require Docker and are `#[ignore]` by default.

## What depends on this

`lovely-server` (the binary that constructs `AppState` and serves the router).
