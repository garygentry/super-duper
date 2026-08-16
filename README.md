# Super Duper

A high-performance duplicate file detector written in Rust. Super Duper scans large file
collections, confirms duplicates by content rather than filename, identifies near-duplicate
directory trees, and stages reviewed deletion plans locally.

The repository contains the Rust engine, CLI, reusable FFI boundary, a versioned worker process,
and the new WPF Windows application scaffold.

## Features

- Two-tier hashing: exact file size, then a 1 KB XxHash64 partial hash, then full-content hashing
  only for candidates
- Streaming full-file hashing with a bounded buffer and a RocksDB cache keyed by canonical path,
  size, and high-resolution modified timestamp
- SQLite session storage for scans, duplicate groups, directory analysis, and deletion plans
- Directory fingerprinting and Jaccard similarity for exact, subset, and near-match folder trees
- Exact duplicate-folder verification by relative structure and content, with redundant nested
  matches suppressed
- Reviewed deletion workflow: files are staged before execution
- Headless CLI for repeatable scans, scripting, and verification
- C-compatible FFI crate for future native clients

## Architecture

Super Duper is a Cargo workspace. The Rust core library owns the product logic; the CLI links it
directly, the FFI crate exposes a stable boundary for future native interfaces, and the Windows app
connects to the Rust engine through a long-lived JSONL worker process.

```text
super-duper/
  Cargo.toml
  Cargo.lock
  Config.toml
  crates/
    super-duper-core/     # scanning, hashing, analysis, storage, deletion plans
    super-duper-cli/      # headless command-line driver
    super-duper-ffi/      # C ABI for future native apps
    super-duper-worker/   # JSONL process boundary for the Windows app
  apps/
    windows/              # WPF/.NET 10 solution, application layers, and tests
  docs/
    architecture.svg
    windows-mvp-plan.md
    worker-protocol-v1.md
```

![Super Duper architecture](docs/architecture.svg)

## Getting Started

### Prerequisites

| Tool | Notes |
|---|---|
| Rust toolchain | `rustup` recommended, stable channel |
| `libclang-dev` | Required by RocksDB's bindgen step on Linux |
| .NET SDK | 10.0.303 or a compatible 10.0 patch; required for the Windows app |
| Windows | Windows 11 x64 for building and running the WPF application |

### Build

```bash
cargo build --workspace
cargo build --release --workspace

# Windows application (build the Rust workspace first so the worker is copied beside the app)
dotnet build apps/windows/SuperDuper.Windows.sln
```

### Test

```bash
cargo test --workspace
dotnet test apps/windows/SuperDuper.Windows.sln
```

### Run The Windows Application

```bash
cargo build -p super-duper-worker
dotnet run --project apps/windows/src/SuperDuper.Windows/SuperDuper.Windows.csproj
```

The application looks for `super-duper-worker.exe` beside its executable and then in the
repository's `target/debug` directory. Set `SUPER_DUPER_WORKER_PATH` to an absolute executable path
to override discovery during development.

### Configure Scan Targets

Edit `Config.toml`:

```toml
root_paths = [
    "C:/Users/you/Documents",
    "D:/Archive",
]

ignore_patterns = [
    "**/node_modules/**",
    "**/.git/**",
    "*/$RECYCLE.BIN",
]
```

### Run The CLI

```bash
# Full duplicate detection pipeline
cargo run -p super-duper-cli -- process

# Re-run directory fingerprinting and similarity analysis
cargo run -p super-duper-cli -- analyze-directories

# Inspect the persistent hash cache
cargo run -p super-duper-cli -- count-hash-cache

# Print loaded configuration
cargo run -p super-duper-cli -- print-config

# Wipe all SQLite tables with confirmation
cargo run -p super-duper-cli -- truncate-db
```

## Environment Variables

Configured via a `.env` file in the working directory when needed.

| Variable | Default | Description |
|---|---|---|
| `TRACING_LEVEL` | `info` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |
| `LOG_FILE_PATH` | `./logs/sd.log` | File log output path |
| `HASH_CACHE_PATH` | `content_hash_cache.db` | RocksDB hash cache location |

## Database

Super Duper uses embedded SQLite (`super_duper.db` in the working directory). New databases use
schema version 3. Version 2 databases are upgraded transactionally and in place; unknown older
schemas and databases created by a newer engine are rejected without modification. See
[`docs/storage-schema-v3.md`](docs/storage-schema-v3.md) for lifecycle and migration details.

Key tables:

| Table | Purpose |
|---|---|
| `scan_session` | Named, editable scan definitions |
| `scan_run` | Immutable executions, parameter snapshots, lifecycle, and counters |
| `scanned_file` | Immutable per-run file snapshots with root-relative paths |
| `duplicate_group` | Confirmed duplicate sets owned by one run |
| `duplicate_group_member` | Duplicate group membership |
| `duplicate_folder_group` | Verified exact-folder groups, including retained suppression state |
| `duplicate_folder_group_member` | Run-owned duplicate-folder roots |
| `directory_node` | Per-run directory tree aggregates |
| `directory_fingerprint` | Per-directory content fingerprints |
| `directory_similarity` | Precomputed Jaccard pairs |
| `deletion_plan` | Files staged for deletion |

## FFI Boundary

The `super-duper-ffi` crate exposes the core through a C ABI for future native clients.

- Handle-based API with opaque `u64` handles
- Rust-owned buffers paired with explicit `sd_free_*()` functions
- Thread-local error messages via `sd_last_error_message()`
- Paginated list queries for large result sets
- Progress callbacks for long-running scans

## Project Status

The Rust core and CLI are functional. Milestone 0 of the clean-slate Windows MVP provides the
WPF/.NET 10 application shell, dependency boundaries, worker protocol contract, and a health
handshake. Milestone 1 provides named session definitions, immutable run history, forward schema
migration, durable run outcomes, run-owned results, and corrected scan accounting. Milestone 2
provides the typed session/run dispatcher, one-scan coordination, ordered progress events,
concurrent cancellation, graceful worker shutdown, and streaming cache-backed hashing. Milestone 3
provides session navigation and editing, Windows folder selection, validated root/ignore settings,
run history restoration, and responsive progress/cancellation UI integration. Milestone 4 adds
run-owned cursor pagination, server-side duplicate-file sorting/filtering, a bounded WPF page cache,
master/detail results, copy-path actions, and native Explorer reveal. Milestone 5 adds structurally
screened and content-verified exact duplicate folders, nested-result suppression, run-owned folder
pagination, and a separate bounded WPF folder-results experience.
