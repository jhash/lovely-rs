CREATE TABLE element_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    page_id UUID NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    -- Full snapshot of the page's elements at this point. Storage is
    -- cheap; replay is `DELETE all elements for page; INSERT each row`.
    snapshot_json JSONB NOT NULL,
    -- Monotonic per page; we walk forward/backward from `cursor`.
    seq BIGSERIAL NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX element_revisions_page_seq_idx ON element_revisions(page_id, seq);

-- Per-page cursor pointing at the "current" revision. Undo decrements,
-- redo increments. NULL means no revisions yet.
ALTER TABLE pages ADD COLUMN revision_cursor BIGINT;
