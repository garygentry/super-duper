CREATE TABLE dupe_file (
    id SERIAL PRIMARY KEY,
    canonical_path TEXT NOT NULL UNIQUE,
    path_no_drive TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_extension TEXT NULL,
    drive_letter CHAR NULL,
    parent_dir TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    last_modified TIMESTAMP NOT NULL,
    full_hash BIGINT NOT NULL,
    partial_hash BIGINT NOT NULL
);

CREATE TABLE dupe_file_part
(
    id              INTEGER PRIMARY KEY,
    canonical_path  TEXT NOT NULL UNIQUE,
    name            TEXT NOT NULL, 
    file_size       BIGINT NOT NULL,
    part_type       INTEGER NOT NULL,
    parent_id       INTEGER NULL
)