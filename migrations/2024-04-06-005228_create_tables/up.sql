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
    partial_hash BIGINT NOT NULL,
    session_id INTEGER NOT NULL
);

CREATE TABLE dupe_file_part
(
    id              INTEGER PRIMARY KEY,
    canonical_path  TEXT NOT NULL UNIQUE,
    name            TEXT NOT NULL, 
    file_size       BIGINT NOT NULL,
    part_type       INTEGER NOT NULL,
    parent_id       INTEGER NULL,
    session_id      INTEGER NOT NULL
);

CREATE TABLE dedupe_session (
    id SERIAL PRIMARY KEY,
    run_start_time TIMESTAMP NOT NULL,
    process_duration DECIMAL NOT NULL,
    scan_duration DECIMAL NOT NULL,
    scan_input_paths TEXT NOT NULL,
    scan_file_count INTEGER NOT NULL,
    scan_file_size BIGINT NOT NULL,
    scan_size_dupe_file_count INTEGER NOT NULL,
    scan_size_dupe_file_size BIGINT NOT NULL,
    hash_duration DECIMAL NOT NULL,
    hash_scan_file_count INTEGER NOT NULL,
    hash_scan_file_size BIGINT NOT NULL,
    hash_cache_hit_full_count INTEGER NOT NULL,
    hash_cache_hit_partial_count INTEGER NOT NULL,
    hash_gen_partial_count INTEGER NOT NULL,
    hash_gen_partial_duration DECIMAL NOT NULL,
    hash_gen_partial_file_size BIGINT NOT NULL,
    hash_gen_full_count INTEGER NOT NULL,
    hash_gen_full_file_size BIGINT NOT NULL,
    hash_gen_full_duration DECIMAL NOT NULL,
    hash_confirmed_dupe_count INTEGER NOT NULL,
    hash_confirmed_dupe_size BIGINT NOT NULL,
    hash_confirmed_dupe_distinct_count INTEGER NOT NULL,
    hash_confirmed_dupe_distinct_size BIGINT NOT NULL,
    cache_map_to_dupe_vec_duration DECIMAL NOT NULL,
    cache_map_to_dupe_vec_count INTEGER NOT NULL,
    db_dupe_file_insert_duration DECIMAL NOT NULL,
    db_dupe_file_insert_count INTEGER NOT NULL
);
ALTER TABLE dupe_file
ADD CONSTRAINT fk_dupe_file_session_id FOREIGN KEY (session_id) REFERENCES dedupe_session(id);

ALTER TABLE dupe_file_part
ADD CONSTRAINT fk_dupe_file_part_session_id FOREIGN KEY (session_id) REFERENCES dedupe_session(id);
