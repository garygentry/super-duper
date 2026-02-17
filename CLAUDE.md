# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Project Is

A Rust CLI tool that finds duplicate files across directories by content-hashing, then stores metadata in PostgreSQL for later analysis. Uses a two-tier hashing strategy: partial hash (first 1KB with XxHash64) to quickly eliminate non-matches, then full hash only on candidates. Hash results are cached in RocksDB to avoid re-hashing unchanged files.

## Build & Run Commands

```bash
cargo build                      # Debug build
cargo build --release            # Release build
cargo run -- process             # Full duplicate detection pipeline
cargo run -- build-path-parts    # Build path_part hierarchy from dupe_file table
cargo run -- count-hash-cache    # Show RocksDB cache entry count
cargo run -- print-config        # Print loaded configuration
cargo run -- truncate-db         # Truncate all database tables
```

## Database Setup

Requires a local PostgreSQL server. Set `DATABASE_URL` in `.env` (see `.env.example`).

```bash
diesel setup                              # Initialize database
diesel migration run                      # Apply migrations
diesel migration generate <name>          # Create new migration (updates schema.rs)
```

## Environment Variables

Configured via `.env` file:
- `DATABASE_URL` — Postgres connection string
- `TRACING_LEVEL` — Log verbosity (debug, info, warn, error, trace)
- `LOG_FILE_PATH` — File log output path (default: `./logs/sd.log`)

## Architecture

### Processing Pipeline (`src/file_proc/`)

1. **Scan** (`scan.rs`) — Parallel directory traversal via rayon. Builds `file_size → Vec<PathBuf>` map, filtering by glob ignore patterns from `Config.toml`. Skips symlinks and 0-byte files.
2. **Hash** (`hash/`) — Two-tier: partial 1KB XxHash64, then full hash only on partial matches. RocksDB cache (`hash_cache.rs`) keyed on `canonical_path|modified_timestamp` avoids re-hashing. Parallel via rayon (`hash/builders/rayon.rs`).
3. **FileInfo** (`file_info.rs`) — Extracts metadata (path, size, modified time, drive letter, parent dir) in parallel.
4. **DB Write** (`db/dupe_file.rs`) — Batch inserts into `dupe_file` table via Diesel.

### Path Part Building (`src/db/part_path.rs`)

Post-processing step that builds a hierarchical tree of path components in a `path_part` table with parent-child relationships and aggregated sizes. Exports to CSV.

### Key Modules

- `src/cli.rs` — clap derive-based CLI definition
- `src/app_config.rs` — Loads `Config.toml` (root_paths, ignore_patterns)
- `src/db/schema.rs` — Auto-generated Diesel schema (do not edit manually)
- `src/db/sd_pg.rs` — DB connection establishment and truncate operations
- `src/logging.rs` — Dual output tracing setup (stdout + file)

### Concurrency Model

Uses `rayon` for data parallelism (par_iter, par_bridge) and `DashMap` for lock-free concurrent hash maps. No async runtime (no tokio).

## Configuration

`Config.toml` defines scan targets and ignore patterns:
```toml
root_paths = ["../test-data/folder1", "../test-data/folder2"]
ignore_patterns = ["**/node_modules/**", "*/$RECYCLE.BIN"]
```

## Platform Notes

Has Windows-specific code (`src/file_proc/win/`) for drive letter extraction. On Unix, drive letter fields default to empty/none.
