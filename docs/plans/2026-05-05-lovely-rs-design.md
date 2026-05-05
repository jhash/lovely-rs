# lovely-rs — Design Document

**Date:** 2026-05-05
**Author:** Jake (with Claude)
**Status:** Validated, ready for milestone-A implementation plan

---

## 1. Goals, non-goals, and roadmap

**Goal.** Rebuild the Swift "lovely" dynamic site builder in Rust as a teaching project, fixing the perf and state-loss problems that grew up in the Swift version. Server-rendered HTML with htmx for interactivity, no SPA. Postgres for internal/system data, one SQLite file per user-created app for dynamic user data.

**Non-goals (v1).** No SPA framework. No client-side rendering of the page tree. No Tailwind. No multi-replica deploy. No remote SQLite (libsql/Turso). No payments, jobs, marketing, or AI surfaces from lovely's sidebar — those come back later only if they earn their keep.

**Three milestones, ship A before starting B:**

- **Milestone A — "Static CMS slice"** (~2–3 weeks of learner-paced work). Auth (GitHub + Google + Apple OAuth + username/password + TOTP), pages with slugs, page elements rendered server-side from Postgres, full test pyramid, criterion benches on the tree, Docker + k8s manifests, deployable to the Linux server. Goal: prove the foundation — maud + axum + sqlx + sessions + migrations + tests + deploy — *without* the live editor.
- **Milestone B — "Live page tree editor"** (~3–4 weeks). The lovely 3-column build page: tree sidebar (htmx targeted swaps, no full-tree rebuild), preview, attributes form. Add `fantoccini` browser tests for the tree. Per-user `user_ui_state` for last-selected and last-open elements. No dynamic user data tables yet.
- **Milestone C — "Dynamic data + forms"** (~4–6 weeks). Per-app SQLite provisioning, runtime DDL via the intent log, `ElementDataSource` and `ElementDataDestination`, dynamic forms reading and writing user data. The full lovely feature surface.

A separate implementation plan is written per milestone (not all at once). Each plan reaches a deployable state before the next begins.

---

## 2. Workspace layout and crate boundaries

```
lovely-rs/
├── Cargo.toml                  # workspace
├── rust-toolchain.toml         # pin to stable (e.g. 1.83)
├── Cargo.lock
├── .env.example
├── docs/
│   └── plans/
│       └── 2026-05-05-lovely-rs-design.md
├── deploy/
│   ├── Dockerfile              # multi-stage, distroless runtime
│   ├── compose.yaml            # local dev / Swarm
│   └── k8s/                    # Deployment, Service, Ingress, PVCs
├── migrations/                 # Postgres migrations (sqlx)
├── static/
│   ├── style.css               # bundled, ~200 lines, CSS custom props
│   ├── tree.js                 # ~50 lines, accordion state + selection
│   └── fonts/                  # Lora woff2 self-hosted
└── crates/
    ├── lovely-tree/            # arena-backed Page Element DOM tree
    │   ├── src/{lib,node,arena,render,errors}.rs
    │   ├── tests/              # integration tests
    │   └── benches/            # criterion benchmarks
    ├── lovely-db/              # sqlx pools + SqliteAppStore trait + schema svc
    │   ├── src/{lib,postgres,sqlite,store,migrate,errors}.rs
    │   └── tests/              # against testcontainers Postgres + temp SQLite
    ├── lovely-web/             # axum router, maud views, auth, htmx wiring
    │   ├── src/{lib,router,views/,handlers/,auth/,errors}.rs
    │   └── tests/              # HTTP-level e2e via reqwest + scraper
    ├── lovely-server/          # main binary — wires LocalSqliteAppStore
    │   └── src/main.rs
    ├── lovely-data/            # stub binary for future remote SQLite split
    │   └── src/main.rs
    └── lovely-test-support/    # path-only dev-dep helpers (TestApp, seeders)
        └── src/lib.rs
```

**Boundary rules** (enforced via `pub` visibility and code review):

- `lovely-web` does not import `sqlx` directly — it only sees `lovely-db`'s public API.
- `lovely-web` does not touch the filesystem — SQLite-per-app goes through `lovely_db::SqliteAppStore`.
- `lovely-tree` has zero dependencies on `tokio`, `sqlx`, `axum`, or `maud`. Pure data plus `Render` impls behind a `render` feature flag (pulls in `maud` only when enabled). Lets `cargo bench -p lovely-tree` compile in seconds.
- `lovely-server` is the only crate that names concrete impls (`LocalSqliteAppStore`, `PostgresSessionStore`). Everything else is generic over traits.

---

## 3. `lovely-tree` data structure (the perf-critical core)

The Page Element tree is the heart of the app and the place lovely got slow. The design here is built around two facts: the tree mutates a lot in the builder, and every render does a lot of "find by element-uuid" and "walk ancestors."

**Core types:**

