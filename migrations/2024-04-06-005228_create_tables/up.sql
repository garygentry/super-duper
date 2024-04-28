CREATE TABLE dupe_file (
    id SERIAL PRIMARY KEY,
    canonical_path TEXT NOT NULL,
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
    id INTEGER,
    canonical_path TEXT NOT NULL,
    name TEXT NOT NULL, 
    file_size BIGINT NOT NULL,
    part_type INTEGER NOT NULL,
    parent_id INTEGER NULL,
    has_child_dirs BOOLEAN NOT NULL,
    session_id INTEGER NOT NULL,
    PRIMARY KEY (id, session_id)
);

CREATE TABLE dedupe_session (
    id SERIAL PRIMARY KEY,
    run_start_time TIMESTAMP NOT NULL,
    process_duration REAL NOT NULL,
    scan_duration REAL NOT NULL,
    scan_input_paths TEXT NOT NULL,
    scan_file_count INTEGER NOT NULL,
    scan_file_size BIGINT NOT NULL,
    scan_size_dupe_file_count INTEGER NOT NULL,
    scan_size_dupe_file_size BIGINT NOT NULL,
    hash_duration REAL NOT NULL,
    hash_scan_file_count INTEGER NOT NULL,
    hash_scan_file_size BIGINT NOT NULL,
    hash_cache_hit_full_count INTEGER NOT NULL,
    hash_cache_hit_partial_count INTEGER NOT NULL,
    hash_gen_partial_count INTEGER NOT NULL,
    hash_gen_partial_duration REAL NOT NULL,
    hash_gen_partial_file_size BIGINT NOT NULL,
    hash_gen_full_count INTEGER NOT NULL,
    hash_gen_full_file_size BIGINT NOT NULL,
    hash_gen_full_duration REAL NOT NULL,
    hash_confirmed_dupe_count INTEGER NOT NULL,
    hash_confirmed_dupe_size BIGINT NOT NULL,
    hash_confirmed_dupe_distinct_count INTEGER NOT NULL,
    hash_confirmed_dupe_distinct_size BIGINT NOT NULL,
    cache_map_to_dupe_vec_duration REAL NOT NULL,
    cache_map_to_dupe_vec_count INTEGER NOT NULL,
    db_dupe_file_insert_duration REAL NOT NULL,
    db_dupe_file_insert_count INTEGER NOT NULL
);
ALTER TABLE dupe_file
ADD CONSTRAINT fk_dupe_file_session_id FOREIGN KEY (session_id) REFERENCES dedupe_session(id);

ALTER TABLE dupe_file
ADD CONSTRAINT uk_dupe_file_canonical_path_session_id UNIQUE (canonical_path, session_id);

ALTER TABLE dupe_file_part
ADD CONSTRAINT fk_dupe_file_part_session_id FOREIGN KEY (session_id) REFERENCES dedupe_session(id);

ALTER TABLE dupe_file_part
ADD CONSTRAINT uk_dupe_file_part_canonical_path_session_id UNIQUE (canonical_path, session_id);
