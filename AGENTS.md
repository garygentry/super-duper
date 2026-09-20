# AGENTS.md

Shared guidance for coding agents (Codex, Claude Code, and others) working in this repository.
`CLAUDE.md` imports this file; keep repository-wide guidance here so the two cannot drift.

## What This Project Is

Super Duper is a duplicate file detector. A Rust workspace owns all product logic — scanning,
hashing, duplicate and exact-folder analysis, SQLite storage, review plans, and deletion planning.
Three front ends sit on top of it:

- `super-duper-cli` drives the engine directly for headless scans.
- `super-duper-worker` is a long-lived JSONL child process that the Windows app talks to.
- `super-duper-ffi` is a UI-agnostic C ABI for future native clients (not used by the Windows app).

The Windows app under `apps/windows` is a WPF/.NET 10 application for Windows 11 x64. It is a
clean-slate product surface over the worker boundary, not a continuation of the Windows app that was
deleted earlier (`ui/windows`); do not reintroduce that structure.

## Repository Layout

```text
super-duper/
  Cargo.toml, Cargo.lock, Config.toml, rust-toolchain.toml, global.json
  crates/
    super-duper-core/     # scanner, hasher, analysis, storage (SQLite), telemetry
    super-duper-cli/      # headless CLI
    super-duper-ffi/      # C ABI + generated super_duper.h
    super-duper-worker/   # JSONL worker process for the Windows app
  apps/windows/
    SuperDuper.Windows.sln
    src/SuperDuper.Windows/                 # WPF executable: XAML views, WPF services, App/MainWindow
    src/SuperDuper.Windows.Core/            # view models, service/worker contracts, validation (net10.0)
    src/SuperDuper.Windows.Infrastructure/  # worker client, JSONL protocol, Shell/Recycle Bin/Explorer interop
    tests/  Core.Tests, Infrastructure.Tests, Smoke.Tests (MSTest; Smoke drives real WPF on an STA thread)
  scripts/        # smoke, UI-dev launch, release verification, third-party notices, icon generation
  docs/           # index in docs/README.md; user-guide/, architecture/ (incl. decisions/), build,
                  # testing, smoke, recovery, release, protocol, progress, schema notes; docplan.json
```

Rust-specific module detail lives in `crates/CLAUDE.md`. Windows app structure, runtime behavior,
conventions and decisions are in `docs/architecture/`; start with
`docs/architecture/windows-app-components.md`.

## Build And Test

Build Rust before .NET: the WPF project copies `target/<profile>/super-duper-worker.exe` beside the
app, and .NET integration tests launch that worker.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo build --workspace
dotnet build apps/windows/SuperDuper.Windows.sln
dotnet test apps/windows/SuperDuper.Windows.sln -m:1
```

- `cargo build` is what produces `super-duper-worker.exe`; `clippy` and `cargo test` do not. After a
  `cargo clean`, skipping it makes nine worker-backed Infrastructure tests report Inconclusive
  instead of running. The WPF build also copies the worker only if it exists, so a stale copy
  already in `bin/` can be used silently.
- Keep the workspace rustfmt-clean and clippy-clean; both are clean today, so any new finding is
  yours. Silence a lint only with a scoped `#[allow]` and a reason.
- Run the .NET test projects serially (`-m:1`). Running the WPF STA smoke suite concurrently with
  the Infrastructure tests can starve dispatcher startup and produce false UI timeouts.
- Release: add `--release` / `--configuration Release`. Release .NET tests select the Release worker.
- Toolchains: Rust stable, edition 2024, 1.98 or newer (`[workspace.package] rust-version`; run
  `rustup update stable` if a build reports an older toolchain), .NET SDK pinned by `global.json`
  (10.0.400), Windows 11 SDK `10.0.22000.0`, VS C++ build tools, and VS Clang (`LIBCLANG_PATH`) for
  RocksDB bindgen. The `scripts/*.ps1` workflows are written for PowerShell 7 (`pwsh`), not Windows
  PowerShell 5.1: they use .NET APIs 5.1 lacks, and `Verify-WindowsRelease.ps1` checks `$IsWindows`.
- Core, Infrastructure and Smoke.Tests all use the MSTest 4 meta-package without
  `Microsoft.NET.Test.Sdk`. Test projects, opt-in categories and test-only variables are in
  `docs/windows-testing.md`.
- `[profile.dev] debug = "line-tables-only"` keeps debug builds near 7 GB instead of ~63 GB of
  PDBs, which previously filled the disk and hit the linker's `LNK1140` limit. Backtraces keep file
  and line numbers. Override locally rather than reverting the default, and reclaim space with
  `cargo clean --profile dev` — disk pressure from `target/` is the recurring build failure here.
