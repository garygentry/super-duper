# CLAUDE.md

This file provides guidance to coding agents working in this repository.

## What This Project Is

Super Duper is a Rust duplicate file detection workspace. The core library handles scanning,
hashing, analysis, storage, and deletion planning. The CLI drives the engine directly. The FFI crate
exists as a reusable C ABI for future native clients.

This branch intentionally contains no Windows app implementation. Keep future Windows app work in a
new app directory and avoid reintroducing assumptions from the deleted implementation.

## Workspace Structure

```text
super-duper/
  Cargo.toml
  Cargo.lock
  Config.toml
  crates/
    super-duper-core/     # business logic
    super-duper-cli/      # headless CLI
    super-duper-ffi/      # C-compatible FFI
  docs/
    architecture.svg
```

For Rust-specific details, see `crates/CLAUDE.md`.

## Build And Run Commands

```bash
cargo build --workspace
cargo build --release --workspace
cargo run -p super-duper-cli -- process
cargo run -p super-duper-cli -- analyze-directories
cargo run -p super-duper-cli -- count-hash-cache
cargo run -p super-duper-cli -- print-config
cargo run -p super-duper-cli -- truncate-db
```

## Database

The engine uses embedded SQLite (`super_duper.db` in the working directory). WAL mode is enabled on
first open. The Rust code owns the schema in
`crates/super-duper-core/src/storage/schema.sql`.

Key tables:

- `scan_session`
- `scanned_file`
- `duplicate_group`
- `duplicate_group_member`
- `directory_node`
- `directory_fingerprint`
- `directory_similarity`
- `deletion_plan`

## Environment Variables

Configured via `.env` when needed:

- `TRACING_LEVEL`
- `LOG_FILE_PATH`
- `HASH_CACHE_PATH`

## Configuration

`Config.toml` defines scan targets and ignore patterns for the CLI:

```toml
root_paths = ["../test-data/folder1", "../test-data/folder2"]
ignore_patterns = ["**/node_modules/**", "*/$RECYCLE.BIN"]
```

## Platform Notes

The core has cfg-gated Windows helpers in `platform/windows.rs` for drive-letter extraction. The FFI
crate compiles as a shared library for native clients, but no client app is part of this branch.