```rust
slotmap::new_key_type! { pub struct NodeId; }

pub struct Tree {
    nodes: SlotMap<NodeId, Node>,
    root: NodeId,
    by_uuid: HashMap<ElementUuid, NodeId>,   // O(1) "find by DB id"
}

pub struct Node {
    uuid: ElementUuid,
    tag: ElementTag,
    payload: ElementPayload,
    attrs: AttrList,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    last_child: Option<NodeId>,
    prev_sibling: Option<NodeId>,
    next_sibling: Option<NodeId>,
}
```

Why these fields:

- **Doubly-linked siblings + first/last child pointers** = O(1) insert-before, insert-after, append, remove. lovely's Swift version stored an `order: Int` and re-indexed on every move — O(n) per move and ugly update churn.
- **`by_uuid` side-table** = O(1) "load attributes for element X" instead of lovely's recursive `findElement` walking the whole tree on every htmx request.
- **`SlotMap` (generational keys)** — freed slots can be reused, but a stale `NodeId` is *detected* (returns `None`) instead of pointing at a recycled node.

**Public API (stable):**

```rust
impl Tree {
    pub fn new(root_uuid: ElementUuid) -> Self;
    pub fn from_db_rows(rows: &[ElementRow]) -> Result<Self, TreeError>;

    pub fn get(&self, id: NodeId) -> Option<&Node>;
    pub fn get_by_uuid(&self, uuid: ElementUuid) -> Option<NodeId>;

    pub fn append_child(&mut self, parent: NodeId, node: NewNode) -> Result<NodeId, TreeError>;
    pub fn insert_before(&mut self, sibling: NodeId, node: NewNode) -> Result<NodeId, TreeError>;
    pub fn insert_after(&mut self, sibling: NodeId, node: NewNode) -> Result<NodeId, TreeError>;
    pub fn remove(&mut self, id: NodeId) -> Result<RemovedSubtree, TreeError>;
    pub fn move_to(&mut self, id: NodeId, new_parent: NodeId, position: Position) -> Result<(), TreeError>;
    pub fn update(&mut self, id: NodeId, patch: NodePatch) -> Result<(), TreeError>;

    pub fn children(&self, parent: NodeId) -> ChildrenIter<'_>;
    pub fn ancestors(&self, id: NodeId) -> AncestorsIter<'_>;
    pub fn descendants(&self, root: NodeId) -> DescendantsIter<'_>;
}
```

**Render** (behind `render` feature): `Tree::render(&self) -> maud::Markup` walks descendants iteratively (no recursion, no stack overflow on deep trees). `Tree::render_subtree(&self, root: NodeId) -> maud::Markup` is the htmx-targeted-swap primitive.

**Criterion benchmarks shipped on day 1:**

- `bench_build_from_rows_1k`, `bench_build_from_rows_10k`
- `bench_find_by_uuid` (hot path)
- `bench_insert_at_random_position_in_1k_tree`
- `bench_remove_random_subtree_in_1k_tree`
- `bench_render_full_1k_tree`
- `bench_render_subtree_depth_10` (htmx swap path — most important)

Baselines saved on first green CI; `cargo bench -- --baseline ci` fails on >10% regression.

**Error model:**

```rust
#[derive(thiserror::Error, Debug)]
pub enum TreeError {
    #[error("node {0:?} not found")]
    NotFound(NodeId),
    #[error("uuid {0} not in tree")]
    UnknownUuid(ElementUuid),
    #[error("would create cycle: moving {child:?} into {ancestor:?}")]
    WouldCycle { child: NodeId, ancestor: NodeId },
    #[error("invalid attribute name: {0:?}")]
    InvalidAttribute(String),
}
```

---

## 4. `lovely-db`: schema, sqlx pools, and the `SqliteAppStore` boundary

**Postgres schema (milestone A):**

```sql
-- 20260505000001_users.up.sql
CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username        TEXT NOT NULL UNIQUE,
    email           TEXT UNIQUE,
    password_hash   TEXT,
    totp_secret     TEXT,
    role            TEXT NOT NULL DEFAULT 'user',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 20260505000002_oauth_identities.up.sql
CREATE TABLE oauth_identities (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id            UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider           TEXT NOT NULL,
    provider_user_id   TEXT NOT NULL,
    raw_profile        JSONB NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(provider, provider_user_id)
);

-- 20260505000003_sessions.up.sql
CREATE TABLE sessions (
    id           TEXT PRIMARY KEY,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    csrf_token   TEXT NOT NULL,
    expires_at   TIMESTAMPTZ NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    user_agent   TEXT,
    ip           INET
);
CREATE INDEX sessions_user_id_idx ON sessions(user_id);
CREATE INDEX sessions_expires_at_idx ON sessions(expires_at);

-- 20260505000004_pages.up.sql
CREATE TABLE pages (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug         TEXT NOT NULL UNIQUE,
    title        TEXT NOT NULL,
    description  TEXT,
    root_element UUID,
    author_id    UUID NOT NULL REFERENCES users(id),
    published_at TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 20260505000005_elements.up.sql
CREATE TABLE elements (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    page_id      UUID NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    parent_id    UUID REFERENCES elements(id) ON DELETE CASCADE,
    prev_sibling UUID REFERENCES elements(id) ON DELETE SET NULL,
    tag          TEXT NOT NULL,
    attrs        JSONB NOT NULL DEFAULT '{}'::jsonb,
    payload      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX elements_page_id_idx ON elements(page_id);
CREATE INDEX elements_parent_id_idx ON elements(parent_id);
ALTER TABLE pages ADD CONSTRAINT pages_root_element_fk
    FOREIGN KEY (root_element) REFERENCES elements(id) ON DELETE SET NULL;
```

