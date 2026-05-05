ALTER TABLE pages DROP CONSTRAINT pages_app_slug_unique;
ALTER TABLE pages ADD CONSTRAINT pages_slug_key UNIQUE (slug);
DROP INDEX pages_app_id_idx;
ALTER TABLE pages DROP COLUMN app_id;
DROP INDEX apps_one_default_per_owner_idx;
DROP INDEX apps_owner_id_idx;
DROP TABLE apps;
