# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Project Is

A multi-platform duplicate file detection tool built as a Cargo workspace. A **Rust core library**
handles scanning, hashing, analysis, and storage. **Native UIs** consume the library via FFI —
starting with a WinUI 3 (C#/.NET) app on Windows 11. Uses a two-tier hashing strategy: partial
hash (first 1KB with XxHash64) to quickly eliminate non-matches, then full hash only on candidates.
Hash results are cached in RocksDB. Results are stored in SQLite.

## Sub-Project Documentation

For depth on each sub-project, see the localized CLAUDE.md files — both are self-contained:

- **Rust crates (core/ffi/cli):** `crates/CLAUDE.md`
- **WinUI 3 C# app:** `ui/windows/SuperDuper/CLAUDE.md`

## Workspace Structure

```
super-duper/
  Cargo.toml                         # Workspace: super-duper-core, super-duper-ffi, super-duper-cli
  Config.toml                        # Scan targets and ignore patterns
  crates/                            # All Rust crates — see crates/CLAUDE.md
    super-duper-core/                # rlib — scanning, hashing, analysis, SQLite storage
    super-duper-ffi/                 # cdylib — C-compatible FFI producing super_duper_ffi.dll
    super-duper-cli/                 # binary — headless CLI for pipeline testing
  ui/
    windows/                         # WinUI 3 C# app — see ui/windows/SuperDuper/CLAUDE.md
      SuperDuper.sln
      SuperDuper/
        App.xaml / App.xaml.cs      # Root app; DI configured in ConfigureServices()
        MainWindow.xaml.cs          # NavigationView shell (built entirely in code-behind)
        NativeMethods/              # P/Invoke declarations + EngineWrapper managed wrapper
        ViewModels/                 # MVVM ViewModels (CommunityToolkit.Mvvm)
        Views/                      # XAML pages (Dashboard, Explorer, Groups, Directories, etc.)
        Controls/                   # UserControls (DuplicateCard, FileList, DirectoryTree, etc.)
        Services/                   # Business logic singletons (ScanService, DatabaseService, etc.)
        Styles/                     # Colors.xaml, SharedStyles.xaml
      TestHarness/                  # Console test harness for FFI
```

## Build & Run Commands

```bash
# Rust (all crates)
cargo build --workspace
cargo build --release --workspace
cargo run -p super-duper-cli -- process            # Full duplicate detection pipeline
cargo run -p super-duper-cli -- analyze-directories # Build directory fingerprints + similarity
cargo run -p super-duper-cli -- count-hash-cache   # Show RocksDB cache entry count
cargo run -p super-duper-cli -- print-config       # Print loaded configuration
cargo run -p super-duper-cli -- truncate-db        # Truncate all SQLite tables

# C# WinUI 3
cd ui/windows/SuperDuper && dotnet build
# Note: dotnet build automatically runs cargo build -p super-duper-ffi first
# (BuildNativeDll target in SuperDuper.csproj). Requires Rust on PATH.
```

## Database

Uses embedded SQLite (`super_duper.db` in the working directory). Both the Rust library and the
C# app open the same file. WAL mode (set by Rust on first open) enables concurrent reads.

**Rust writes:**

- `scan_session` — scan runs
- `scanned_file` — files with metadata + hashes
- `duplicate_group` / `duplicate_group_member` — duplicate groups
- `directory_node` / `directory_fingerprint` / `directory_similarity` — directory analysis
- `deletion_plan` — files marked for deletion

**C# writes (created by DatabaseService.EnsureSchemaAsync()):**

- `review_decisions` — per-file Keep/Delete/Skip decisions
- `undo_log` — persisted undo history
- `scan_profiles` — named scan configurations

The C# side sets `PRAGMA busy_timeout=5000` to tolerate Rust write phases.

## Environment Variables

Configured via `.env` file (Rust only):

- `TRACING_LEVEL` — Log verbosity (debug, info, warn, error, trace)
- `LOG_FILE_PATH` — File log output path (default: `./logs/sd.log`)
- `HASH_CACHE_PATH` — RocksDB cache location (default: `content_hash_cache.db`)

## Configuration

`Config.toml` defines scan targets and ignore patterns for the CLI:

```toml
root_paths = ["../test-data/folder1", "../test-data/folder2"]
ignore_patterns = ["**/node_modules/**", "*/$RECYCLE.BIN"]
```

The UI manages scan paths and patterns directly via the ScanDialog and persists them in `scan_profiles`.

## Platform Notes

Has Windows-specific Rust code (`platform/windows.rs`) for drive letter extraction,
cfg-gated with `#[cfg(target_os = "windows")]`. On Unix, drive letter fields default to empty.

The FFI crate compiles as `cdylib` producing `super_duper_ffi.dll` (Windows) or
`libsuper_duper_ffi.so` (Linux).

<!-- ralph:start -->

## Autonomous Loop (Ralph)

When running as a ralph loop iteration, follow these operational rules:

### Reading Your Task

1. Read `.ralph/RALPH.md` for detailed per-iteration instructions
2. Read `.ralph/backlog.json` — find the current `in_progress` item
3. The item's `acceptanceCriteria` define "done" for this iteration

### Working

4. Implement the changes described in the item's description
5. Follow acceptance criteria precisely — each one must pass
6. Run the verification command before considering work complete

### Completing

7. If all acceptance criteria pass: output `RALPH_DONE` as your final line
8. If blocked (missing dependency, unclear requirement): output `RALPH_BLOCKED:<reason>`
9. If human input needed (API key, design decision): output `RALPH_NEEDS_HUMAN:<reason>`
10. Commit your changes with message: `[ralph] <item-id>: <title>`

### Rules

- ONE item per iteration — do not work on multiple items
- Do not modify `.ralph/backlog.json` — the loop runner manages status
- Do not modify `.ralph/state.json` — the loop runner manages state
- Read `.ralph/progress.md` for accumulated project learnings
- Append new learnings to `.ralph/progress.md` if you discover important patterns
<!-- ralph:end -->