- Run the app: `dotnet run --project apps/windows/src/SuperDuper.Windows/SuperDuper.Windows.csproj`.
- CLI: `cargo run -p super-duper-cli -- process|analyze-directories|count-hash-cache|print-config|truncate-db|export`.
  The machine-readable commands (everything but `process` and `truncate-db`) take `--format json`
  for scripting; `export` additionally defaults to CSV (`docs/export-format-v1.md`).

Repeatable Windows workflows:

```powershell
./scripts/Start-WindowsUiDev.ps1 -CreateFixture   # real app + disposable state (docs/windows-ui-dev-session.md)
./scripts/Invoke-WindowsSmoke.ps1                  # worker/WPF smoke (docs/windows-smoke.md)
./scripts/Verify-WindowsRelease.ps1                # full Release matrix + publish to artifacts/windows-x64
```

See `docs/windows-build.md`, `docs/windows-smoke.md`, and `docs/windows-recovery.md`.

## Architecture Rules

- Keep `super-duper-core` UI-agnostic. Product logic belongs in core, not in the worker or C#.
- The worker protocol is `docs/worker-protocol-v1.md`. Stdout is protocol-only (one JSON object
  per LF-terminated frame, 1 MiB max); diagnostics go to stderr. Additive fields are allowed within
  v1; update the doc with any protocol change.
- Keep `super-duper-ffi` a stable, app-neutral contract. `crates/super-duper-ffi/super_duper.h` is
  generated by `build.rs` (cbindgen) and may change when the crate builds.
- Windows app layering: XAML views and WPF-specific services in `SuperDuper.Windows`; view models,
  contracts, and validation in `SuperDuper.Windows.Core` (no WPF references); process, protocol, and
  native Shell/Win32 interop in `SuperDuper.Windows.Infrastructure` (CsWin32, `NativeMethods.txt`).
  MVVM uses CommunityToolkit.Mvvm; composition uses Microsoft.Extensions.DependencyInjection in
  `App.xaml.cs`.

## Safety Invariants

- The Windows app is review-only. `App.xaml.cs` registers `DisabledRecycleOperationCapabilityExecutor`,
  so production Recycle Bin execution is disabled. Do not wire `WindowsRecycleOperationExecutor` or
  any other file-mutating path into production, and do not add deletion to the worker or UI, without
  explicit operator direction.
- Scans, reviews, and previews must not modify scanned files. Review decisions are durable
  snapshot-backed records, not filesystem actions.
- Smoke, acceptance, and fault-injection workflows must use disposable state
  (`SUPER_DUPER_DB_PATH`, `SUPER_DUPER_STATUS_DB_PATH`, `HASH_CACHE_PATH` under `artifacts/` or temp),
  never real user data.
- Runtime files (`super_duper.db` and its `.lock`, `scan_status.db`, `content_hash_cache.db`,
  `logs/`, `artifacts/`) stay out of source control.
- One worker owns a database: it holds `<database>.lock` for its lifetime and answers
  `database_unavailable` otherwise, and the app is single-instance per state folder. Do not add a
  second concurrent worker on the same database, even in scripts.

## Storage

- Main database: embedded SQLite `super_duper.db`, WAL mode. Rust owns the schema:
  `crates/super-duper-core/src/storage/schema.sql` for new databases plus in-place transactional
  migrations in `storage/sqlite.rs` (`CURRENT_SCHEMA_VERSION`, currently 15). Newer-than-supported
  databases are rejected. Any schema change needs a migration step, a bumped version, tests in
  `crates/super-duper-core/tests/storage_tests.rs`, and a `docs/storage-schema-vN.md`.
- Table families: sessions and immutable runs (`scan_session`, `scan_run`, `run_exclusion`,
  `run_warning_aggregate`); results (`scanned_file`, `duplicate_group[_member]`,
  `duplicate_folder_group[_member]`, `directory_*`); review (`review_plan`, `review_decision`,
  `review_folder_decision`, `*_command` idempotency ledgers); preference rules; live validation and
  root reconciliation (`review_live_*`); preflight and recycle operations (`preflight*`,
  `recycle_operation*`, `recovery_review_observation`); legacy `deletion_plan`.
- Scan telemetry lives in a separate worker-owned status database (`scan_status.db`,
  `telemetry/status_schema.sql`), never in the main schema.
- Content-hash cache: one RocksDB store at `HASH_CACHE_PATH` (default `content_hash_cache.db`),
  owned by `hasher/repeat_cache.rs`. RocksDB locks that directory even against a second open from
  the same process, so a scan opens it once and shares it (hashing and exact-folder verification);
  never add another open of the same path. `hasher/cache.rs` keeps only path resolution plus
  read-only counting and locked clearing for the CLI and FFI.
