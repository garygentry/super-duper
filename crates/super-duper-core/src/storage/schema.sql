PRAGMA user_version = 3;

-- Reusable, user-owned scan definitions.
CREATE TABLE IF NOT EXISTS scan_session (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    roots_json TEXT NOT NULL,
    ignore_patterns_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Immutable executions. parameters_json is the authoritative settings snapshot.
CREATE TABLE IF NOT EXISTS scan_run (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES scan_session(id) ON DELETE CASCADE,
    parameters_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'running', 'cancelling', 'completed', 'cancelled', 'failed', 'interrupted')),
    phase TEXT CHECK(phase IS NULL OR phase IN
        ('discovering', 'hashing', 'persisting', 'analyzing_folders', 'finalizing')),
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    files_discovered INTEGER NOT NULL DEFAULT 0 CHECK(files_discovered >= 0),
    bytes_discovered INTEGER NOT NULL DEFAULT 0 CHECK(bytes_discovered >= 0),
    files_hashed INTEGER NOT NULL DEFAULT 0 CHECK(files_hashed >= 0),
    duplicate_file_groups INTEGER NOT NULL DEFAULT 0 CHECK(duplicate_file_groups >= 0),
    duplicate_folder_groups INTEGER NOT NULL DEFAULT 0 CHECK(duplicate_folder_groups >= 0),
    wasted_bytes INTEGER NOT NULL DEFAULT 0 CHECK(wasted_bytes >= 0),
    warning_count INTEGER NOT NULL DEFAULT 0 CHECK(warning_count >= 0),
    error_message TEXT,
    engine_version TEXT NOT NULL
);

-- Immutable file snapshots owned by exactly one run.
CREATE TABLE IF NOT EXISTS scanned_file (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    root_path TEXT NOT NULL DEFAULT '',
    canonical_path TEXT NOT NULL,
    relative_path TEXT NOT NULL DEFAULT '',
    file_name TEXT NOT NULL,
    parent_dir TEXT NOT NULL,
    drive_letter TEXT NOT NULL DEFAULT '',
    file_size INTEGER NOT NULL,
    last_modified INTEGER NOT NULL,
    partial_hash INTEGER,
    content_hash INTEGER,
    file_identity TEXT,
    warning_message TEXT,
    marked_deleted INTEGER NOT NULL DEFAULT 0,
    UNIQUE(run_id, canonical_path)
);

CREATE TABLE IF NOT EXISTS duplicate_group (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    content_hash INTEGER NOT NULL,
    file_size INTEGER NOT NULL,
    file_count INTEGER NOT NULL,
    wasted_bytes INTEGER NOT NULL,
    UNIQUE(run_id, content_hash, file_size)
);

CREATE TABLE IF NOT EXISTS duplicate_group_member (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id INTEGER NOT NULL REFERENCES duplicate_group(id) ON DELETE CASCADE,
    file_id INTEGER NOT NULL REFERENCES scanned_file(id) ON DELETE CASCADE,
    UNIQUE(group_id, file_id)
);

-- Directory analysis remains deferred for the MVP UI, but ownership is run-scoped now.
CREATE TABLE IF NOT EXISTS directory_node (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    parent_id INTEGER REFERENCES directory_node(id) ON DELETE CASCADE,
    total_size INTEGER NOT NULL DEFAULT 0,
    file_count INTEGER NOT NULL DEFAULT 0,
    depth INTEGER NOT NULL DEFAULT 0,
    UNIQUE(run_id, path)
);

CREATE TABLE IF NOT EXISTS directory_fingerprint (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    directory_id INTEGER NOT NULL UNIQUE REFERENCES directory_node(id) ON DELETE CASCADE,
    content_fingerprint TEXT NOT NULL,
    file_hash_set TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS directory_similarity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES scan_run(id) ON DELETE CASCADE,
    dir_a_id INTEGER NOT NULL REFERENCES directory_node(id) ON DELETE CASCADE,
    dir_b_id INTEGER NOT NULL REFERENCES directory_node(id) ON DELETE CASCADE,
    similarity_score REAL NOT NULL,
    shared_bytes INTEGER NOT NULL,
    match_type TEXT NOT NULL,
    UNIQUE(run_id, dir_a_id, dir_b_id),
    CHECK(dir_a_id < dir_b_id)
);

CREATE TABLE IF NOT EXISTS deletion_plan (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL UNIQUE REFERENCES scanned_file(id) ON DELETE CASCADE,
    marked_at TEXT NOT NULL,
    strategy TEXT,
    executed_at TEXT,
    execution_result TEXT
);

CREATE INDEX IF NOT EXISTS idx_session_name ON scan_session(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_run_session ON scan_run(session_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_run_status ON scan_run(status);
CREATE INDEX IF NOT EXISTS idx_file_run_size ON scanned_file(run_id, file_size);
CREATE INDEX IF NOT EXISTS idx_file_run_hash ON scanned_file(run_id, content_hash) WHERE content_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_file_run_parent ON scanned_file(run_id, parent_dir);
CREATE INDEX IF NOT EXISTS idx_file_run_path ON scanned_file(run_id, canonical_path);
CREATE INDEX IF NOT EXISTS idx_group_run ON duplicate_group(run_id);
CREATE INDEX IF NOT EXISTS idx_group_run_wasted ON duplicate_group(run_id, wasted_bytes DESC);
CREATE INDEX IF NOT EXISTS idx_group_member_group ON duplicate_group_member(group_id);
CREATE INDEX IF NOT EXISTS idx_dir_run_parent ON directory_node(run_id, parent_id);
CREATE INDEX IF NOT EXISTS idx_dir_fingerprint ON directory_fingerprint(content_fingerprint);
CREATE INDEX IF NOT EXISTS idx_dir_similarity_run_score ON directory_similarity(run_id, similarity_score DESC);
