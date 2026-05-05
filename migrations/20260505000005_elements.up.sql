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
