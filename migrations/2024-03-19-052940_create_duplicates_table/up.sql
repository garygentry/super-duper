CREATE TABLE dupe_file (
    id SERIAL PRIMARY KEY,
    canonical_name TEXT NOT NULL,
    file_index BIGINT NOT NULL,
    drive_letter CHAR NOT NULL,
    path_no_drive TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    last_modified TIMESTAMP NOT NULL,
    content_hash BIGINT NOT NULL,
    volume_serial_number INTEGER NOT NULL
);