The `prev_sibling` column on `elements` matches the linked-list ordering in `lovely-tree`, so DB rows load straight into the in-memory structure without an O(n²) order-column re-sort.

**Milestone B/C tables added later:** `apps`, `app_members`, `app_schema_migrations`, `user_ui_state`. Not in v1 migrations.

**The `SqliteAppStore` trait — the boundary that keeps web/data split-friendly:**

```rust
#[async_trait]
pub trait SqliteAppStore: Send + Sync + 'static {
    async fn get_pool(&self, app_id: AppId) -> Result<sqlx::SqlitePool, DbError>;
    async fn ensure_migrated(&self, app_id: AppId) -> Result<(), DbError>;
    async fn close_pool(&self, app_id: AppId) -> Result<(), DbError>;
    async fn delete_app(&self, app_id: AppId) -> Result<(), DbError>;
}

pub struct LocalSqliteAppStore {
    data_dir: PathBuf,
    pools: DashMap<AppId, sqlx::SqlitePool>,
    schema: Arc<SchemaService>,
    locks: DashMap<AppId, Arc<tokio::Mutex<()>>>,
    version_cache: DashMap<AppId, AtomicI64>,
    idle_timeout: Duration,
}
```

`get_pool` returns from the `DashMap` if cached, otherwise opens with `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;` and runs `ensure_migrated` once. A background task scans pools every 60s and closes any whose last-use timestamp is older than `idle_timeout`.

**Postgres pool:** single `PgPool` with `max_connections = 16` by default, configurable. Constructed in `lovely-server::main` and passed to `lovely-web` as state.

**Error type:**

```rust
#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error(transparent)]                 Sqlx(#[from] sqlx::Error),
    #[error(transparent)]                 Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("app {0} not found")]         AppNotFound(AppId),
    #[error("invalid identifier: {0:?}")] InvalidIdentifier(String),
    #[error("schema conflict: {0}")]      SchemaConflict(String),
    #[error(transparent)]                 Io(#[from] std::io::Error),
}
```

---

## 5. `lovely-web`: routing, auth, sessions, htmx wiring

**Router shape (axum, milestone A surface):**

```
GET    /                              → home / page index
GET    /healthz                       → liveness
GET    /readyz                        → readiness

GET    /auth/login                    → login form
POST   /auth/login                    → username/password submit (htmx-aware)
GET    /auth/register                 → register form
POST   /auth/register                 → register submit
POST   /auth/logout

GET    /auth/github                   → start GitHub OAuth
GET    /auth/github/callback          → finish GitHub OAuth
GET    /auth/google                   → start Google OAuth
GET    /auth/google/callback          → finish Google OAuth
GET    /auth/apple                    → start Apple OAuth (signed-JWT secret)
POST   /auth/apple/callback           → Apple posts the callback (note: POST)

GET    /auth/totp/enroll              → QR + secret
POST   /auth/totp/enroll              → confirm TOTP code
POST   /auth/totp/verify              → verify on login challenge

GET    /pages                         → list (auth required)
GET    /pages/new                     → form
POST   /pages                         → create
GET    /pages/:slug                   → public render
GET    /pages/:slug/edit              → milestone B
POST   /pages/:slug                   → update metadata
DELETE /pages/:slug

GET    /static/*path                  → tower-http ServeDir
```

Anything under `/pages*` (except `GET /pages/:slug` for published pages) requires an authenticated session. Handlers can opt out by accepting `Option<AuthUser>`.

**Middleware stack (outermost → innermost):**

1. `tower_http::trace::TraceLayer` — request span with `request_id`, `method`, `uri`, htmx headers.
2. `tower_http::compression::CompressionLayer` — gzip/brotli for HTML/CSS/JS.
3. Custom `RequestIdLayer` — UUID per request, attached to span and response header.
4. `tower_http::timeout::TimeoutLayer` — 30s default.
5. `SessionLayer` (custom, on `tower-sessions-sqlx-store::PostgresStore`) — reads the `lovely_session` cookie, loads session row, attaches `SessionContext` to extensions.
6. `CsrfLayer` — double-submit cookie pattern; verifies `X-CSRF-Token` on non-GET.
7. `AuthExtractorLayer` — populates `Option<AuthUser>` from `SessionContext`.

**Custom axum extractors:**

- `AuthUser` — required-auth. Rejects with 401 + `HX-Redirect: /auth/login` for htmx, 302 otherwise.
- `Option<AuthUser>` — optional auth.
- `SuperAdmin` — requires `users.role = 'superadmin'`.
- `HtmxRequest` — typed wrapper around htmx headers (re-exported from `axum-htmx`).
- `Csrf` — extracts/validates CSRF token.

