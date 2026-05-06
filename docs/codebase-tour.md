# Codebase tour

A guided walk through `lovely-rs` for someone new to Rust who wants to
start contributing. Pairs with:

- `README.md` — quick start, run instructions
- `docs/rust-notes.md` — Rust concept glossary, with file:line links
- `docs/plans/2026-05-05-lovely-rs-design.md` — the full design doc
- `CLAUDE.md` — durable conventions (also useful for humans)

This doc is the wayfinder. Read it once, then keep it open while you
poke around.

---

## 1. What lovely-rs is

A web app for building web apps. Sign up, get an "app" with a default
home page. Drop elements onto pages with a 3-column live editor (tree
sidebar, preview, attributes form). Define collections with typed
fields. Bind page elements to collection records so visitors see real
content. Forms write back to those collections.

We're rebuilding the Swift `lovely` codebase in Rust. Everything is
server-rendered HTML; htmx makes it feel live. No SPA, no React, no
JSON API.

---

## 2. The 30-second mental model

Two databases:

- **Postgres** — *system* data: users, sessions, OAuth identities,
  pages, elements, collections-and-fields metadata, the per-app schema
  intent log. One pool, shared by every request.
- **SQLite** — *per-app user data*: one `.sqlite` file per app, holding
  the actual records the user collects (posts, events, etc.) as real
  typed rows. Schema changes go through an "intent log" stored in
  Postgres so SQLite is fully reconstructible.

Stack:

- **axum 0.8** — HTTP router. Handlers are async functions returning
  `Result<Response, WebError>`.
- **maud** — compile-time HTML templates (it's just Rust syntax that
  builds a `Markup` tree).
- **htmx** — partial-page updates. The server sends HTML fragments;
  htmx swaps them into place.
- **sqlx 0.8** — SQL queries with bound parameters. We use the runtime
  variant (`query_as::<_, T>(...)`) — no `query!` macro yet.
- **slotmap** — arena allocator for the page-element tree. Generational
  keys mean stale handles return `None` instead of dangling.

---

## 3. The crates and how they stack

```
                lovely-server (bin)
                       │
                       ▼
                lovely-web ─────────┐
                       │            │
                       ▼            ▼
                lovely-db ─►  lovely-tree
                       │
                       ▼
              Postgres + SQLite

lovely-test-support → lovely-web (test fixtures)
lovely-data         → (future remote SQLite store; stubbed)
```

| Crate | Role | Worth reading |
|---|---|---|
| `lovely-tree` | Page-element DOM. Arena + generational handles. Tag whitelist. Iterative renderer. | `src/tags.rs`, `src/build.rs`, `src/render.rs` |
| `lovely-db` | Postgres pool + repos (users, pages, elements, collections), the per-app SQLite store, the schema intent log. | `src/pages.rs`, `src/intent.rs`, `src/schema_service.rs` |
| `lovely-web` | HTTP layer. Auth, CSRF, sessions, htmx wiring, maud views. | `src/state.rs`, `src/router.rs`, `src/handlers/*.rs` |
| `lovely-server` | The actual binary. Parses CLI flags, builds `AppState`, calls `axum::serve`. | `src/main.rs` (tiny) |
| `lovely-test-support` | Path-only dev-dep crate. Boots Postgres in a Docker container, spins up `TestApp` on an ephemeral port. | `src/lib.rs` |
| `lovely-data` | Reserved for the future remote SQLite RPC. Currently a stub. | skip |

Each crate has its own `README.md` with a paragraph or two of context.

---

## 4. A request, end to end

Pick this scenario: **"I'm logged in as alice, I'm on the data page,
I add a `title` field to my `posts` collection."** Browser sends:

```
POST /apps/personal/data/posts/fields
Cookie: lovely_session=...; lovely_csrf=abc123
_csrf=abc123&name=title&type=text
```

**(1) Router match** — `crates/lovely-web/src/router.rs:79` mounts:

```rust
.route(
    "/apps/{app_slug}/data/{coll_name}/fields",
    post(handlers::data::post_field_add),
)
```

**(2) Extractors run** —
`crates/lovely-web/src/handlers/data.rs:122` declares:

```rust
pub async fn post_field_add(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((app_slug, coll_name)): Path<(String, String)>,
    jar: CookieJar,
    Form(form): Form<AddFieldForm>,
) -> Result<Response, WebError>
```

axum looks at each parameter type and runs its extractor in order:
- `State` clones the shared app state.
- `AuthUser` reads the session cookie, looks up the session row, then
  the user (`crates/lovely-web/src/auth/extractors.rs`). Anonymous →
  redirect to login.
- `Path<(String, String)>` parses `/apps/{app_slug}/data/{coll_name}/...`.
- `CookieJar` is the cookie collection; we use it to read the CSRF
  cookie.
- `Form<AddFieldForm>` deserializes the URL-encoded body.

**(3) CSRF check** — compare the `_csrf` form field to the cookie via
`crate::auth::csrf::verify_token`. Mismatch → `403`.

