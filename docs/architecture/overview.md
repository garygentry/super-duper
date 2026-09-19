# System overview

This is the container view of Super Duper: the processes and libraries that run, the stores they
own, and how they talk. It is written for maintainers deciding where a change belongs. The Windows
app's internals are in [Windows app components](windows-app-components.md).

A rendered container diagram is pending; the tables below carry the same information.

## Context

Super Duper runs entirely on one Windows 11 x64 PC for one person: the owner reviewing duplicate
files on their own drives. There is no server, account or network service; neither the app nor the
engine contains network client code, and mapped or UNC roots are reached through the file system.

| Outside party | What Super Duper does with it | Where |
|---|---|---|
| The owner | Chooses locations, starts scans, records keep/remove decisions, checks them | the Windows app |
| Local, removable and network file systems | Reads metadata and content to find exact duplicates; never deletes, moves or modifies scanned files | worker (scan, validation, whole-plan check); app (root watchers, reachability checks) |
| File Explorer | Reveals a file or selects up to 200 items grouped by parent folder | `apps/windows/src/SuperDuper.Windows.Infrastructure/WindowsExplorerService.cs` |
| Windows Cloud Files registrations | Lists registered sync roots so cloud folders can be excluded before scanning | `apps/windows/src/SuperDuper.Windows.Infrastructure/WindowsCloudLocationService.cs` |
| Recycle Bin | Opens the Recycle Bin folder for viewing only (`shell:RecycleBinFolder`) | `apps/windows/src/SuperDuper.Windows.Infrastructure/WindowsRecycleBinService.cs` |
| Windows text-size setting | Scales the app's font tokens | `apps/windows/src/SuperDuper.Windows.Infrastructure/WindowsTextScaleSource.cs` |

The app is review-only: production builds register a disabled Recycle Bin executor and expose no
action that changes scanned files. See [ADR-0002](decisions/0002-review-only-windows-app.md).

## Containers

| Container | Technology | Responsibility | Owns |
|---|---|---|---|
| Windows app (`SuperDuper.Windows.exe`) | C#, WPF, .NET 10, win-x64; three projects under `apps/windows/src/` | Screens, navigation, confirmations, Windows shell integration, watching scan roots for review hints | `presentation-preferences.json`; `logs\worker.log` (the worker's stderr, written by the app) |
| Worker (`super-duper-worker.exe`) | Rust, long-lived child process; `crates/super-duper-worker/src/` | JSONL protocol, request dispatch, scan and whole-plan-check lifecycles, events, database lock | `super_duper.db` and its `.lock`; `scan_status.db` |
| Core (`super-duper-core`) | Rust library linked into worker, CLI and FFI; `crates/super-duper-core/src/` | All product logic: scanning, hashing, duplicate and exact-folder analysis, review rules, schema and migrations, telemetry | Schema (`storage/schema.sql`, `storage/sqlite.rs`); the hash-cache store (`hasher/repeat_cache.rs`) |
| Main database (`super_duper.db`) | SQLite, WAL mode, schema version 15 | Saved scans, immutable runs and results, review decisions, preference rules, whole-plan checks, the dormant recycle ledger | Written only through core, by the one worker that holds its lock |
| Status database (`scan_status.db`) | SQLite, WAL mode; `telemetry/status_schema.sql` | Scan telemetry for progress and performance views | The worker; never part of the main schema |
| Hash cache (`content_hash_cache.db`) | RocksDB directory | Reuse of verified content hashes across repeat scans | Opened once per scan by core and shared by hashing and exact-folder verification |
| CLI (`super-duper-cli`) | Rust executable; `crates/super-duper-cli/src/` | Headless scans and maintenance driven by `Config.toml` and `.env` | Its own `super_duper.db` in the current directory |
| FFI (`super-duper-ffi`) | Rust `cdylib` plus generated `super_duper.h` | App-neutral C ABI for future native clients; not used by the Windows app | Nothing beyond what it opens through core |

