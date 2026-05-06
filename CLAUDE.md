# lovely-rs — working notes for Claude

This file captures durable preferences and conventions the user has
articulated about how to work on this repo. Treat it as load-bearing.

## UI conventions

### "New X" lives on its own page

Whenever a new resource type is added to the app — apps, pages,
collections, anything user-facing — the "create" form lives on a
dedicated route, never inline on the index/list page.

- Index page: list-of-things + a "New X" button that links to the new
  page (button-style anchor, not a form button).
- New page: `/.../{resource}/new` (sometimes nested deeper, but always
  its own URL). Has its own breadcrumb + sub-nav.
- POST handler at the parent collection URL (`/apps`, `/apps/{slug}/data`,
  `/apps/{slug}/pages`) — the new GET is just the form's host page.

Examples already in the codebase:
- `/apps/new` → POST `/apps`
- `/apps/{slug}/pages/new` → POST `/apps/{slug}/pages`
- `/apps/{slug}/data/new` → POST `/apps/{slug}/data`

Share view code via small helpers (e.g. `app_subnav`, `pages_summary_section`)
rather than copy-paste.

### App sub-nav

Every page under `/apps/{slug}*` renders the sub-nav (Home / Pages /
Data / Settings) so users can pivot without going up to the dashboard.
The active tab uses both `aria-current="page"` and an `.active` class.
Hover, focus, and active states are all distinct (see static/style.css
:: `nav.app-subnav`).

### Breadcrumbs

Last segment is rendered as `<span class="current">…</span>` and styled
bold + dark via `nav.breadcrumbs .current` so the active location is
visually distinct from the navigable ancestors.

### Click-target wrappers

Tree row "buttons" and the canvas backdrop are plain `<div role="button"
tabindex="0">` wrappers — htmx hx-get/hx-post bind to clicks regardless
of element. A small global `keydown` handler in `static/tree.js`
forwards Enter/Space on `[role="button"]` to a click so keyboard
accessibility is preserved without real `<button>` tags.

### Reusable form atoms

`crates/lovely-web/src/views/components.rs` hosts shared maud helpers
(`labeled_checkbox(name, label, checked)`). Use these for inline
checkboxes — they enforce a single `.checkbox-row` layout that wins
against the `.inspector-form` column flex.

## Page model

### Default Home page per app

Every app gets a default Home page (slug = `""`, title = `Home`) at
`create_app` time. The empty-slug page can be published or not, but
`delete_page_handler` returns 422 — the home page is structural and
can't be removed.

### Page rendering pipeline

`render_public` (and the editor canvas fragment) run these passes in
order on the loaded element rows:

1. `expand_repeaters` — every `data-lovely-repeat` element's first
   child is cloned once per record in the named collection, with
   `{{field}}` interpolation in text + attrs.
2. `resolve_bindings` — `data-lovely-bind="coll.field"` substitutes the
   first record's value as the element's text (or `value` attr for
   `<input>` / `<textarea>` content).
3. `interpolate_collection_refs` — global `{{coll.field}}` placeholders
   in any `#text` payload or attribute value are replaced from a
   per-collection first-record cache.
4. `auto_wire_forms` — forms whose descendants carry
   `data-lovely-source` get their action + method rewritten to
   `/p/{user}/{slug}/_submit/{coll}`, descendant input `name` attrs
   mapped to the source field, and a synthetic `<input type="hidden"
   name="_csrf">` injected.

### Public render paths

- `/{username}` — default app's home page.
- `/{username}/{page-slug}` — default app's named page.
- `/{username}/{app-slug}/{page-slug}` — non-default app's named page
  (use `~home` for empty page-slug).

Non-owners visiting an unpublished page get a 303 to `/`. Unlisted
pages 404 (semantically distinct from "not yet published"). Owners
always see their drafts.

### Element tag conventions

- `lovely_tree::ElementTag::TEXT_NAME` is the canonical `"#text"`
  literal; prefer this constant over the bare string.
- `ElementRow::is_text()` / `ElementDbRow::is_text()` are the right way
  to check for the inline text node.
- Text content lives ONLY on `#text` nodes. Regular elements get an
  empty payload — `post_add_element`/`patch_element` enforce this and
  the renderer drops `payload.text` for non-text tags.

### Undo/redo

`create_page` takes a baseline snapshot so the first edit can be undone
back to the just-created state. Mutating handlers call `snapshot_page`
*after* their write — the snapshot represents the post-state, and undo
walks `seq < cursor` to find the previous post-state. New edits after
an undo truncate the redo branch.

### Selection after add

Element-creation handlers emit `HX-Trigger: {"lovely:select":{"id":
"...","focus":""|"text"}}` (no `preview-stale` — that races the asides'
initial-render hx-get URLs). The `lovely:select` JS handler updates the
asides' static hx-get URLs to `?sel=NEW`, runs `htmx.process`, swaps
inspector + tree to the new selection, and triggers `preview-stale` on
the canvas for an inline re-render. For #text additions, focus the
content textarea once it's in the DOM.

### Multi-step resource creation

When a resource has more than a name (collections have fields, pages
have elements, …) prefer:

1. POST creates the resource with the minimum (usually just `name`).
2. Redirect to a dedicated `/edit` (or first-class editor) for the rest.

So: collection create asks for name only, then lands on
`/apps/{slug}/data/{coll}/edit` where fields are added/renamed/deleted
one at a time. Don't take a comma-separated list for what should be a
configurable structure.

## Per-app SQLite (milestone C)

### Identifier safety

Any user-supplied name that ends up in a DDL string (collection name,
field name) MUST go through `lovely_db::Identifier::new`. The newtype
enforces:

- 1–63 bytes;
- lowercase ASCII letters, digits, underscores;
- leading char is letter or underscore;
- not a SQL reserved word (`select`, `where`, `when`, …);
- not the `_lovely` internal namespace.

The DDL renderer splats `Identifier` values straight into format
strings — never re-validate by hand, never accept `&str` instead.

### Intent log + SchemaService

Postgres `app_schema_migrations` is the source of truth. Every
collection / field create / rename / delete records an `Intent` row
via `SchemaService::record(app_id, user, intent)` AFTER the Postgres
mirror write succeeds (sequential — failure leaves an orphaned but
harmless intent row at worst).

`SchemaService::ensure_migrated(app_id, sqlite)` is idempotent and
runs implicitly on every `LocalSqliteAppStore::get_pool` call. Per-app
`tokio::Mutex` + a SQLite-side `_lovely_schema_version` pointer make
double-apply impossible across processes.

### Record dual-write

`post_record_create` + `post_public_submit` write to Postgres first
(authoritative), then best-effort mirror the row into SQLite via
`mirror_record_insert`. Failures are logged + swallowed so the user
flow doesn't break. The renderer still reads from Postgres records;
SQLite is the staging substrate for the future cutover.

### SQL console

`/apps/{slug}/data/console` runs read-only `SELECT/WITH/EXPLAIN/PRAGMA`
against the per-app SQLite. Multi-statement is rejected. Results capped
at 100 rows. Anything that needs to mutate user data must go through a
typed handler — never SQL.

## Testing

Red-then-green per phase. Red tests are scaffolding; resist editing
them once written. Integration tests live in `crates/lovely-web/tests/`
and are gated on Docker (`#[ignore = "requires Docker"]`). Run with
`./bin/test-integration`.

## Memory of past conversations

Server lives at port 8080 in dev (Homebrew Postgres on 5432, isolated
`lovely_rs` role + db). Bin scripts in `./bin/` start everything.