- Dependency pins with reasons: `bincode` stays on 2.0.1 (3.0.0 on crates.io is an empty
  placeholder) and the stored encoding uses `config::legacy()` to stay byte-identical with the 1.x
  on-disk format — `stored_encoding_bytes_are_pinned` in `hasher/repeat_cache.rs` fails if that
  changes. `resolver` is `"3"` (the edition 2024 default): `cargo update` prefers versions whose
  MSRV fits `rust-version`; the resolved graph and features matched `"2"` when it moved. RocksDB
  0.25 (bundled 11.8) is a one-way upgrade: a store it writes may not open under the previously
  bundled 8.10.

## Environment Variables

- `TRACING_LEVEL`, `LOG_FILE_PATH` — CLI logging (via `.env`; see `.env.example`).
- `HASH_CACHE_PATH` — RocksDB hash cache location.
- `SUPER_DUPER_DB_PATH`, `SUPER_DUPER_STATUS_DB_PATH` — worker database overrides. With none set,
  the app keeps all state in `%LOCALAPPDATA%\SuperDuper` (`WorkerStateLocations`); with only the
  database set, the status database and hash cache follow its folder.
- `SUPER_DUPER_LOG` — worker stderr tracing filter; `SUPER_DUPER_DIAGNOSTIC_LOG_PATH` — bounded
  diagnostic log.
- `SUPER_DUPER_WORKER_PATH` — app override for worker discovery; Debug builds only (Release launches
  only the sibling worker).
- `SUPER_DUPER_DISABLE_CLOUD_REGISTRATION_DISCOVERY` — test/diagnostic switch.
- Test-only switches: `SUPER_DUPER_EXPECTED_CLOUD_ROOT`, `SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS`,
  `SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS` with `SUPER_DUPER_RECYCLE_*`, the WPF capture
  folders `SUPER_DUPER_UIR03_CAPTURES`, `_UIR04_`, `_UIR04B_`, `_UIR05C_CAPTURES`, and the storage-test
  outputs `SUPER_DUPER_WPM13_EVIDENCE_PATH` and `SUPER_DUPER_REVIEW_PROFILE_EVIDENCE`. See
  `docs/windows-testing.md`.

Debug app builds also honor a `.uidev` sidecar next to the executable (written by
`Start-WindowsUiDev.ps1`) when no database override is set; it is compiled out of Release.

## Documentation

`docs/README.md` indexes every document by audience. `docs/docplan.json` (validated by
`docs/docplan.schema.json`) records what each document covers, its sources of truth and known gaps.

- A change to user-visible labels, messages, limits or shortcuts in the Windows app must update
  `docs/user-guide/`. The guide quotes labels exactly as displayed.
- A change to layering, the worker boundary, startup/shutdown or crosscutting rules must update
  `docs/architecture/`. Record a new significant decision as the next ADR in
  `docs/architecture/decisions/`; supersede rather than edit accepted ADRs.
- Protocol and schema changes keep their existing rules (see Architecture Rules and Storage).

## Project Status And History

v0.1.0 is released (see `CHANGELOG.md` and GitHub releases). Open work is tracked in GitHub issues;
production Recycle Bin execution remains disabled and parked (#28). Earlier plans, ledgers, handoffs,
evidence and campaign scripts were removed after v0.1.0 and remain available at the `v0.1.0` tag.
They are records, not work queues: do not replay accepted gates or infer new work from them unless
the operator reopens that scope.

The active work queue is the burn-down tracking issue
[#51](https://github.com/garygentry/super-duper/issues/51) (operator-directed, 2026-09-19): staged
sessions toward v0.2.0, with a tracker, the operator's decisions, and the commit and merge protocol
those sessions follow. Its handoff comments are the session log.

## Dedicated Windows VM

The dedicated Windows VM is where this project is developed and where anything touching the UI must
be verified. The app cannot be exercised on the operator's desktop machine, so a session there is
limited to engine, worker, documentation and other headless work, and must hand UI verification to
the VM rather than claiming or skipping it.

The operator has granted standing approval (reconfirmed 2026-09-16) for agents to use computer
control on that VM for authorized tasks: launching, capturing, and operating the app and performing
requested appearance checks. An app-approval timeout is a tool
availability failure, not missing authorization. This does not extend to merging, pushing,
releasing, deleting branches, or enabling deletion, which each still need explicit direction.

Remote desktop input may be unavailable while the operator's session is backgrounded or locked.
Continue builds, tests, isolated worker fixtures, and loaded-STA WPF captures without waiting
(`docs/windows-ui-dev-session.md`). Record any native check you could not run as unrun; background
evidence does not substitute for a required physical, provider, or release gate.