**(4) Authorization** — `find_app_by_owner_and_slug` ensures the app
exists *and* belongs to alice. None → `WebError::NotFound`.

**(5) Validation** —
`Identifier::new(&form.name)` (`crates/lovely-db/src/intent.rs:103`).
That's the safety boundary: lowercase ASCII letters, digits,
underscores; not a SQL keyword; ≤ 63 chars. If it returns `Err`, we
respond `422` with the `IDENT_HELP` message.

**(6) Two writes happen, in order:**

a. `set_collection_fields` updates the Postgres `collections.fields_json`
   blob — the legacy "soft schema" still drives the existing list/form
   views.

b. `state.schema.record(app_id, user_id, Intent::AddColumn { ... })`
   inserts a row into `app_schema_migrations`
   (`crates/lovely-db/src/schema_service.rs:58`). This is the intent
   log: the source of truth for the per-app SQLite schema.

**(7) The next time the renderer or SQL console asks for the SQLite
pool**, `LocalSqliteAppStore::get_pool` calls
`SchemaService::ensure_migrated`
(`crates/lovely-db/src/schema_service.rs:111`), which runs any pending
DDL inside `BEGIN IMMEDIATE`. The new column shows up in SQLite.

**(8) Response** — `Redirect::to(/apps/personal/data/posts/edit)`. The
browser follows it, the next page renders the updated collection.

That's the loop. Most other handlers follow the same shape:
extractors → validate → mutate Postgres → mutate SQLite (if relevant)
→ render fragment or redirect.

---

## 5. Suggested reading order

If you want to get oriented in ~30 minutes, read in this order. Each
file has a job. None is huge.

1. **`crates/lovely-server/src/main.rs`** — the binary. Shows you
   exactly what happens at startup: read env vars, connect to Postgres,
   run migrations, build `AppState`, hand it to the router, serve.
2. **`crates/lovely-web/src/state.rs`** — the shared state struct. It's
   tiny; everything in there is a pointer or a string. Get familiar.
3. **`crates/lovely-web/src/router.rs`** — the URL → handler table. If
   you're hunting for "what runs at /foo", start here.
4. **`crates/lovely-web/src/handlers/pages.rs`** — the public-page
   render path. Read `render_public` to see how a page becomes HTML.
5. **`crates/lovely-tree/src/build.rs`** — how a flat list of element
   rows becomes a Tree. Pay attention to `order_siblings` — a
   defensive pass that recovers from corrupt prev_sibling chains.
6. **`crates/lovely-tree/src/render.rs`** — the iterative renderer
   (explicit stack, no recursion). Why iterative? So deeply nested
   user content can't overflow the call stack.
7. **`crates/lovely-db/src/intent.rs`** — `Identifier`, `Intent`,
   `ColumnSpec`. Read the unit tests at the bottom; they show the
   shape of every variant.
8. **`crates/lovely-db/src/schema_service.rs`** — `record` and
   `ensure_migrated`. The header comment explains the concurrency
   model.
9. **`crates/lovely-web/src/handlers/data.rs`** — collections, fields,
   records. The dual-write pattern lives here.

After that, browse what catches your eye.

---

## 6. Where to look when ___

| When you're chasing… | Start here |
|---|---|
| "Why am I getting redirected to /auth/login?" | `crates/lovely-web/src/auth/extractors.rs::AuthUser` |
| "How does the CSRF token actually work?" | `crates/lovely-web/src/auth/csrf.rs` |
| "Where do htmx fragment responses get built?" | `crates/lovely-web/src/handlers/builder.rs` (search for `HX-Trigger`) |
| "How does the page tree get rendered?" | `crates/lovely-tree/src/render.rs` |
| "Where do collection schema changes get recorded?" | `crates/lovely-db/src/schema_service.rs::record` |
| "Where's the per-app SQLite file written?" | `crates/lovely-db/src/sqlite_store.rs::LocalSqliteAppStore` |
| "How does undo/redo work?" | `crates/lovely-db/src/revisions.rs` |
| "How does `{{collection.field}}` interpolation work?" | `crates/lovely-web/src/handlers/pages.rs::interpolate_collection_refs` |
| "Why is there an `order_siblings` salvage pass?" | git log on `crates/lovely-tree/src/build.rs` — the bug it fixes |
| "Where do tests live?" | `crates/lovely-web/tests/*.rs` (HTTP-level), `crates/lovely-db/tests/*.rs` (DB-level), `#[cfg(test)] mod tests` blocks for unit tests |

---

## 7. Rust patterns you'll meet repeatedly

`docs/rust-notes.md` has the deep dives. This is the cheat sheet.

- **`Result<T, E>` and `?`** — every fallible call returns a `Result`,
  and `?` early-returns the error (with auto-conversion via `From`).
  Errors flow up: `sqlx::Error → DbError → WebError`.