The protocol and store details live in their own documents:
[worker protocol v1](../worker-protocol-v1.md),
[scan progress contract](../scan-progress-contract-v1.md),
[scan status database](../scan-status-database.md) and the `docs/storage-schema-v*.md` notes.

## How they communicate

- **App to worker.** Each app instance starts one worker as a child process and talks to it over
  UTF-8 newline-delimited JSON on stdin and stdout: requests with string IDs, correlated responses,
  and unsolicited events (`run.progress`, run lifecycle, `result.state_changed`). Every frame is at
  most 1,048,576 bytes in either direction. Stdout carries protocol frames only; diagnostics go to
  stderr. The first request is `hello`, which must select protocol version 1
  (`apps/windows/src/SuperDuper.Windows.Infrastructure/WorkerClient.cs`,
  `crates/super-duper-worker/src/lib.rs`). See [ADR-0001](decisions/0001-worker-process-boundary.md).
- **Configuration by environment.** The app tells the worker where state lives through
  `SUPER_DUPER_DB_PATH`, `SUPER_DUPER_STATUS_DB_PATH`, `HASH_CACHE_PATH` and
  `SUPER_DUPER_DIAGNOSTIC_LOG_PATH`, resolved by
  `apps/windows/src/SuperDuper.Windows.Infrastructure/WorkerStateLocations.cs`.
- **Worker to core.** Ordinary Rust calls. The worker runs scans and whole-plan checks on background
  threads so it keeps answering requests during a run.
- **One owner per database.** The worker holds an exclusive `<database>.lock` for its lifetime and
  answers every request with `database_unavailable` when it cannot; the app allows one window per
  state folder. The lock lives in the worker, not in core, so the CLI and FFI do not take it. See
  [ADR-0003](decisions/0003-one-owner-per-database.md).
- **CLI and FFI.** Both link core directly; neither uses the worker or its protocol.

## What ships

The release is one zip, `super-duper-<version>-win-x64.zip`, with a `.zip.sha256` beside it, built by
`scripts/Verify-WindowsRelease.ps1`. It holds a single top-level folder containing a self-contained
.NET publish of the app, `super-duper-worker.exe` beside it, `THIRD-PARTY-NOTICES.txt`,
`LICENSE.txt` and `CHANGELOG.md`. The script checks that both executables report the version in
`apps/windows/Directory.Build.props`, which must match the root `Cargo.toml`. Release builds launch
only the worker beside the app (`WorkerExecutableLocator.cs`). The CLI and FFI are not shipped.

At runtime, with no overrides, all state lives in `%LOCALAPPDATA%\SuperDuper`
([ADR-0005](decisions/0005-app-state-in-localappdata.md)):

| Path | Written by |
|---|---|
| `super_duper.db`, `super_duper.db.lock`, and SQLite's `-wal` and `-shm` files | worker |
| `scan_status.db` | worker |
| `content_hash_cache.db\` | worker, through core, during scans |
| `presentation-preferences.json` | app |
| `logs\worker.log`, `logs\worker.log.previous` | app, from worker stderr |

Setting `SUPER_DUPER_DB_PATH` moves the database, and the preferences file with it. The status
database and hash cache follow the database's folder unless `SUPER_DUPER_STATUS_DB_PATH` or
`HASH_CACHE_PATH` overrides them. The diagnostic log always stays under
`%LOCALAPPDATA%\SuperDuper\logs`.

## Related decisions

- [ADR-0001: The Windows app reaches the engine through a worker process](decisions/0001-worker-process-boundary.md)
- [ADR-0002: The Windows app is review-only](decisions/0002-review-only-windows-app.md)
- [ADR-0003: One worker owns a database, one window per data folder](decisions/0003-one-owner-per-database.md)
- [ADR-0005: App state lives in %LOCALAPPDATA%\SuperDuper](decisions/0005-app-state-in-localappdata.md)
