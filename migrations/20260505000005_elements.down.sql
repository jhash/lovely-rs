ALTER TABLE pages DROP CONSTRAINT IF EXISTS pages_root_element_fk;
DROP INDEX IF EXISTS elements_parent_id_idx;
DROP INDEX IF EXISTS elements_page_id_idx;
DROP TABLE IF EXISTS elements;