**Auth flow specifics:**

- **OAuth (GitHub/Google):** `oauth2` crate, PKCE enabled, state token in a short-lived signed cookie. Callback exchanges code → token → fetches profile → upsert into `oauth_identities` → look up or create `users` row → create session.
- **Apple Sign In:** OAuth2 with two extras — (1) the `client_secret` is a JWT we sign with the `.p8` private key on each token request (cached for ~5 min, regenerated before expiry); (2) the callback is `POST` if `response_mode=form_post` is requested (it will be).
- **Username/password:** `argon2` with crate defaults (memory: 19MiB, iterations: 2). On login, if `totp_secret IS NOT NULL`, redirect to `/auth/totp/verify` with a short-lived "pending" cookie.
- **TOTP:** `totp-rs` for verification, `qrcode` for the enrollment PNG, rendered inline as `data:image/png;base64,...` in the maud view.
- **`AuthProvider` trait** wraps all four; `MockAuthProvider` for tests.

**htmx integration patterns:**

- Form submits return either a fragment or a full page based on `HX-Request`.
- Error responses in htmx return a `<div class="error">…</div>` fragment with status 422 and `HX-Reswap: innerHTML`.
- Auth-required redirects: 401 + `HX-Redirect: /auth/login` for htmx; 302 + `Location` for plain.
- `HX-Trigger` typed enum for cross-fragment events (`lovely:element-deleted`, `lovely:tree-changed`, etc.).

---

## 6. maud rendering, the `define_tags!` macro, and the build-page DSL

**The `define_tags!` macro** (single source of truth for "what tags exist," in `lovely-tree/src/tags.rs`):

```rust
define_tags! {
    div, section, article, header, footer, nav, main, aside,
    h1, h2, h3, h4, h5, h6, p, span, strong, em, blockquote, code, pre,
    a, ul, ol, li,
    img, figure, figcaption,
    table, thead, tbody, tr, th, td,
    form { method: FormMethod, action: String, enctype: Enctype },
    input { input_type: InputType, name: String, value: Option<String>, required: bool },
    textarea { name: String, value: String, rows: u16 },
    select { name: String, options: Vec<SelectOption>, value: Option<String> },
    button { button_type: ButtonType, label: String },
    label { for_id: Option<String>, text: String },
    hr, br,
}
```

The macro expands to: `ElementTag` enum, `ElementPayload` enum, `name()`/`from_str()` methods, and the open/close tag render dispatch.

**Why a macro and not a build.rs:** macro is in one file you can read top-to-bottom, `cargo expand` shows the output, no build-time codegen step. Promote to a proc-macro in `lovely-tree-derive` only if `macro_rules!` runs out of expressivity.

**Attribute handling:**

```rust
pub struct AttrList { entries: Vec<(AttrName, String)> }

#[derive(Clone, PartialEq, Eq)]
pub struct AttrName(SmolStr);

impl AttrName {
    pub fn new(s: &str) -> Result<Self, TreeError> { /* validates */ }
}
```

Validator regex: `^[a-zA-Z][a-zA-Z0-9-]*$`. Denylist `hx-*` and `on*` in user-provided attributes — those only come from typed payloads. Values stored unescaped, escaped at render time via maud.

**maud `Render` impl:**

```rust
#[cfg(feature = "render")]
impl maud::Render for Tree {
    fn render_to(&self, out: &mut String) {
        render_subtree_iterative(self, self.root, out);
    }
}
```

The walker is iterative (`Vec<TraversalState>` stack) — crucial because user-built trees can be hundreds deep, and we will not blow the stack.

**Build-page DSL (`lovely-web/src/views/build.rs`):**

The 3-column build page is plain maud (fixed structure):

```rust
pub fn build_page(ctx: &BuildPageCtx) -> Markup {
    html! {
        (page_shell(&ctx.shell, html! {
            div #build-grid {
                (sidebar_tree(ctx))
                (preview_pane(ctx))
                (attributes_panel(ctx))
            }
        }))
    }
}
```

`sidebar_tree` walks the same `Tree` but emits a structural outline — each node becomes:

```html
<details id="tree-{uuid}" class="tree-node" data-uuid="{uuid}" {open_if_in_state}>
  <summary class="tree-summary {selected_if_match}">
    <span class="tree-tag">div</span>
    <span class="tree-label">{computed_label_or_empty}</span>
  </summary>
  <div class="tree-children" id="tree-children-{uuid}">
    {recurse}
  </div>
</details>
```

**Targeted htmx swaps for mutations:**

- **Add child:** `POST /elements` returns the new `<details>` block with `hx-swap-oob="beforeend:#tree-children-{parent}"`.
- **Insert before/after sibling:** `hx-swap-oob="beforebegin:#tree-{sibling}"` or `afterend:`.
- **Edit element (no structural change):** `PATCH /elements/{id}` returns just the affected `<details>` summary or content; `hx-swap-oob="outerHTML:#tree-summary-{id}"` and `outerHTML:#preview-{id}`.
- **Delete:** `DELETE /elements/{id}` returns empty body with `HX-Trigger: lovely:element-deleted`; `tree.js` listener removes `#tree-{id}` and `#preview-{id}`.

