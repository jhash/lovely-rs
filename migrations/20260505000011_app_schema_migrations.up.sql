-- The intent log: every schema change applied to a per-app SQLite
-- database is recorded here first, then applied to SQLite by
-- `SchemaService::ensure_migrated`. Postgres is the source of truth;
-- losing the SQLite file is recoverable by replaying these rows.
CREATE TABLE app_schema_migrations (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_id      UUID NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    version     BIGINT NOT NULL,
    intent      JSONB NOT NULL,
    forward_sql TEXT NOT NULL,
    reverse_sql TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by  UUID NOT NULL REFERENCES users(id),
    UNIQUE (app_id, version)
);

CREATE INDEX app_schema_migrations_app_id_version_idx
    ON app_schema_migrations(app_id, version);
