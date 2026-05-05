-- Per-app theme: a JSON map of CSS-var-name -> value, injected into
-- the public <head> as `:root { --lovely-X: Y; }`.
ALTER TABLE apps ADD COLUMN theme_json JSONB NOT NULL DEFAULT '{}'::jsonb;

-- Per-page custom <head> snippet, e.g. OG tags / favicons. Owner-only,
-- sanitized server-side at render time (no <script>, no on*).
ALTER TABLE pages ADD COLUMN head_html TEXT NOT NULL DEFAULT '';

-- Per-page password gate (argon2 hash). NULL = unprotected.
ALTER TABLE pages ADD COLUMN password_hash TEXT;
ALTER TABLE pages ADD COLUMN unlisted BOOLEAN NOT NULL DEFAULT FALSE;