- **Newtypes for safety** — `ElementUuid`, `PageUuid`, `Identifier`.
  Wraps a primitive in a one-field struct so the compiler can tell
  them apart. `Identifier` adds *validation*: the only way to
  construct one is via `Identifier::new`, which returns `Result`.
- **`Arc<dyn Trait>` for plug-in services** — `AppState.app_store:
  Arc<dyn SqliteAppStore>` lets us swap the local-disk impl for a
  remote-RPC impl later without touching handler code.
- **Async traits via `#[async_trait]`** — see `SqliteAppStore` in
  `crates/lovely-db/src/sqlite_store.rs`. Stable Rust still needs the
  macro for object-safe async traits.
- **`State<T>`, `Path<T>`, `Form<T>` extractors** — axum reads
  parameter types and runs the matching extractor. Zero glue code.
- **maud `html! { … }`** — looks like a templating language; it's
  actually a procedural macro that expands to `String`-builder
  Rust code at compile time. Type errors and mismatched braces are
  compiler errors, not runtime template errors.
- **`Result` returned from a unit test** — in our integration tests,
  `unwrap()` is fine; assertions panic on failure and the test runner
  reports them. Production code never `unwrap`s on a fallible op
  unless we genuinely know it can't fail (e.g. `Identifier::new("id")`
  where the literal is hand-checked).
- **`tracing::info!`/`warn!`/`error!`** — structured logging. Use
  fields (`tracing::warn!(error = %e, app_id = %id, "msg")`) instead
  of formatting into the message — the log JSON output indexes them.

---

## 8. What's done vs what's next

The full design doc is at `docs/plans/2026-05-05-lovely-rs-design.md`.
Milestones map to roughly:

- **Milestone A — static CMS slice** ✅ shipped.
  Auth (username/password + CSRF + sessions), pages CRUD, public
  rendering, the tree crate with benches, Postgres migrations.
- **Milestone B — live editor** ✅ shipped.
  3-column build page, htmx fragments, undo/redo with snapshots,
  selection-after-add, inline preview (no iframe), inline `#text`
  nodes, `data-lovely-bind` / `data-lovely-source` /
  `data-lovely-repeat` attributes.
- **Milestone C — dynamic data + forms** 🚧 in progress.
  - ✅ `Identifier` validation
  - ✅ `Intent` log + `SchemaService` (record + ensure_migrated)
  - ✅ `LocalSqliteAppStore` (per-app .sqlite files, WAL mode)
  - ✅ Collection / field mutations dual-write into the intent log
  - ✅ Record inserts dual-write into per-app SQLite (best-effort)
  - ✅ Read-only SQL console at `/apps/{slug}/data/console`
  - ✅ Schema-history audit list on the data page
  - ✅ `delete_app` drops the per-app .sqlite
  - 🟡 *renderer still reads records from Postgres* — cutover is the
    next big chunk.
  - 🟡 ElementDataSource / ElementDataDestination as proper inspector
    concepts (currently `data-lovely-bind` / `data-lovely-source` only
    cover the basic cases).
  - 🟡 `lovely-data` remote RPC — design only, not built.

Test inventory at last green run: ~110 integration tests across
`lovely-db` and `lovely-web`, plus the unit suite in each crate's
library and `lovely-tree`'s criterion benches.

---

## 9. Dev loop

The `bin/` scripts cover everything you'll need day-to-day. The Warp
launch config opens five tabs at once (pg, server, test, shell, git).

```sh
./bin/pg                # boot dev Postgres in Docker
./bin/server            # cargo run lovely-server (auto-reloads with cargo-watch)
./bin/test              # unit tests, watch mode
./bin/test-integration  # full Docker-gated suite (slow)
./bin/check             # fmt + clippy + test
./bin/psql              # psql shell against the dev DB
./bin/bench             # criterion benches against the saved baseline
./bin/pg-stop           # tear down the dev Postgres container
```

Manual paths:

```sh
cargo build --workspace                            # whole workspace
cargo test --workspace                             # unit + non-Docker integration
cargo test --workspace -- --ignored                # full suite (needs Docker)
cargo test -p lovely-web --test sqlite_mirror -- --ignored
cargo clippy --workspace --all-targets             # lint
cargo fmt                                          # format
cargo bench -p lovely-tree --features render       # tree benches
```

A failing integration test prints `RUST_BACKTRACE=1` for tracebacks.
For more chatty server logs:

```sh
LOVELY_LOG_LEVEL=debug ./bin/server
```

---

## 10. If you only remember three things

1. **The renderer is iterative**, not recursive. Look at how
   `lovely-tree::render` builds a `Vec` stack. This is why deep user
   content can't crash the server.
2. **Identifier is the only safe way to build SQL DDL.** If you ever
   `format!("CREATE TABLE {name} ...")` from a `&str`, you've broken
   the model. Take an `Identifier` instead.
3. **Postgres is the source of truth.** SQLite is derived state. If
   the SQLite file is missing or stale, opening it replays the intent
   log. You can `rm -rf data/apps/*.sqlite` and the server will heal
   on next request.
