-- This file should undo anything in `up.sql`

ALTER TABLE dupe_file
DROP CONSTRAINT fk_dupe_file_session_id;

ALTER TABLE dupe_file
DROP CONSTRAINT uk_dupe_file_canonical_path_session_id;

ALTER TABLE dupe_file_part
DROP CONSTRAINT fk_dupe_file_part_session_id;

ALTER TABLE dupe_file_part
DROP CONSTRAINT uk_dupe_file_part_canonical_path_session_id;

DROP TABLE dupe_file;
DROP TABLE dupe_file_part;
DROP TABLE dedupe_session;