# Roadmap

This roadmap tracks work for the Rust engine, CLI, reusable FFI surface, and clean-slate Windows
MVP. The previous Windows app implementation was removed before the current WPF app was built.

## Current State

- Core duplicate detection pipeline is functional
- CLI supports processing, directory analysis, hash-cache inspection, config printing, and database
  truncation
- SQLite schema v3 separates editable named sessions from immutable runs, owns file/group/directory
  results by run, and persists lifecycle outcomes and scan counters
- FFI exposes handles, progress callbacks, paginated queries, and deletion actions for future native
  clients

## Now - Safety And Correctness

### 1. Safer Deletion

`execute_deletion_plan()` currently removes files directly. Add a recoverable deletion path, likely
via the `trash` crate, and keep permanent deletion as an explicit opt-in.

Files: `crates/super-duper-core/src/analysis/deletion_plan.rs`,
`crates/super-duper-core/src/platform/windows.rs`

### 2. Shared Bytes Accuracy

The directory similarity shared-bytes estimate should consult actual file sizes rather than count
shared hashes. All required data already exists in SQLite.

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

### 11. New Windows App (Acceptance Remediation)

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
Final operator acceptance found bounded release blockers; the code-complete remediation plan is
[`docs/windows-release-acceptance-remediation-plan.md`](docs/windows-release-acceptance-remediation-plan.md).
Post-MVP safe deletion review, distinct similar-folder presentation, bounded thumbnails,
packaging/shell integration, and export remain deferred until that plan passes.

The detailed post-MVP duplicate-review, cloud-safety, deletion, live-reconciliation, and Activity
roadmap is maintained in
[`docs/windows-post-mvp-ux-plan.md`](docs/windows-post-mvp-ux-plan.md). Its Milestones 7-14 are
planning targets and remain gated on completion of the release-acceptance remediation plan.