**`tree.js` (~50 lines total scope):**

1. On `<details>` toggle: write `open|closed` to `localStorage["lovely:open:{app_id}:{element_id}"]`.
2. On page load: restore `open` attributes from `localStorage`.
3. On `click .tree-summary`: toggle `.selected` class, fire `htmx.ajax('GET', '/elements/{id}/attrs', '#attributes-panel')`.
4. Listen for `lovely:element-deleted` and `lovely:element-moved` to clean up DOM.
5. CSRF: read `csrf_token` cookie, set `htmx.config.headers["X-CSRF-Token"]`.

Vanilla JS, single `<script src="/static/tree.js" defer>`.

---

## 7. Per-app SQLite, schema service, intent log (designed now, ships in C)

**The intent log (Postgres, single source of truth):**

```sql
CREATE TABLE app_schema_migrations (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_id       UUID NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    version      BIGINT NOT NULL,
    intent       JSONB NOT NULL,
    forward_sql  TEXT NOT NULL,
    reverse_sql  TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by   UUID NOT NULL REFERENCES users(id),
    UNIQUE(app_id, version)
);
CREATE INDEX app_schema_migrations_app_id_version_idx
    ON app_schema_migrations(app_id, version);
```

**The `Intent` type:**

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Intent {
    CreateTable { name: Identifier, columns: Vec<ColumnSpec> },
    DropTable   { name: Identifier },
    AddColumn   { table: Identifier, column: ColumnSpec },
    DropColumn  { table: Identifier, column: Identifier },
    RenameColumn { table: Identifier, from: Identifier, to: Identifier },
    AddIndex    { table: Identifier, name: Identifier, columns: Vec<Identifier>, unique: bool },
    DropIndex   { name: Identifier },
}

pub struct Identifier(String);   // ^[a-z_][a-z0-9_]{0,62}$ + reserved-word denylist

pub struct ColumnSpec {
    pub name: Identifier,
    pub kind: ColumnKind,
    pub nullable: bool,
    pub default: Option<DefaultValue>,
}

pub enum ColumnKind { Text, Integer, Real, Boolean, Blob, Datetime, Json, Uuid }
```

`Identifier::new` is the *only* way to construct one — strict regex, length cap, reserved-word denylist. The DDL renderer never quotes user input as SQL; only `Identifier` values appear in DDL strings.

**The `SchemaService`:**

```rust
pub struct SchemaService {
    pg: sqlx::PgPool,
    version_cache: DashMap<AppId, AtomicI64>,
    locks: DashMap<AppId, Arc<tokio::Mutex<()>>>,
}

impl SchemaService {
    pub async fn record(&self, app_id: AppId, user: UserId, intent: Intent)
        -> Result<MigrationVersion, DbError>;

    pub async fn ensure_migrated(&self, app_id: AppId, sqlite: &sqlx::SqlitePool)
        -> Result<(), DbError>;
}
```

**`ensure_migrated` algorithm:**

1. Acquire per-app `tokio::Mutex` (concurrent calls for the same app wait; different apps proceed in parallel).
2. Read `applied` from `version_cache`. If absent, `SELECT applied_version FROM _lovely_schema_version` once and seed cache.
3. `SELECT version, forward_sql FROM app_schema_migrations WHERE app_id = $1 AND version > $2 ORDER BY version`.
4. If empty, release lock, return Ok.
5. For each pending: `BEGIN IMMEDIATE` → exec `forward_sql` → `UPDATE _lovely_schema_version SET applied_version = ?` → `COMMIT`.
6. Update `version_cache` atomic.
7. Release lock.

**`record` algorithm:**

1. Validate `intent`.
2. Render `forward_sql` and `reverse_sql` from `intent` (pure function).
3. Postgres `BEGIN`.
4. `SELECT COALESCE(MAX(version), 0) + 1` (with `FOR UPDATE` on the `apps` row).
5. `INSERT INTO app_schema_migrations`.
6. `COMMIT`.
7. Bump `version_cache` atomic.

**Atomicity properties:**

- Postgres-side intent recording is atomic per row.
- SQLite-side application is atomic per migration (`BEGIN IMMEDIATE`/`COMMIT`).
- No double-apply (in-process `tokio::Mutex` + version-pointer check).
- Process crash mid-apply: next `ensure_migrated` resumes from version pointer.
- Crash after Postgres insert but before SQLite apply: fine — next open applies it.

**Best-effort reverse:** `AddColumn` → `ALTER TABLE ... DROP COLUMN`; `DropColumn` → `NULL`; `CreateTable` → `DROP TABLE`; `DropTable` → `NULL`; rename swaps; indexes always reversible.

**Read/write of user data rows** doesn't go through `SchemaService` — handlers call `app_store.get_pool(app_id).await?` then run parameterized `sqlx::query`.

---

## 8. Testing pyramid, benchmarks, and CI

**1. Unit tests** — colocated `#[cfg(test)] mod tests` per module.

