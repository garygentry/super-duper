# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Project Is

A multi-platform duplicate file detection tool built as a Cargo workspace. A **Rust core library** handles scanning, hashing, analysis, and storage. **Native UIs** consume the library via FFI — starting with a WinUI 3 (C#/.NET) app on Windows 11. Uses a two-tier hashing strategy: partial hash (first 1KB with XxHash64) to quickly eliminate non-matches, then full hash only on candidates. Hash results are cached in RocksDB. Results are stored in SQLite.

## Workspace Structure

```
super-duper/
  Cargo.toml                          # [workspace] members
  Config.toml                         # Scan targets and ignore patterns
  crates/
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
        storage/sqlite.rs             # SQLite connection + pragmas
        storage/models.rs             # Plain Rust structs (no ORM)
        storage/queries.rs            # Insert/query operations (rusqlite)
        storage/schema.sql            # SQLite DDL
        analysis/dir_fingerprint.rs   # Directory fingerprinting (bottom-up)
        analysis/dir_similarity.rs    # Jaccard similarity + exact matching
        analysis/deletion_plan.rs     # Deletion workflow
        platform/mod.rs               # cfg-gated platform code
    super-duper-ffi/                  # cdylib — C-compatible FFI
      src/
        handle.rs                     # Handle table (u64 → EngineState)
        types.rs                      # C-repr types, SdResultCode
        callbacks.rs                  # Progress callback support
        error.rs                      # Thread-local error detail
        queries.rs                    # Paginated query FFI functions
        actions.rs                    # Engine lifecycle + deletion FFI
    super-duper-cli/                  # binary — headless CLI
      src/
        main.rs                       # Entry point + command dispatch
        commands.rs                   # clap derive CLI definition
        logging.rs                    # Dual tracing (stdout + file)
  ui/
    windows/                          # WinUI 3 C# project
      SuperDuper.sln
      SuperDuper/
        NativeMethods/                # P/Invoke declarations
        ViewModels/                   # MVVM ViewModels
        Views/                        # XAML pages
      TestHarness/                    # Console test harness for FFI
```

## Build & Run Commands

```bash
cargo build --workspace            # Build all crates (debug)
cargo build --release --workspace  # Build all crates (release)
cargo run -p super-duper-cli -- process            # Full duplicate detection pipeline
cargo run -p super-duper-cli -- analyze-directories # Build directory fingerprints + similarity
cargo run -p super-duper-cli -- count-hash-cache   # Show RocksDB cache entry count
cargo run -p super-duper-cli -- print-config       # Print loaded configuration
cargo run -p super-duper-cli -- truncate-db        # Truncate all SQLite tables
```

## Database

Uses embedded SQLite (via rusqlite with bundled feature). Database file: `super_duper.db` in the working directory.

Key tables:
- `scan_session` — tracks scan runs
- `scanned_file` — files with metadata and hashes
- `duplicate_group` / `duplicate_group_member` — duplicate groups
- `directory_node` / `directory_fingerprint` / `directory_similarity` — directory analysis
- `deletion_plan` — files marked for deletion

SQLite is configured with WAL mode, 64MB cache, 256MB mmap for performance.

## Environment Variables

Configured via `.env` file:
- `TRACING_LEVEL` — Log verbosity (debug, info, warn, error, trace)
- `LOG_FILE_PATH` — File log output path (default: `./logs/sd.log`)
- `HASH_CACHE_PATH` — RocksDB cache location (default: `content_hash_cache.db`)

## Architecture

### Processing Pipeline (super-duper-core)

1. **Scan** (`scanner/walk.rs`) — Parallel directory traversal via rayon. Builds `file_size → Vec<PathBuf>` map, filtering by glob ignore patterns. Skips symlinks and 0-byte files.
2. **Hash** (`hasher/`) — Two-tier: partial 1KB XxHash64, then full hash only on partial matches. RocksDB cache (`cache.rs`) keyed on `canonical_path|modified_timestamp.subsec_nanos` avoids re-hashing.
3. **DB Write** (`engine.rs` + `storage/`) — Writes to SQLite: scanned_file records + duplicate_group records in a transaction.

### Directory Analysis (analysis/)

- **Fingerprinting** (`dir_fingerprint.rs`) — Bottom-up: builds directory_node tree, computes XxHash64 fingerprint of sorted recursive child hashes.
- **Similarity** (`dir_similarity.rs`) — Jaccard index on hash sets. Exact matches via fingerprint comparison. Subset detection.
- **Deletion** (`deletion_plan.rs`) — Mark files/directories, auto-mark duplicates, execute with verification.

### FFI Layer (super-duper-ffi)

- Handle-based: UI receives opaque u64 handles, never raw pointers
- Rust allocates, Rust frees: every returned buffer has a matching `sd_free_*()` function
- Error codes + thread-local detail via `sd_last_error_message()`
- Paginated queries for large datasets

### Concurrency Model

Uses `rayon` for data parallelism (par_iter, par_bridge) and `DashMap` for lock-free concurrent hash maps. No async runtime.

## Configuration

`Config.toml` defines scan targets and ignore patterns:
```toml
root_paths = ["../test-data/folder1", "../test-data/folder2"]
ignore_patterns = ["**/node_modules/**", "*/$RECYCLE.BIN"]
```

## Platform Notes

Has Windows-specific code (`platform/windows.rs`) for drive letter extraction, cfg-gated with `#[cfg(target_os = "windows")]`. On Unix, drive letter fields default to empty.

The FFI crate compiles as `cdylib` producing `super_duper_ffi.dll` (Windows) or `libsuper_duper_ffi.so` (Linux).
