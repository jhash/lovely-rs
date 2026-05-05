# lovely-test-support

Path-only dev-dependency crate. Never published. Provides:

- `PgTestContainer` — boots `postgres:17` via `testcontainers-rs` and creates fresh databases on demand with migrations applied.
- `TestApp` — boots `lovely-web` on an ephemeral port with a `reqwest::Client` ready to drive it (cookie jar enabled, redirects disabled).
- `csrf_token()` — convenience helper that hits `/auth/login` and parses the resulting `csrf_token` cookie out of the `Set-Cookie` header.

Anything in here is fair game to break between releases — there are no API guarantees.
