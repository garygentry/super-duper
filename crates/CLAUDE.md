# CLAUDE.md — Rust Crates

The Rust workspace is the core of Super Duper. `super-duper-core` is an rlib containing all
business logic (scanning, hashing, analysis, SQLite storage). `super-duper-ffi` wraps it as a
cdylib producing `super_duper_ffi.dll`, consumed by the WinUI 3 C# app via P/Invoke.
`super-duper-cli` is a headless binary for pipeline testing and scripting.

## Build Commands

```bash
cargo build --workspace
cargo build --release --workspace
cargo run -p super-duper-cli -- process            # Full duplicate detection pipeline
cargo run -p super-duper-cli -- analyze-directories # Build directory fingerprints + similarity
cargo run -p super-duper-cli -- count-hash-cache   # Show RocksDB cache entry count
cargo run -p super-duper-cli -- print-config       # Print loaded configuration
cargo run -p super-duper-cli -- truncate-db        # Truncate all SQLite tables
```

## Environment Variables

Configured via `.env` file in the working directory:

- `TRACING_LEVEL` — Log verbosity (debug, info, warn, error, trace)
- `LOG_FILE_PATH` — File log output path (default: `./logs/sd.log`)
- `HASH_CACHE_PATH` — RocksDB cache location (default: `content_hash_cache.db`)

## Configuration

`Config.toml` in the repo root defines scan targets and ignore patterns:

```toml
root_paths = ["../test-data/folder1", "../test-data/folder2"]
ignore_patterns = ["**/node_modules/**", "*/$RECYCLE.BIN"]
```

## Workspace Structure

```
super-duper-core/                 # rlib — all business logic
  src/
    lib.rs                        # Public API exports
    config.rs                     # AppConfig, load_configuration()
    engine.rs                     # ScanEngine orchestrator
    error.rs                      # Unified error types (thiserror)
    progress.rs                   # ProgressReporter trait
    scanner/walk.rs               # Parallel directory traversal (rayon)
    hasher/xxhash.rs              # Two-tier XxHash64 hashing
    hasher/cache.rs               # RocksDB hash cache
    storage/sqlite.rs             # SQLite connection + pragmas + schema migration
    storage/models.rs             # Plain Rust structs (no ORM)
    storage/queries.rs            # Insert/query operations (rusqlite)
    storage/schema.sql            # SQLite DDL
    analysis/dir_fingerprint.rs   # Directory fingerprinting (bottom-up XxHash64 tree)
    analysis/dir_similarity.rs    # Jaccard similarity + exact/subset matching
    analysis/deletion_plan.rs     # Deletion workflow + verification
    platform/windows.rs           # cfg-gated Windows drive letter extraction

super-duper-ffi/                  # cdylib — C-compatible FFI
  src/
    handle.rs                     # Handle table (u64 → Arc<Mutex<EngineState>>)
    types.rs                      # C-repr types, SdResultCode enum
    callbacks.rs                  # Progress callback (C function pointer bridge)
    error.rs                      # Thread-local error detail storage
    queries.rs                    # Paginated query FFI functions
    actions.rs                    # Engine lifecycle + deletion FFI
  super_duper.h                   # Auto-generated C header (cbindgen)
  build.rs                        # Invokes cbindgen to regenerate the header

super-duper-cli/                  # binary — headless CLI
  src/
    main.rs                       # Entry point + command dispatch
    commands.rs                   # clap derive CLI definition
    logging.rs                    # Dual tracing output (stdout + file)
    progress.rs                   # CliReporter (indicatif progress bars)
```

## Processing Pipeline (super-duper-core)

1. **Scan** (`scanner/walk.rs`) — Parallel directory traversal via rayon. Builds
   `file_size → Vec<PathBuf>` map, filtering by glob ignore patterns. Skips symlinks and 0-byte files.

2. **Hash** (`hasher/`) — Two-tier: partial 1KB XxHash64 (`xxhash.rs`), then full hash only on
   partial matches. RocksDB cache (`cache.rs`) keyed on
   `canonical_path|modified_timestamp.subsec_nanos` avoids re-hashing unchanged files.

3. **DB Write** (`engine.rs` + `storage/`) — Writes to SQLite: `scanned_file` records +
   `duplicate_group` records in a transaction. Schema defined in `storage/schema.sql`.

## Directory Analysis (analysis/)

- **Fingerprinting** (`dir_fingerprint.rs`) — Bottom-up: builds `directory_node` tree, computes
  XxHash64 fingerprint of sorted recursive child hashes. Populates `directory_node` and
  `directory_fingerprint` tables.

- **Similarity** (`dir_similarity.rs`) — Jaccard index on hash sets (0.5 threshold default).
  Exact matches via fingerprint comparison. Subset detection. Populates `directory_similarity` table.

- **Deletion** (`deletion_plan.rs`) — Mark files/directories, auto-mark duplicates, execute with
  verification. Writes to `deletion_plan` table.

## Concurrency Model

Uses `rayon` for data parallelism (`par_iter`, `par_bridge`) and `DashMap` for lock-free concurrent
hash maps. No async runtime. `ScanEngine::cancel()` sets an `Arc<AtomicBool>` cancel token
passed through all phases.

## FFI Layer (super-duper-ffi)

### Design Principles

- **Handle-based** — UI receives opaque `u64` handles, never raw pointers. Handle table in
  `handle.rs` maps `u64 → Arc<Mutex<EngineState>>`.
- **Rust allocates, Rust frees** — every returned buffer has a matching `sd_free_*()` function.
  Never free FFI-returned pointers in C#.
- **Error reporting** — `SdResultCode` (from `types.rs`) as return value + thread-local error
  detail via `sd_last_error_message()`. C# reads this immediately after any non-Ok result.
- **Paginated queries** — all list-returning functions take `offset`/`limit` and return a page
  struct with `Count` and `TotalAvailable` fields.

### Key Files

- `handle.rs` — `allocate_handle()`, `with_handle()`, `destroy_handle()`; `EngineState` holds
  the engine, db handle, scan paths, is_scanning flag, cancel token, and active_session_id.
- `types.rs` — `#[repr(C)]` structs: `SdDuplicateGroup`, `SdFileRecord`, `SdDirectoryNode`,
  `SdDirectorySimilarity`, `SdSessionInfo`; page structs; `SdResultCode` enum (Ok=0, ..., InternalError=99).
- `callbacks.rs` — `SdProgressCallback` function pointer type; `FfiProgressBridge` marshals
  progress events to a C callback.
- `error.rs` — `thread_local!` error message storage; `set_last_error()`, `map_core_error()`.
- `queries.rs` — `sd_query_duplicate_groups()`, `sd_query_group_files()`,
  `sd_query_directory_nodes()`, `sd_query_directory_similarity()`, `sd_query_sessions()`.
- `actions.rs` — `sd_engine_create()`, `sd_engine_destroy()`, `sd_engine_start_scan()`,
  `sd_engine_cancel_scan()`, `sd_engine_wait_scan()`; deletion FFI (`sd_mark_file_for_deletion()`,
  `sd_deletion_execute()`).

### C Header and C# Wrapper

- Generated header: `crates/super-duper-ffi/super_duper.h` (regenerated by `cargo build`)
- C# P/Invoke declarations: `ui/windows/SuperDuper/NativeMethods/SuperDuperEngine.cs`
- C# managed wrapper: `ui/windows/SuperDuper/NativeMethods/EngineWrapper.cs`
