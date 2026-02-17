-- Track scan runs
CREATE TABLE IF NOT EXISTS scan_session (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL DEFAULT 'running',
    root_paths TEXT NOT NULL,
    files_scanned INTEGER DEFAULT 0,
    total_bytes INTEGER DEFAULT 0
);

-- All scanned files (duplicates populated during scan; extended to all files for directory analysis)
CREATE TABLE IF NOT EXISTS scanned_file (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    canonical_path TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    parent_dir TEXT NOT NULL,
    drive_letter TEXT DEFAULT '',
    file_size INTEGER NOT NULL,
    last_modified INTEGER NOT NULL,
    partial_hash INTEGER,
    content_hash INTEGER,
    scan_session_id INTEGER NOT NULL REFERENCES scan_session(id),
    marked_deleted INTEGER NOT NULL DEFAULT 0
);

-- Explicit duplicate groups
CREATE TABLE IF NOT EXISTS duplicate_group (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content_hash INTEGER NOT NULL,
    file_size INTEGER NOT NULL,
    file_count INTEGER NOT NULL,
    wasted_bytes INTEGER NOT NULL,
    UNIQUE(content_hash, file_size)
);

CREATE TABLE IF NOT EXISTS duplicate_group_member (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id INTEGER NOT NULL REFERENCES duplicate_group(id),
    file_id INTEGER NOT NULL REFERENCES scanned_file(id),
    UNIQUE(file_id)
);

-- Directory hierarchy
CREATE TABLE IF NOT EXISTS directory_node (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    parent_id INTEGER REFERENCES directory_node(id),
    total_size INTEGER DEFAULT 0,
    file_count INTEGER DEFAULT 0,
    depth INTEGER DEFAULT 0
);

-- Directory fingerprints for exact + similarity matching
CREATE TABLE IF NOT EXISTS directory_fingerprint (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    directory_id INTEGER NOT NULL UNIQUE REFERENCES directory_node(id),
    content_fingerprint TEXT NOT NULL,
    file_hash_set TEXT NOT NULL
);

-- Pre-computed similar directory pairs
CREATE TABLE IF NOT EXISTS directory_similarity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dir_a_id INTEGER NOT NULL,
    dir_b_id INTEGER NOT NULL,
    similarity_score REAL NOT NULL,
    shared_bytes INTEGER NOT NULL,
    match_type TEXT NOT NULL,
    UNIQUE(dir_a_id, dir_b_id),
    CHECK(dir_a_id < dir_b_id)
);

-- Deletion planning
CREATE TABLE IF NOT EXISTS deletion_plan (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL UNIQUE REFERENCES scanned_file(id),
    marked_at TEXT NOT NULL,
    strategy TEXT,
    executed_at TEXT,
    execution_result TEXT
);

-- Indexes for common UI queries
CREATE INDEX IF NOT EXISTS idx_file_size ON scanned_file(file_size);
CREATE INDEX IF NOT EXISTS idx_file_content_hash ON scanned_file(content_hash) WHERE content_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_file_parent_dir ON scanned_file(parent_dir);
CREATE INDEX IF NOT EXISTS idx_file_canonical_path ON scanned_file(canonical_path);
CREATE INDEX IF NOT EXISTS idx_group_wasted ON duplicate_group(wasted_bytes DESC);
CREATE INDEX IF NOT EXISTS idx_group_member_group ON duplicate_group_member(group_id);
CREATE INDEX IF NOT EXISTS idx_dir_parent ON directory_node(parent_id);
CREATE INDEX IF NOT EXISTS idx_dir_fingerprint ON directory_fingerprint(content_fingerprint);
CREATE INDEX IF NOT EXISTS idx_dir_similarity_score ON directory_similarity(similarity_score DESC);
