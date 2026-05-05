# lovely-rs

A dynamic site builder in Rust. Server-rendered HTML with htmx, no SPA. Postgres for system data, SQLite-per-app for dynamic user data.

This is a teaching project — see `docs/rust-notes.md` for a running glossary of Rust concepts as they appear in the codebase, and `docs/plans/2026-05-05-lovely-rs-design.md` for the full design.

## Status

**Milestone A — Static CMS slice** in progress. See `docs/plans/2026-05-05-milestone-a-static-cms.md`.

What works today:
- Workspace builds clean with `cargo build --workspace`
- 53 unit tests pass, 9 integration tests ready (gated on Docker)
- `lovely-tree` arena-backed Page Element DOM with criterion benchmarks (render_subtree at 55ns)
- Postgres migrations + sqlx repos for users, sessions, oauth_identities, pages, elements
- axum 0.8 router with username/password auth + CSRF + Postgres-backed sessions
- Pages CRUD (list, create, public render, delete) with htmx-aware redirects
- Multi-stage Dockerfile, docker-compose, k8s manifests
- Lora self-hosted, ~200-line CSS with magenta/black accent palette

What's not yet wired up: GitHub/Google/Apple OAuth handlers, TOTP enrollment/verify, the build page (milestone B), per-app SQLite (milestone C).

## Run locally

The fastest path is the `bin/` scripts:

```sh
./bin/pg              # boot or start the dev Postgres container, tail logs
./bin/server          # run lovely-server (auto-reloads if cargo-watch installed)
./bin/test            # run unit tests; auto-reruns on file changes
./bin/test-integration  # run Docker-gated integration suite (requires Docker)
./bin/bench           # criterion benchmarks against the saved baseline
./bin/check           # local CI: fmt + clippy + test
./bin/psql            # psql shell against the dev DB
./bin/pg-stop         # stop and remove the dev Postgres container
```

Or open the **lovely-rs** Warp launch configuration (Command Palette → "Launch Configurations") to spawn 5 tabs (pg, server, test, shell, git) all at once.

### Postgres setup

Local dev uses **Homebrew Postgres** (`postgresql@17`) with a dedicated `lovely_rs` role and database — separate from any pre-existing `lovely` database (e.g. the Swift Vapor app's). `./bin/pg` creates the role/db idempotently on first run.

Manual equivalent (only needed if not using `./bin/pg`):

```sh
brew services start postgresql@17
psql postgres -c "CREATE ROLE lovely_rs LOGIN PASSWORD 'lovely_rs' SUPERUSER"
psql postgres -c "CREATE DATABASE lovely_rs OWNER lovely_rs"
export LOVELY_DATABASE_URL=postgres://lovely_rs:lovely_rs@localhost:5432/lovely_rs
export LOVELY_SESSION_SECRET=$(openssl rand -hex 32)
cargo run -p lovely-server
```

Visit `http://localhost:8080`.

## Test

```sh
cargo test --workspace                      # unit tests + non-Docker integration
cargo test --workspace -- --ignored         # full integration suite (needs Docker)
cargo bench --workspace --features render   # criterion benchmarks
```

## Layout

```
crates/lovely-tree/         arena-backed page element DOM tree + benches
crates/lovely-db/           sqlx pools + SqliteAppStore trait + repos
crates/lovely-web/          axum router, maud views, auth, htmx wiring
crates/lovely-server/       main binary — wires LocalSqliteAppStore (stubbed in v1)
crates/lovely-data/         stub binary for future remote SQLite split
crates/lovely-test-support/ path-only test helpers (TestApp, PgTestContainer)
migrations/                 Postgres SQL migrations
static/                     CSS, JS, Lora woff2
deploy/                     Dockerfile, compose, k8s manifests
docs/plans/                 design + per-milestone implementation plans
docs/rust-notes.md          Rust concepts as they appear, with file:line links
```

## License

MIT or Apache-2.0, at your option.
