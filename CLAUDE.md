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

Every page under `/apps/{slug}*` renders the sub-nav (Pages / Data /
Settings) so users can pivot without going up to the dashboard. The
active tab uses both `aria-current="page"` and an `.active` class. Hover,
focus, and active states are all distinct (see static/style.css ::
`nav.app-subnav`).

### Multi-step resource creation

When a resource has more than a name (collections have fields, pages
have elements, …) prefer:

1. POST creates the resource with the minimum (usually just `name`).
2. Redirect to a dedicated `/edit` (or first-class editor) for the rest.

So: collection create asks for name only, then lands on
`/apps/{slug}/data/{coll}/edit` where fields are added/renamed/deleted
one at a time. Don't take a comma-separated list for what should be a
configurable structure.

## Testing

Red-then-green per phase. Red tests are scaffolding; resist editing
them once written. Integration tests live in `crates/lovely-web/tests/`
and are gated on Docker (`#[ignore = "requires Docker"]`). Run with
`./bin/test-integration`.

## Memory of past conversations

Server lives at port 8080 in dev (Homebrew Postgres on 5432, isolated
`lovely_rs` role + db). Bin scripts in `./bin/` start everything.
