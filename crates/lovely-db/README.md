# lovely-db

The data layer: a `PgPool` plus repos for users, sessions, oauth_identities, pages, and elements; the `SqliteAppStore` trait that abstracts per-app SQLite (so the web binary can stay agnostic of where the SQLite files live).

Repos are plain functions taking `&PgPool`. No service objects. No ORM. `sqlx::query_as::<_, T>()` with `FromRow` derives — runtime-checked queries (we'll switch to compile-time `query_as!` once the dev-loop can rely on a live DB).

## Tests

Integration tests in `tests/` boot Postgres via `testcontainers-rs`. They are `#[ignore]` by default so `cargo test` succeeds without Docker. Run with:

```sh
cargo test -p lovely-db -- --ignored
```

## What depends on this

`lovely-web` (handlers call repos directly), `lovely-server` (constructs the pool), `lovely-test-support` (boots the test container).
