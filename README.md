# Super Duper

A high-performance duplicate file detector written in Rust. Super Duper scans large file
collections, confirms duplicates by content rather than filename, identifies near-duplicate
directory trees, and stages reviewed deletion plans locally.

The repository contains the Rust engine, CLI, reusable FFI boundary, versioned worker process, and
the Windows 11 x64 WPF MVP.

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
- Windows 11 WPF front end with durable sessions/runs, cancellation, run-owned cursor paging,
  duplicate-file and exact-folder browsing, and Explorer reveal

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
    windows-build.md
    windows-smoke.md
    windows-recovery.md
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
| Windows SDK | A Windows 11 SDK capable of targeting `10.0.22000.0` |

### Build The Windows Application

Open PowerShell in the repository root (the directory containing `Cargo.toml`) and build Rust
before .NET. The WPF project copies the worker for the selected configuration beside the Windows
executable.

```powershell
# Debug engine, worker, Windows application, and tests
cargo build --workspace
cargo test --workspace
dotnet build apps/windows/SuperDuper.Windows.sln
dotnet test apps/windows/SuperDuper.Windows.sln

# Release engine, worker, Windows application, and tests
cargo build --workspace --release
dotnet build apps/windows/SuperDuper.Windows.sln --configuration Release
dotnet test apps/windows/SuperDuper.Windows.sln --configuration Release
```

The Debug app uses `target/debug/super-duper-worker.exe`; the Release app uses
`target/release/super-duper-worker.exe`. If the worker has not been built first, the WPF build or
startup will report the missing worker rather than silently using a stale binary.

### Test

```bash
cargo test --workspace
cargo test --workspace --release
dotnet test apps/windows/SuperDuper.Windows.sln
dotnet test apps/windows/SuperDuper.Windows.sln --configuration Release
```

### Run A Debug Build

From the repository root:

```powershell
cargo build -p super-duper-worker
dotnet run --project apps/windows/src/SuperDuper.Windows/SuperDuper.Windows.csproj
```

`dotnet run` builds and starts the Debug WPF application. Keep the terminal open while developing;
closing the WPF window shuts down its privately owned worker.

After `dotnet build`, the Debug executable can also be started directly:

```powershell
$debug = Resolve-Path 'apps/windows/src/SuperDuper.Windows/bin/Debug/net10.0-windows10.0.22000.0/win-x64'
Start-Process -FilePath (Join-Path $debug 'SuperDuper.Windows.exe') -WorkingDirectory $debug
```

### Build And Run The Verified Release

Run the release verifier on an interactive Windows 11 x64 desktop. Do not use `-SkipWpfSmoke` for
a release candidate.

```powershell
./scripts/Verify-WindowsRelease.ps1
```

The verifier runs the Rust and .NET Release tests, creates a framework-dependent `win-x64`
publish, places the matching Release worker beside the app, and runs the real worker/WPF smoke
workflow. After it passes, start the published application with:

```powershell
$publish = Resolve-Path 'artifacts/windows-x64'
Start-Process -FilePath (Join-Path $publish 'SuperDuper.Windows.exe') -WorkingDirectory $publish
```

The machine must have the .NET 10 Desktop Runtime installed because the publish is
framework-dependent.

### Runtime State And Overrides

By default, the worker stores `super_duper.db` beside the worker and creates
`content_hash_cache.db` relative to its working directory. The selected locations must be
writable. The app looks for `super-duper-worker.exe` beside its executable and then in the
repository's `target/debug` directory during development.

These optional environment variables override those locations:

- `SUPER_DUPER_WORKER_PATH`: absolute path to `super-duper-worker.exe`
- `SUPER_DUPER_DB_PATH`: absolute path to the worker-owned SQLite database
- `HASH_CACHE_PATH`: path to the RocksDB content-hash cache directory

PowerShell environment variables are inherited by applications started from that terminal. Clear
old overrides before a normal launch if they refer to deleted disposable state:

```powershell
Remove-Item Env:SUPER_DUPER_WORKER_PATH -ErrorAction SilentlyContinue
Remove-Item Env:SUPER_DUPER_DB_PATH -ErrorAction SilentlyContinue
Remove-Item Env:HASH_CACHE_PATH -ErrorAction SilentlyContinue
```

For an isolated disposable run, pass explicit state only to the new process:

```powershell
$publish = (Resolve-Path 'artifacts/windows-x64').Path
$state = Join-Path ([IO.Path]::GetTempPath()) ('super-duper-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $state | Out-Null

$start = [Diagnostics.ProcessStartInfo]::new()
$start.FileName = Join-Path $publish 'SuperDuper.Windows.exe'
$start.WorkingDirectory = $publish
$start.UseShellExecute = $false
$start.Environment['SUPER_DUPER_DB_PATH'] = Join-Path $state 'super_duper.db'
$start.Environment['HASH_CACHE_PATH'] = Join-Path $state 'hash-cache'
$start.Environment.Remove('SUPER_DUPER_WORKER_PATH')
[Diagnostics.Process]::Start($start)
```

Do not point these overrides at real user data when running smoke or fault-injection workflows.

Run `./scripts/Invoke-WindowsSmoke.ps1` for the repeatable worker/WPF smoke fixture. See
[`docs/windows-smoke.md`](docs/windows-smoke.md) and
[`docs/windows-recovery.md`](docs/windows-recovery.md) for diagnostics, limitations, and recovery.

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
| `SUPER_DUPER_DB_PATH` | `super_duper.db` beside worker | Worker-owned SQLite database override |
| `SUPER_DUPER_LOG` | `super_duper_core=info,super_duper_worker=info` | Worker stderr tracing filter |
| `SUPER_DUPER_WORKER_PATH` | Auto-detected | Absolute Windows worker executable override |

## Database

Super Duper uses embedded SQLite (`super_duper.db` in the working directory). New databases use
schema version 4. Version 2 and 3 databases are upgraded transactionally and in place; unknown older
schemas and databases created by a newer engine are rejected without modification. See
[`docs/storage-schema-v4.md`](docs/storage-schema-v4.md) for lifecycle and migration details.

Key tables:

| Table | Purpose |
|---|---|
| `scan_session` | Named, editable scan definitions |
| `scan_run` | Immutable executions, parameter snapshots, lifecycle, and counters |
| `run_exclusion` | Run-owned cloud/manual subtree exclusions recorded before content access |
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

The Rust core and CLI are functional, and Windows MVP Milestones 0–6 are implemented. The bounded
release-acceptance remediation for immediate rerun, native Explorer reveal, deterministic
shutdown, unexpected-worker recovery, sorting, and accessibility is code complete and verified.
See
[`docs/windows-release-acceptance-remediation-plan.md`](docs/windows-release-acceptance-remediation-plan.md)
for the acceptance scope. The Windows surface exposes no scanned-file deletion operation.