- `lovely-tree`: every public method has happy path + at least one edge case. Required: cycle detection on `move_to`, `NodeId` validity after `remove`, `by_uuid` invariant after every mutation (`debug_assert_invariants()` runs in debug builds).
- `lovely-db`: pure functions get unit tests (Identifier validator, DDL renderer). DB-touching → integration tests.
- `lovely-web`: pure helpers (htmx response builders, CSRF, error mapping).

**2. Integration tests** — `tests/` per crate.

- `lovely-db/tests/`: `testcontainers-rs` boots Postgres per test *binary*; per-test isolation via random `search_path`. SQLite via `tempfile::TempDir`.
- `lovely-web/tests/`: full `Router` + `tower::ServiceExt::oneshot` for fast in-process HTTP, `reqwest` against bound port for real wire tests.

**3. HTTP-level e2e** — `lovely-web/tests/e2e/*.rs` (milestone A primary).

Boots `lovely-server` on ephemeral port, drives via `reqwest::Client` (cookie jar), parses HTML with `scraper`. Required milestone A flows:

- register → log in → log out → log in
- enroll TOTP → log in → verify TOTP
- create page → render → edit metadata → delete
- OAuth flow against `MockOAuthProvider`
- CSRF rejection (POST without token → 403)
- Auth required (anonymous → 302 / 303 + `HX-Redirect`)

**4. Browser e2e via `fantoccini`** — milestone B.

Drives Chromedriver. Required scenarios:

- Open build page, expand sidebar, navigate to a leaf, verify accordion state survives htmx mutation.
- Add a child, verify only the targeted `<details>` block changed (`MutationObserver` snapshot).
- Move a node, selection follows.
- Refresh, open state restored from `user_ui_state`.

**5. Criterion benchmarks** — `lovely-tree/benches/`, day 1.

Listed in §3. CI on `main` runs `cargo bench -- --save-baseline ci`; PRs run `cargo bench -- --baseline ci`. >10% regression fails.

**Test data / fixtures:** `lovely-test-support` workspace dev-dep crate with `TestApp`, `seed_user`, `seed_page`, `MockOAuthProvider::deterministic`, `AuthedClient` helpers.

**CI shape (GitHub Actions):** parallel jobs — `fmt`, `clippy --all-targets -D warnings`, `test --workspace --all-targets`, `bench` (PRs to `main`), `docker` (build + scan).

**Pre-commit hook (optional):** `cargo fmt`, `cargo clippy --all-targets`, `cargo test --workspace`. Available via `make hooks`.

---

## 9. Deployment, ops, and the dev loop

**Multi-stage Dockerfile** (`deploy/Dockerfile`):

```dockerfile
FROM rust:1.83-slim AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked
COPY rust-toolchain.toml ./
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin lovely-server
COPY . .
RUN cargo build --release --bin lovely-server

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /build/target/release/lovely-server /usr/local/bin/lovely-server
COPY static/ /opt/lovely/static/
COPY migrations/ /opt/lovely/migrations/
ENV LOVELY_STATIC_DIR=/opt/lovely/static
ENV LOVELY_SQLITE_DATA_DIR=/data/apps
USER nonroot:nonroot
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/lovely-server"]
```

`cargo-chef` for fast incremental Docker builds. `distroless/cc` runtime (~22MB, no shell, no package manager). `nonroot` user.

**Local dev loop:**

```
docker compose up -d postgres
cp .env.example .env
cargo run -p lovely-server   # or: cargo watch -x 'run -p lovely-server'
# Second terminal:
cargo watch -x 'test --workspace'
```

**Compose file** (`deploy/compose.yaml`, used for local + Swarm):

```yaml
services:
  lovely-server:
    image: lovely-rs:latest
    build: { context: .., dockerfile: deploy/Dockerfile }
    ports: ["8080:8080"]
    environment:
      LOVELY_DATABASE_URL: postgres://lovely:${LOVELY_DB_PASSWORD}@postgres:5432/lovely
      LOVELY_BASE_URL: ${LOVELY_BASE_URL:-http://localhost:8080}
    secrets: [session_secret, github_client_secret, google_client_secret, apple_private_key]
    volumes: ["app-data:/data"]
    depends_on: { postgres: { condition: service_healthy } }
    deploy:
      replicas: 1
      restart_policy: { condition: on-failure }
  postgres:
    image: postgres:17
    environment:
      POSTGRES_USER: lovely
      POSTGRES_PASSWORD: ${LOVELY_DB_PASSWORD}
      POSTGRES_DB: lovely
    volumes: ["pg-data:/var/lib/postgresql/data"]
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U lovely"]
      interval: 5s
      timeout: 5s
      retries: 5
volumes: { app-data: {}, pg-data: {} }
secrets:
  session_secret:        { external: true }
  github_client_secret:  { external: true }
  google_client_secret:  { external: true }
  apple_private_key:     { external: true }
```

