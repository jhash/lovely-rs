CREATE TABLE apps (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug         TEXT NOT NULL,
    name         TEXT NOT NULL,
    description  TEXT,
    owner_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    is_default   BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(owner_id, slug)
);
CREATE INDEX apps_owner_id_idx ON apps(owner_id);
CREATE UNIQUE INDEX apps_one_default_per_owner_idx
    ON apps(owner_id) WHERE is_default;

-- Backfill: every existing user gets a default app named "Personal"
-- with slug = "personal".
INSERT INTO apps (slug, name, owner_id, is_default)
SELECT 'personal', 'Personal', id, TRUE FROM users;

ALTER TABLE pages ADD COLUMN app_id UUID REFERENCES apps(id) ON DELETE CASCADE;
CREATE INDEX pages_app_id_idx ON pages(app_id);

-- Backfill: every existing page goes into its author's default app.
UPDATE pages
SET app_id = (
    SELECT id FROM apps
    WHERE owner_id = pages.author_id AND is_default
    LIMIT 1
);

ALTER TABLE pages ALTER COLUMN app_id SET NOT NULL;
ALTER TABLE pages DROP CONSTRAINT pages_slug_key;
ALTER TABLE pages ADD CONSTRAINT pages_app_slug_unique UNIQUE (app_id, slug);
