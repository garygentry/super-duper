# Roadmap

This roadmap tracks work for the Rust engine, CLI, reusable FFI surface, and clean-slate Windows
MVP. The previous Windows app implementation was removed before the current WPF app was built.

## Current State

- Core duplicate detection pipeline is functional
- CLI supports processing, directory analysis, hash-cache inspection, config printing, and database
  truncation
- SQLite schema v15 separates editable named sessions from immutable runs, owns file/group/directory
  results by run, persists lifecycle outcomes and scan counters, snapshots each run's cloud-
  exclusion policy, and stores snapshot-backed manual review plans, preference rules, live
  validation, preflight and recycle-operation state independently from deletion
- FFI exposes handles, progress callbacks, paginated queries, and deletion actions for future native
  clients
- The WPF Windows app talks to the engine through the `super-duper-worker` JSONL process and is
  review-only: production Recycle Bin execution is disabled
- The Rust workspace is edition 2024 and requires stable 1.98 or newer

## Active Roadmap Streams

| Stream | Scheduling state | Authority | Next boundary |
|---|---|---|---|
| Windows UI redesign | Complete at UIR-09, plus the P00–P08 usability/visual-polish stream; merged into `master` on 2026-09-18 as a fast-forward ([#1](https://github.com/garygentry/super-duper/pull/1)) | [`plans/ui-redesign/README.md`](plans/ui-redesign/README.md), [`plans/ui-redesign/execution-plan.md`](plans/ui-redesign/execution-plan.md), [`UIR-09 evidence`](plans/ui-redesign/evidence/uir-09-final-acceptance.md), and [`plans/ui-polish/session-checkpoint.md`](plans/ui-polish/session-checkpoint.md) | No redesign gate remains. NVDA and physical 200% remain unavailable/unrun, not passed or waived. Non-blocking follow-ups are listed under "Post-Merge Follow-Ups" below; preserve `wpf-poc` at `deefa40`, production deletion locks, parked release-validation authority and consumed campaigns. |
| Large-drive scan optimization and observability | Complete at SOP10 with physical campaign `sop10-physical-v1` accepted as `accepted_with_observation_limit` | [`docs/scan-optimization-plan.md`](docs/scan-optimization-plan.md) and [`docs/sop10-physical-acceptance-checklist.md`](docs/sop10-physical-acceptance-checklist.md) | No package remains. Do not rerun the consumed SOP10 or SOP9 identities. The Windows stream remains parked until separately authorized. |
| Windows post-MVP release validation | Parked with its finite closure ledger intact | [`docs/windows-roadmap-closure-ledger.md`](docs/windows-roadmap-closure-ledger.md), [`docs/windows-post-mvp-ux-plan.md`](docs/windows-post-mvp-ux-plan.md), and [`docs/windows-release-validation-kickoff-prompt.md`](docs/windows-release-validation-kickoff-prompt.md) | Resume at `WPM8-high-contrast` only after SOP10 reaches its documented boundary and the operator explicitly authorizes one qualifying physical high-contrast pass. Production Recycle Bin execution remains disabled. |

The shared startup checkpoint is
[`docs/windows-roadmap-session-handoff.md`](docs/windows-roadmap-session-handoff.md). Work advances
through finite named gates or coherent gate groups, with bounded commits and gate-specific authority.
Rescheduling the scan stream changes work selection, not retained SOP9 evidence, consumed campaign
identities, safety boundaries, production locks, or the parked Windows ledger.

## First Release (v0.1.0)

Operator decisions, 2026-09-18:

- Ship a self-contained win-x64 build as a zip (no .NET runtime prerequisite, no installer).
- Unsigned; the release notes explain the SmartScreen "unknown publisher" prompt.
- The parked release-validation ledger stays parked and out of scope. High contrast, Narrator/NVDA
  and multi-monitor DPI are listed as unverified, not passed.
- Deletion stays disabled: the app ships review-only.

Release work, in order: gate development overrides in Release builds; audit what Release still
honors; failure-mode checks against a large disposable fixture; the post-merge follow-ups below;
then release hygiene (version metadata, CHANGELOG, license and third-party notices, a release
checklist that names CI versus VM gates) and a full `Verify-WindowsRelease.ps1` run.

Done:

- `SUPER_DUPER_WORKER_PATH` and the repository `target/debug` fallback are honored only in Debug
  builds; Release launches only the worker beside the app (`WorkerExecutableLocator`).
- Release audit: evidence hooks (`SUPER_DUPER_SOP*`, WPM13/UIR05C) exist only in ignored tests and
  test fixtures, and `.uidev` is Debug-only. Release still honors the state-location overrides
  (`SUPER_DUPER_DB_PATH`, `SUPER_DUPER_STATUS_DB_PATH`, `HASH_CACHE_PATH`), `SUPER_DUPER_LOG`, and
  `SUPER_DUPER_DISABLE_CLOUD_REGISTRATION_DISCOVERY` (fail-closed: scans cannot start). These are
  kept deliberately.
- App state defaults to `%LOCALAPPDATA%\SuperDuper` instead of the install folder
  (`WorkerStateLocations`).
- One owner per database (F2) and named database failures (F3): the worker holds
  `<database>.lock`, answers `database_unavailable` with a reason, rejects newer schemas before any
  pragma, and the app is single-instance per state folder.
- Bounded root probes (F6): a switched-off network share blocked the worker's request loop for the
  TCP connect timeout, about 10.7 s per `session.create`/`update` and again at `run.start` (21.4 s
  measured). Probes now run in parallel with a 3 s total deadline (6.3 s for both requests). A
  nonexistent server name was never slow (about 1.5 s).
- Cancelled, failed and interrupted runs keep the phase they reached (F1); only `completed` records
  `finalizing`. Both `terminal_run` and startup reconciliation used to overwrite it.
- A root that does not exist is no longer reported as lost watcher coverage (F5), which had made
  it look dirty with "Watcher coverage overflowed … reconciliation required".
- Paths are shown and copied in plain form (F4): `DisplayPaths.Plain` and `PlainPathConverter`
  cover every results, Setup, history, preflight and preference surface, and Copy path copies the
  plain spelling. Stored paths stay verbatim, and exact-path search accepts either spelling.
- Setup-dirty prompt (operator decision, 2026-09-18): a difference between this PC's registered
  cloud locations and the saved definition is no longer an edit, so switching saved scans does not
  ask to save. Setup notes the change, Save stays available, and starting a scan re-detects and
  saves the current list as before (`HasUnsavedCloudDetection`).
- README editorial pass: states that the app is review-only, describes the current state locations,
  env vars and test commands (`-m:1`), and replaces the MVP-era status with the v0.1.0 status and
  its unverified accessibility checks.
- `resolver = "3"` (edition 2024 default). The resolved dependency graph and features were
  identical to `"2"` (`cargo tree`, 233 package/feature lines) and `Cargo.lock` was unchanged.
- Exclusions written with 8.3 short names (`C:\Users\RUNNER~1\...`) now prune the canonical walk:
  such an exclusion is also matched by its `GetLongPathNameW` spelling. Only paths with a `~`
  component are looked up, so ordinary cloud exclusions are never touched on disk.
- Release hygiene: MIT `LICENSE`, one version (0.1.0) with license and product metadata in Cargo
  and `Directory.Build.props`, `CHANGELOG.md`, generated `THIRD-PARTY-NOTICES.txt`
  (`scripts/New-ThirdPartyNotices.ps1`: cargo-about for crates, deps.json for .NET, bundled native
  library licenses), and `Verify-WindowsRelease.ps1` producing the self-contained zip after smoking
  the published app. `docs/release-checklist.md` names CI versus VM gates.

Failure-mode pass, 2026-09-18 (Release app, 60,000-file disposable fixture). Degraded correctly:
worker killed mid-scan, corrupt/truncated/newer/read-only main database (file never modified),
corrupt status database, held hash-cache `LOCK`, long paths, a missing root, closing during a scan.
Its six findings (F1–F6) are fixed above.

Not run: full disk (needs an operator-mounted small VHDX) and an offline OneDrive root (no signed-in
account on the VM; the unavailable-detection fail-closed path is covered by the smoke).

Release candidate, 2026-09-18: `Verify-WindowsRelease.ps1` passed end to end on `master` at
`b34af74`, including the worker and WPF smoke against the published app. It produced
`super-duper-0.1.0-win-x64.zip` (79,088,178 bytes, SHA-256
`5056cd2113a61b50c9c9ff3ae107da0ce98c3390ad406509cf6736842718fa8e`). The unzipped package ran a
scan with its own worker. Tagged `v0.1.0` at `b34af74` on 2026-09-19; the GitHub release is not
yet published (`docs/release-checklist.md`).

After the database lock landed, 5 of 20 CI runs had intermittent Infrastructure failures (hello
timeouts, SQLite locked or unopenable files) that did not reproduce locally under triple load, and
four re-runs before and after the change all passed. Worker-backed test classes now run one at a
time (`[DoNotParallelize]`); watch CI for recurrences.

Those failures clustered in seconds-long windows in which fresh workers also missed their 15 s
hello, so the runner itself stalled; with a 1 ms busy timeout the whole Infrastructure suite still
passed three times on the VM, so in-worker lock contention is rare. The investigation found two
real ways a transient writer turns into "database is locked", both fixed after v0.1.0:
`Database::open_connection` rewrote `user_version` (a write) on every open, so each request and
progress save waited for the write lock; and preflight start and its survivor check read before
writing in a deferred transaction, which SQLite fails at once rather than waiting.

CI now also runs `Verify-WindowsRelease.ps1 -SkipWpfSmoke` in a parallel `release-package` job:
Release tests, self-contained publish, notices, the worker smoke against the publish, and the zip
(kept 14 days from `master` builds). Only the WPF smoke remains VM-only.

`super-duper-worker.exe` has a Windows version resource (`build.rs`, `winresource`, version from
Cargo), and `Verify-WindowsRelease.ps1` checks it against the release version.

## Post-Merge Follow-Ups

Carried over from the handoff that stabilized `codex/ui-redesign` for the merge. None blocked it.

- **Optionally surface `exact_folder_hash_cache_warning`** (after v0.1.0; verified, but the hash cache
  degraded). It currently renders as a plain aggregate row; only `hash_recoverable_warning` gets
  navigation affordances.
- **Exclusions spelled through a junction or `subst` drive** still do not match the canonical walk.
  8.3 short names are handled (see Done); other aliases would need the excluded path to be opened,
  which the cloud-safety boundary avoids. The app supplies canonical long paths today.
- **Run `scripts/Verify-WindowsHashReadPath.ps1` at the next SOP7 check.** Its SOP7 assertion was
  retargeted from `hasher/cache.rs` to `hasher/xxhash.rs` but has not been run.

## Now - Safety And Correctness

### 1. Safer Deletion

`execute_deletion_plan()` sends files to the Recycle Bin via the `trash` crate on Windows and falls
back to permanent deletion elsewhere. Since `ae3658d` it also re-validates each target's identity,
size, timestamp and content hash immediately before removal and requires every duplicate group to
retain a verified, unplanned survivor. What remains: make the non-Windows fallback an explicit
opt-in rather than automatic, and give the CLI a way to choose recoverable versus permanent.

Files: `crates/super-duper-core/src/analysis/deletion_plan.rs`,
`crates/super-duper-core/src/platform/windows.rs`

### 2. Shared Bytes Accuracy

Done in `d083fef`: directory similarity sums the sizes of the shared contents rather than counting
shared hashes, now pinned by `test_similarity_shared_bytes_sums_shared_file_sizes`. Directory
similarity is used by the CLI (`analyze-directories`) and the FFI, not the Windows app.

Files: `crates/super-duper-core/src/analysis/dir_similarity.rs`,
`crates/super-duper-core/src/storage/queries.rs`

### 3. CLI Deletion Command

Add a `delete` subcommand with `--dry-run`, terminal confirmation, and clear reporting before
executing a reviewed deletion plan.

Files: `crates/super-duper-cli/src/commands.rs`, `crates/super-duper-cli/src/main.rs`

## Soon - Workflow And Automation

### 4. Export Results

Add an `export` subcommand for duplicate groups and selected session data in CSV or JSON.

Files: `crates/super-duper-cli/src/commands.rs`, `crates/super-duper-cli/src/main.rs`,
`crates/super-duper-core/src/storage/queries.rs`

### 5. Auto-Mark Strategies

Replace the hardcoded keep-first behavior with explicit strategies such as newest, oldest, and
preferred path prefix.

Files: `crates/super-duper-core/src/analysis/deletion_plan.rs`,
`crates/super-duper-ffi/src/actions.rs`

### 6. Configurable Similarity Thresholds

Move the Jaccard threshold and noise cutoff into `AppConfig`.

Files: `crates/super-duper-core/src/config.rs`,
`crates/super-duper-core/src/analysis/dir_similarity.rs`

### 7. Structured CLI Output

Add `--format json` to machine-consumable commands so scans can be scripted reliably.

Files: `crates/super-duper-cli/src/commands.rs`, `crates/super-duper-cli/src/main.rs`

## Later - Scale And Native Clients

### 8. Hash Cache Eviction

Add a cache trim operation so stale entries do not grow forever.

Files: `crates/super-duper-core/src/hasher/cache.rs`,
`crates/super-duper-ffi/src/actions.rs`

### 9. Incremental Scan

Record directory modification times and skip unchanged subtrees during repeat scans.

Files: `crates/super-duper-core/src/storage/schema.sql`,
`crates/super-duper-core/src/storage/queries.rs`,
`crates/super-duper-core/src/scanner/walk.rs`

### 10. Async Or Cancellable FFI Scan

Improve the FFI scan lifecycle so native clients can start, cancel, and observe scans without
blocking their UI thread.

Files: `crates/super-duper-ffi/src/actions.rs`, `crates/super-duper-ffi/src/callbacks.rs`

### 11. New Windows App

The Windows app is implemented against the Rust worker-process boundary as a new product surface,
not a continuation of the deleted app. Post-MVP work is ordered below rather than tracked as an
unfinished application build.

Files: `apps/windows/`, `crates/super-duper-worker/`

The approved WPF/.NET 10 MVP architecture, scope, milestones, and acceptance criteria are documented
in [`docs/windows-mvp-plan.md`](docs/windows-mvp-plan.md). The MVP uses a Rust worker-process boundary
rather than consuming the current synchronous FFI scan API.

Milestones 0 (worker/WPF shell), 1 (session/run persistence repair), 2 (worker scan lifecycle), 3
(session navigation, editing, history, and progress/cancellation UI), 4 (server-paged
duplicate-file results, bounded caching, and Explorer integration), 5 (verified exact-folder
results), and 6 (filesystem, diagnostics, smoke, Release, and recovery hardening) are implemented.
Final operator acceptance found bounded release blockers, preserved in
[`docs/windows-release-acceptance-remediation-plan.md`](docs/windows-release-acceptance-remediation-plan.md);
commit `6f1c405` fixed them and closed the full code-complete gate. Post-MVP work is therefore no
longer gated on release remediation.

The detailed post-MVP duplicate-review, cloud-safety, deletion, live-reconciliation, and Activity
roadmap is the active release-validation plan in
[`docs/windows-post-mvp-ux-plan.md`](docs/windows-post-mvp-ux-plan.md), with finite gate state in the
[`Windows roadmap closure ledger`](docs/windows-roadmap-closure-ledger.md). The required Milestone 7
surface is the accepted fail-closed `exclude_registered_roots` policy;
`include_sync_roots_skip_placeholders` and `allow_cloud_access` are unavailable reviewed follow-ons.
The read-only Milestone 8 foundation and representative 100,000-group warm-query performance gate
are accepted; three physical accessibility gates remain tracked there. All four Milestone 9
criteria are accepted: bounded side-by-side exact-folder
relationship cards, responsive single-folder Explorer reveal, bounded current-page parent-grouped
Explorer selection, and physical-file de-duplication. All four Milestone 10 review/rule slices are
accepted, and the first bounded
Milestone 11 non-deleting preflight slice is accepted. The first Milestone 12 live-state gate is
also accepted: bounded selected-set/visible-page metadata validation invalidates working review
choices after external deletion or modification while preserving immutable scan and recorded-decision
history and excluding placeholders before access. Durable schema-v13 watcher-overflow state now
marks affected immutable selected roots visibly dirty across restart and advances explicit server-
cursor reconciliation in batches of at most 200 duplicate members without binding full results.
A single 100 ms Infrastructure coalescer now collapses watcher bursts into read-only worker hints of
at most 200 distinct paths, producing at most ten Core/WPF updates per second and routing capacity or
watcher failure to the durable dirty-root fallback. Schema v14 now accounts for every persisted run
warning through immutable bounded aggregates with at most three examples, opaque sort-bound worker
paging, and an accessible cancellable Run-history drilldown that reconstructs after restart. A
retained 100,000-aggregate Release fixture accepts the unchanged query/memory guards while Core
caches only five pages and WPF binds only the current virtualized page. The existing
`scan/hash_recoverable_warning` family now resolves its stable run ID before opening that completed
run's immutable duplicate-file set, with cancellable stale-context rejection and actionable missing-
target feedback. In-app outcome reconciliation and historical
cross-overlay closure remain separate gates. Milestone 11's
revision-bound Recycle Bin operation contract, strictly non-mutating durable foundation, separately
gated native executor, and acceptance evidence tooling are implemented.

The reviewed Windows post-MVP completion contract requires every required workflow to be operator
accepted and production enabled; `code complete` is interim only. Required Milestone 14 closure
scope is keyboard/accessibility completion, coherent empty/stale/dirty/unavailable/invalidated/
resolved/partial states, query instrumentation, retained Release large-result/large-operation/
large-Activity verification, and end-to-end cloud safety. Saved filters/preferred-location
profiles, export, run-to-run deltas, and cache-only Shell thumbnails are reviewed deferred
follow-ons.

Production Recycle Bin execution remains disabled while provider, physical-accessibility,
representative-performance, constants, and residual-TOCTOU evidence gates remain open. The accepted
development-host controlled ambiguous-start campaign proves restart reconstruction and the complete
Option A checklist without retry, inference, or source-evidence mutation. The accepted schema-v11
recovery-review workflow now provides bounded WPF
unknown-item and append-only observation-history paging, all five manual observations, explicit
supersession, evidence/path copy, Recycle Bin navigation, and fresh-scan navigation while preserving
original unknown evidence; live-state inference, replay, and outcome overwrite remain prohibited.
The completion contract does not authorize `WPM11-production-wiring`: after every dependency is
accepted, that gate still requires separate explicit product/safety approval. Until then,
`RecycleOperationViewModel.CanSubmit` remains false, production uses
`DisabledRecycleOperationCapabilityExecutor`, every worker response reports
`executorEnabled:false`, and no **Move to Recycle Bin now** action is exposed. Read-only recovery
handoff includes path-free durable identifiers, signatures, lifecycle times, aggregate outcomes,
and stored error codes. Recovery-required reconstruction pages only the stored unknown results for
operator triage, reports and announces exact reviewed ranges, and exposes their durable item/batch/
source/result correlation, but does not inspect or resolve ambiguous items.