**Kubernetes manifests** (`deploy/k8s/`): `Deployment` (replicas: 1, strategy: Recreate — important for local SQLite), `Service` (ClusterIP), `Ingress` (TLS via cert-manager), `PersistentVolumeClaim` (RWO, 20Gi default for `/data`), `StatefulSet` for Postgres (or external managed), `Secret` for session/OAuth/Apple key.

**Postgres version:** 17 in v1. sqlx supports 11–17; bumping to 18 (when GA) is a one-line image tag change after smoke-testing. Major upgrades require `pg_upgrade` or dump/restore — documented in `deploy/README.md`.

**Health endpoints:**

- `/healthz` — 200 if process responds (no DB).
- `/readyz` — 200 only if Postgres pool can `SELECT 1`, sqlx migrations applied, schema-version cache initialized.

**Graceful shutdown:** `axum::serve(...).with_graceful_shutdown(shutdown_signal())` listens for `SIGTERM`, drains in-flight with 25s timeout, closes Postgres pool, walks per-app `DashMap` and `pool.close().await` each SQLite pool (flushes WAL), exits 0.

**Backup story (documented, not implemented in v1):**

- Postgres: `pg_dump` cron in sidecar.
- SQLite: nightly `.backup` per file, or `litestream` (preferred path).

**Future split into web + data binaries:** Implement `RemoteSqliteAppStore` in `lovely-data`, expose RPC. Web binary unchanged because it sees only `dyn SqliteAppStore`. Same Docker image reused as either binary via `ENTRYPOINT` override.

---

## 10. Documentation, teaching commentary, and the Rust learning path

**(a) In-code comments.** Default to none. Only added when:

- Choice would surprise a Rust-fluent reader (e.g. "we use `Arc<Mutex<T>>` here instead of `RwLock` because writes are more frequent than reads"), written *only* in the file where it lands.
- A workaround for a third-party bug or an external constraint (link to issue).
- A safety invariant the borrow checker can't enforce (rare, mostly inside `unsafe`).

No file-header banners, no "// the following function does X" comments, no `// TODO` without name+date, no `///` on private items.

**(b) Public API docstrings (`///`).** Yes on every `pub` item in `lovely-tree` and `lovely-db`. One-line summary for trivial items, one paragraph + one example for non-trivial. `cargo doc --workspace --open` should produce useful docs. `lovely-web`/`lovely-server` get docs only on public `lib.rs` re-exports.

**(c) Teaching commentary — separate from code.** When a new Rust concept appears, explain in conversation, not in code comments. A single `docs/rust-notes.md` index: each entry is "concept — ELI5 paragraph — link to the file/line where we first used it."

Concepts I'll proactively flag the first time they appear:

1. Ownership & moves — `Tree::new` (A, Step 2)
2. Borrowing (`&`/`&mut`) — `Tree::get` vs `append_child` (A, Step 2)
3. Generational keys / `slotmap` — why `NodeId` is safe (A, Step 2)
4. Iterators & lazy evaluation — `ChildrenIter`, `AncestorsIter` (A, Step 3)
5. `Result` / `?` / `From` conversions — first error type (A, Step 2)
6. `thiserror` vs `anyhow` — at the `lovely-db`/`lovely-server` boundary (A, Step 5)
7. Async + `tokio` — first sqlx call (A, Step 5)
8. `Send`/`Sync`/`'static` bounds — at `SqliteAppStore: Send + Sync + 'static` (A, Step 6)
9. Trait objects (`dyn`) vs generics — `Arc<dyn SqliteAppStore>` choice (A, Step 6)
10. Axum extractors and `FromRequestParts` — `AuthUser` (A, Step 8)
11. Macros (`macro_rules!`) — `define_tags!` (A or B)
12. Lifetimes — first non-elidable case, likely iterator returns (A, Step 3)
13. Smart pointers (`Arc`, `Rc`, `Box`, `Cow`) — as they appear naturally
14. Pattern matching exhaustiveness — at `define_tags!` render dispatch

