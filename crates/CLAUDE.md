# CLAUDE.md - Rust Crates

The Rust workspace is the core of Super Duper. `super-duper-core` contains the business logic,
`super-duper-cli` is the headless driver, and `super-duper-ffi` exposes a C-compatible boundary for
future native clients.

## Build Commands

```bash
cargo build --workspace
cargo build --release --workspace
cargo run -p super-duper-cli -- process
cargo run -p super-duper-cli -- analyze-directories
cargo run -p super-duper-cli -- count-hash-cache
cargo run -p super-duper-cli -- print-config
cargo run -p super-duper-cli -- truncate-db
```

## Environment Variables

Configured via `.env` in the working directory:

- `TRACING_LEVEL` - Log verbosity (`debug`, `info`, `warn`, `error`, `trace`)
- `LOG_FILE_PATH` - File log output path, default `./logs/sd.log`
- `HASH_CACHE_PATH` - RocksDB cache location, default `content_hash_cache.db`

## Configuration

`Config.toml` in the repo root defines scan targets and ignore patterns:

```toml
root_paths = ["../test-data/folder1", "../test-data/folder2"]
ignore_patterns = ["**/node_modules/**", "*/$RECYCLE.BIN"]
```

## Workspace Structure

```text
super-duper-core/
  src/
    lib.rs
    config.rs
    engine.rs
    error.rs
    progress.rs
    scanner/walk.rs
    hasher/xxhash.rs
    hasher/cache.rs
    storage/sqlite.rs
    storage/models.rs
    storage/queries.rs
    storage/schema.sql
    analysis/dir_fingerprint.rs
    analysis/dir_similarity.rs
    analysis/deletion_plan.rs
    platform/windows.rs

super-duper-cli/
  src/
    main.rs
    commands.rs
    logging.rs
    progress.rs

super-duper-ffi/
  src/
    handle.rs
    types.rs
    callbacks.rs
    error.rs
    queries.rs
    actions.rs
  super_duper.h
  build.rs
```

## Processing Pipeline

1. Scan: `scanner/walk.rs` traverses directories in parallel and groups candidate files by size.
2. Hash: `hasher/` applies partial and full XxHash64 hashing, backed by RocksDB cache entries.
3. Store: `engine.rs` and `storage/` write sessions, files, groups, and analysis data to SQLite.
4. Analyze: `analysis/` builds directory fingerprints, directory similarity results, and deletion
   plans.

## Concurrency Model

The core uses `rayon` for data parallelism and `DashMap` for concurrent maps. It does not use an
async runtime. `ScanEngine::cancel()` sets an atomic cancel token passed through scan phases.

## FFI Layer

- `handle.rs` manages opaque `u64` handles.
- `types.rs` defines `#[repr(C)]` structs and result codes.
- `callbacks.rs` bridges progress callbacks.
- `error.rs` stores thread-local error detail.
- `queries.rs` exposes paginated read APIs.
- `actions.rs` exposes engine lifecycle, scan, cancellation, and deletion APIs.

The FFI crate should stay UI-agnostic. Future clients should consume it as a contract rather than
shape it around a particular app implementation.