**(d) READMEs.** Root `README.md` (what is this, how to run, link to design). Per-crate (one paragraph: what's in here, what depends on it). `deploy/README.md` covers Postgres upgrade gotcha, secrets layout, k8s vs Compose differences. No marketing docs. No architecture diagrams in v1.

**(e) Plan documents.** Each milestone gets its own implementation plan via `superpowers:writing-plans` *before* writing code. Plans live in `docs/plans/YYYY-MM-DD-milestone-X-plan.md`.

---

## 11. Concrete crate dependencies and version pins

Workspace-level (versions latest stable as of 2026-05-05):

```toml
[workspace]
resolver = "2"
members = ["crates/lovely-tree", "crates/lovely-db", "crates/lovely-web",
           "crates/lovely-server", "crates/lovely-data", "crates/lovely-test-support"]

[workspace.package]
edition = "2021"
rust-version = "1.83"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
futures = "0.3"

axum = { version = "0.8", features = ["macros", "tracing"] }
axum-extra = { version = "0.10", features = ["cookie", "cookie-signed", "typed-header", "form"] }
axum-htmx = "0.7"
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "compression-gzip", "compression-br",
                                              "fs", "timeout", "request-id"] }
hyper = "1"

maud = "0.27"

sqlx = { version = "0.8", default-features = false, features = [
    "runtime-tokio-rustls", "postgres", "sqlite", "uuid", "chrono",
    "json", "macros", "migrate"] }

oauth2 = "5"
argon2 = "0.5"
totp-rs = { version = "5", features = ["qr"] }
qrcode = "0.14"
jsonwebtoken = "9"
secrecy = { version = "0.10", features = ["serde"] }
rand = "0.8"

tower-sessions = "0.13"
tower-sessions-sqlx-store = { version = "0.14", features = ["postgres"] }

slotmap = "1"
smol_str = "0.3"

dashmap = "6"
moka = { version = "0.12", features = ["future"] }

serde = { version = "1", features = ["derive"] }
serde_json = "1"

uuid = { version = "1", features = ["v4", "v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
clap = { version = "4", features = ["derive", "env"] }
dotenvy = "0.15"

reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json", "cookies"] }

testcontainers = "0.23"
testcontainers-modules = { version = "0.11", features = ["postgres"] }
scraper = "0.21"
fantoccini = "0.21"
tempfile = "3"
criterion = { version = "0.5", features = ["html_reports"] }
```

**Per-crate selection:**

- `lovely-tree`: `slotmap`, `smol_str`, `serde`, `serde_json`, `thiserror`, `uuid`. Optional `maud` behind `render` feature. Dev: `criterion`.
- `lovely-db`: `sqlx`, `tokio`, `async-trait`, `dashmap`, `serde`, `serde_json`, `uuid`, `chrono`, `thiserror`, `tracing`. Dev: `testcontainers`, `testcontainers-modules`, `tempfile`.
- `lovely-web`: `axum`, `axum-extra`, `axum-htmx`, `tower`, `tower-http`, `maud`, `oauth2`, `argon2`, `totp-rs`, `qrcode`, `jsonwebtoken`, `secrecy`, `tower-sessions`, `tower-sessions-sqlx-store`, `serde`, `serde_json`, `uuid`, `chrono`, `thiserror`, `anyhow`, `tracing`, `reqwest`, `lovely-db`, `lovely-tree` (with `render`). Dev: `scraper`, `fantoccini`, `lovely-test-support`.
- `lovely-server`: `tokio`, `axum`, `clap`, `dotenvy`, `tracing-subscriber`, `anyhow`, `secrecy`, `lovely-db`, `lovely-web`.
- `lovely-data`: `tokio`, `tracing-subscriber`, `clap`. Stub.
- `lovely-test-support`: `axum`, `reqwest`, `tokio`, `sqlx`, `uuid`, `lovely-db`, `lovely-web`, `lovely-server`.

**One non-obvious choice:** `rustls`, not `native-tls`, for both `sqlx` and `reqwest`. Distroless runtime has no system OpenSSL; rustls compiles into the binary. Trade-off: rustls is stricter about certs.

---

## 12. Risks, open questions, and what's deferred

**Risks:**

1. **SQLite-on-single-node = single point of failure for app data.** WAL + `synchronous=NORMAL` is durable across crashes; only hardware mid-fsync loses data. v2: `litestream`. Documented in `deploy/README.md`.
2. **Per-app `tokio::Mutex` re-entrancy.** Code review rule: `SchemaService::record` cannot be called from within `ensure_migrated`. Add a `debug_assert!` re-entrancy guard.
3. **`define_tags!` macro getting too large.** ~80 tags is fine; >150 → split into categories or move to proc-macro.
4. **Apple Sign In private key rotation.** `--apple-private-key-path` reads at startup and on `SIGHUP`. Account expires 2026-11-25 — calendar reminder.
5. **htmx attribute injection.** `AttrName` validator denylists `hx-*` and `on*` in user-provided attributes.
6. **Compile-time SQL checking.** `cargo sqlx prepare --workspace` in CI on schema changes, commit `.sqlx/`.
7. **Tree depth → stack overflow.** All tree walkers iterative with explicit `Vec` stack; fuzz test with 10k-deep tree.
8. **CSRF and htmx form submits.** Forms include hidden `<input name="_csrf" value="...">`; `tree.js` also sets `htmx.config.headers["X-CSRF-Token"]`. Both paths covered in e2e.

**Open questions (non-blocking):**

- A. **Email for password reset.** Username-only registration → some users have no email. Defer until B; v1 password reset only works for users with email.
- B. **Multi-tenant model.** Read of earlier answers: flat user → apps, Groups deferred. Confirm in milestone B planning.
- C. **Backups/restore.** Documented but not implemented in v1.
- D. **Rate limiting.** Not in v1. `tower-governor` when needed.
- E. **i18n.** Stub `t!("key")` macro that returns the literal, added in milestone A so future swap is mechanical.

**Explicitly deferred:**

- Custom domain support per app
- Payments/billing
- Scheduled jobs / cron
- Email/SMS notifications
- AI integrations
- Marketing surfaces
- Analytics dashboard
- Multi-language UI
- Public app discovery / marketplace

---

*End of design document.*
