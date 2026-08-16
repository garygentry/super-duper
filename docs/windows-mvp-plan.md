# Windows Front-End MVP Plan

## Status

Approved architecture and implementation plan for the first Super Duper Windows front end.

Implementation status: Milestones 0–5 are complete on the `wpf-poc` line of development. The next
slice is Milestone 6, MVP hardening.

This plan treats the Windows application as a new product surface over the existing Rust engine. It
does not restore or reuse the deleted Windows application.

## Product Goal

Deliver a Windows 11 desktop application that lets a user:

1. Create and edit named scan sessions containing root paths and ignore settings.
2. Start one long-running duplicate scan at a time.
3. Observe durable, responsive progress and cancel an active scan.
4. Reopen completed results after restarting the application.
5. Browse duplicate files and exact duplicate folders in distinct views.
6. Sort and page through large result sets without materializing all rows in memory.
7. Reveal a selected file or folder in Windows File Explorer.

The MVP proves the Windows/Rust boundary, lifecycle, persistence, and large-result UI foundation.
It deliberately does not attempt the complete product experience.

## Fixed Decisions

The following decisions are approved for the MVP:

- UI: C#, WPF, and .NET 10 with WPF's built-in Fluent theme.
- Presentation pattern: CommunityToolkit.Mvvm with constructor-based dependency injection.
- Rust boundary: a long-lived `super-duper-worker` child process, not in-process FFI.
- Transport: versioned UTF-8 newline-delimited JSON over stdin/stdout.
- Ownership: only the Rust worker reads or writes the product SQLite database.
- Concurrency: at most one scan runs globally; other sessions remain viewable.
- Sessions: named, reusable scan definitions.
- Runs: immutable executions belonging to a session, with durable status and result ownership.
- Folder duplicates: exact matches require identical descendant relative paths, multiplicities, and
  file content; the two root folder names and locations may differ.
- Filesystem safety: skip reparse points, junctions, and symbolic links; warn and continue on access
  errors; do not count hard links to the same physical file as recoverable copies.
- Initial target: Windows 11 x64. Explicitly selected mapped or UNC paths are best-effort.
- Distribution during MVP: unpackaged developer/publish output. MSIX comes later.

## MVP Scope

### Included

- Session list, creation, editing, and deletion when no run is active.
- One or more root directories per session.
- Per-session ignore patterns, seeded with safe Windows defaults.
- Run history metadata, with the latest run emphasized in the UI.
- Start, progress, cancellation, completion, failure, and interrupted-run recovery.
- Separate Duplicate Files and Duplicate Folders result tabs.
- Server-side sort, filters, counts, and stable pagination.
- A bounded in-memory page cache for result rows.
- File/folder detail rows and copy-path commands.
- Show the selected item in File Explorer.
- Logs and actionable error/warning summaries.
- Automated Rust, protocol, C#, view-model, and end-to-end tests.

### Explicitly Deferred

- File deletion, Recycle Bin operations, and deletion strategies.
- Thumbnails, previews, and `IShellItemImageFactory`.
- Near-duplicate/similar-folder presentation and threshold controls.
- Explorer context-menu registration and its native C++ `IExplorerCommand` extension.
- MSIX packaging, sparse packages, Store distribution, and auto-update.
- Concurrent scans, pause/resume, scheduled scans, and background services.
- Automatic drive or network discovery.
- Windows 10 and ARM64 certification.
- Result export.

The existing deletion APIs remain in Rust but are not surfaced by the MVP application.

## Target Architecture

```text
SuperDuper.Windows.exe                 super-duper-worker.exe
C# / .NET 10 / WPF                    Rust

Views + controls                      command dispatcher
        |                                      |
ViewModels                             scan coordinator
        |                                      |
Application services                  core scan/hash/analysis
        |                                      |
WorkerClient <---- JSONL stdio ----> progress events
        |                                      |
Windows shell service                 SQLite + RocksDB hash cache
```

### Proposed Repository Layout

```text
super-duper/
  apps/
    windows/
      SuperDuper.Windows.sln
      src/
        SuperDuper.Windows/                 # WPF app, views, controls, resources
        SuperDuper.Windows.Core/            # application models, contracts, view models
        SuperDuper.Windows.Infrastructure/  # worker client, process lifecycle, shell APIs
      tests/
        SuperDuper.Windows.Core.Tests/
        SuperDuper.Windows.Infrastructure.Tests/
  crates/
    super-duper-core/
    super-duper-cli/
    super-duper-ffi/
    super-duper-worker/                     # new long-lived JSONL worker
  docs/
    worker-protocol-v1.md                   # protocol contract, added with the worker
    windows-mvp-plan.md
```

The WPF executable is the composition root. `Windows.Core` must not depend on WPF controls,
process APIs, SQLite, or native Windows APIs. `Windows.Infrastructure` implements its interfaces.

## Rust Work Required Before UI Integration

The current engine is functional, but its persistence model does not yet satisfy the product's
session semantics. These changes are prerequisites, not optional cleanup.

### 1. Separate Sessions From Runs

Replace the current reuse-by-path behavior with two concepts:

- `scan_session`: a user-owned named definition with current roots and ignore settings.
- `scan_run`: one immutable execution with a snapshot of the settings used for that execution.

Starting a run creates its database row before filesystem traversal begins. Every terminal path
must set the run to `completed`, `cancelled`, or `failed`. On worker startup, any run left in
`running` or `cancelling` becomes `interrupted`.

Suggested run fields:

```text
id
session_id
parameters_json
status
phase
started_at
completed_at
files_discovered
bytes_discovered
files_hashed
duplicate_file_groups
duplicate_folder_groups
wasted_bytes
warning_count
error_message
engine_version
```

Supported statuses:

```text
pending -> running -> completed
                   -> cancelling -> cancelled
                   -> failed
                   -> interrupted
```

Supported progress phases:

```text
discovering
hashing
persisting
analyzing_folders
finalizing
```

### 2. Make Results Run-Scoped

All result-producing records must belong directly or transitively to a run. A result from one run
must never change because another session or later run scanned the same path.

The schema should include run-scoped file snapshots rather than pointing old groups at a mutable
global path row. At minimum, a run file snapshot retains:

```text
run_id
canonical_path
relative path within its selected root
file name and parent directory
size
last-write timestamp
content hash when computed
stable Windows file identity when available
warning/error state when relevant
```

The RocksDB content-hash cache may remain global because it is an optimization, not result state.

Directory nodes, directory duplicate groups, and group members must also be run-scoped. Similarity
tables may be migrated now but their UI remains deferred.

### 3. Correct Scan Accounting

Persist `files_discovered` and `bytes_discovered` from traversal statistics, not the number and size
of confirmed duplicate records. Track hashing and result counts separately.

Progress writes should be throttled, for example to no more than twice per second, while worker
events may be coalesced to at most ten per second. The UI must not receive one event per file.

### 4. Make Cancellation Real

Cancellation must be callable while work is running and checked in every expensive phase:

- directory traversal;
- partial hashing;
- full hashing;
- database batches;
- exact-folder verification.

Cancellation is a normal run outcome. It must not require killing the worker. The WPF host may
terminate an unresponsive worker only as a last-resort shutdown policy after a bounded timeout.

The current synchronous FFI handle lock is not part of this design and does not need to be extended
for the MVP.

### 5. Improve Full-File Hashing

Replace whole-file buffering with streaming hashing using a reusable bounded buffer. Do not hold the
global RocksDB mutex while reading file contents. The cache operation should be:

1. Build the cache key from canonical path, size, and high-resolution modification time.
2. Lock briefly for lookup, then release.
3. Stream and hash the file on a miss.
4. Lock briefly to store the computed hash.

The cache remains an optimization. A cache failure should be reported but should not corrupt a run.

## Exact Duplicate Folder Semantics

Two folders are exact duplicates when all of the following are true:

- Their root names and absolute locations are ignored.
- They contain the same number of descendant regular files.
- Each descendant file has the same normalized relative path in both trees.
- Corresponding files have identical content.
- Empty-directory handling is explicit and consistent. For MVP, empty directories do not create a
  duplicate-folder result by themselves, but their presence may participate in tree structure if
  both candidate trees otherwise contain files.
- Reparse points and links are not traversed.
- Multiple directory results that merely restate a larger duplicate ancestor are suppressed by
  default. Users should see the highest useful duplicate roots, not every duplicated child folder.

### Proposed Exact-Folder Algorithm

1. During discovery, persist every regular file's root-relative path, size, and file identity.
2. Build a cheap structural fingerprint for each directory from sorted descendant relative paths
   and sizes.
3. Only directories sharing the structural fingerprint become verification candidates.
4. For each candidate set, obtain content hashes for corresponding files, using the global cache.
5. Build a verified fingerprint from each relative path plus its content hash and multiplicity.
6. Group directories sharing the verified fingerprint.
7. Remove nested groups whose members are fully covered by a larger exact duplicate group, while
   retaining the data needed for a future "show nested matches" option.

This avoids hashing every unique file solely for folder comparison while preventing the false
positives produced by a set of content hashes alone.

## Worker Protocol V1

The worker is started once with redirected stdin, stdout, and stderr. Stdout is reserved exclusively
for protocol frames. Each line is one complete UTF-8 JSON object.

### Envelope Types

```json
{"type":"request","id":"42","method":"run.start","params":{"sessionId":7}}
{"type":"response","id":"42","ok":true,"result":{"runId":19}}
{"type":"response","id":"42","ok":false,"error":{"code":"scan_busy","message":"A scan is already running"}}
{"type":"event","event":"run.progress","data":{"runId":19,"phase":"hashing","current":1200,"total":8000}}
```

The request ID is an opaque string selected by the client. Responses may arrive while events are
being emitted. Unknown fields are ignored within a major protocol version.

### Initial Commands

```text
hello
app.status
session.list
session.get
session.create
session.update
session.delete
run.list
run.get
run.start
run.cancel
duplicate_file_group.page
duplicate_file_group.members
duplicate_folder_group.page
duplicate_folder_group.members
warning.page
```

Every page query includes a run ID, page size, allowed sort field/direction, filter object, and an
opaque continuation cursor. Avoid exposing arbitrary SQL or using large numeric offsets as the
long-term paging contract.

### Initial Events

```text
worker.ready
run.started
run.progress
run.warning
run.completed
run.cancelled
run.failed
```

The protocol document must define allowed fields, error codes, maximum message size, shutdown
behavior, path encoding, protocol negotiation, and representative success/failure transcripts.

## WPF Application Design

### Application Shell

Use a single main window with session navigation and a session-specific content area:

```text
+------------------+--------------------------------------------------+
| Sessions         | Session name                         [Start scan] |
|                  +--------------------------------------------------+
| Photos           | Setup | Progress | Duplicate Files | Folders     |
| Archives         +--------------------------------------------------+
| Backups          | active tab content                               |
|                  |                                                  |
| + New session    |                                                  |
+------------------+--------------------------------------------------+
| Worker/run status and latest warning                               |
+---------------------------------------------------------------------+
```

Views and view models should be session-specific components rather than one large main-window view
model. Suggested components:

- `SessionListView` / `SessionListViewModel`
- `SessionSetupView` / `SessionSetupViewModel`
- `ScanProgressView` / `ScanProgressViewModel`
- `DuplicateFilesView` / `DuplicateFilesViewModel`
- `DuplicateFoldersView` / `DuplicateFoldersViewModel`
- `RunHistoryView` / `RunHistoryViewModel`
- shared paged-grid, empty-state, error-state, and status components

### Session Setup

- Name is required and unique case-insensitively.
- At least one existing or currently reachable root is required to start a run.
- Nested selected roots are normalized so the child is not scanned twice.
- Warn on overlapping roots, unavailable network paths, and broad drive roots.
- Use the .NET folder picker for MVP.
- Ignore patterns are editable text entries with validation before saving.

### Progress Experience

- Never block the WPF dispatcher thread on worker I/O or process exit.
- Display phase, elapsed time, files/bytes discovered, hashes completed when known, current path at a
  throttled rate, and accumulated warning count.
- Use indeterminate progress for discovery and any phase without a reliable total.
- A cancel request changes the UI state immediately to `Cancelling`; completion of cancellation is
  confirmed only by the worker event or a subsequent run query.
- Closing the window during a run prompts the user to cancel and exit or keep the application open.

### Duplicate Files View

Use a master/detail layout:

- Master `DataGrid`: group size, copy count, recoverable bytes, representative name/type.
- Detail grid: each path, modified time, size, and available action.
- Initial sort: recoverable bytes descending, then group ID for stability.
- Initial filters: text/path search and minimum size.
- Commands: copy path and show selected item in Explorer.

### Duplicate Folders View

Keep folders visually and conceptually separate from file groups:

- Master `DataGrid`: folder group, copy count, total bytes, descendant file count.
- Detail list: each duplicate root path.
- Explain that nested exact matches covered by a larger root are suppressed.
- Commands: copy path and show selected folder in Explorer.

### Large-Result Rules

- Never bind the entire result table to an `ObservableCollection`.
- Use worker-owned sorting/filtering and cursor-based pages.
- Keep a bounded cache, initially two pages before and after the visible page.
- Use WPF row and column virtualization and recycling.
- Cancel stale page requests when the session, run, sort, or filter changes.
- Apply a monotonically increasing query generation so late responses cannot replace newer results.
- Load detail rows only after group selection.

### Explorer Integration

For MVP, implement only reveal/select behavior through `SHOpenFolderAndSelectItems`, generated with
Microsoft.Windows.CsWin32. The command applies to one selected file or folder. A future multi-select
command must group items by parent directory because one Explorer selection call targets one parent.

Do not add Windows App SDK, MSIX, or the C++ shell extension until a concrete feature requires them.

## Error Handling and Recovery

- Worker startup failure produces a dedicated recovery screen with executable path and captured
  stderr, not a generic empty state.
- Malformed protocol output is a fatal worker error and is logged with sensitive path data redacted
  from telemetry. Local diagnostic logs may retain paths.
- If the worker exits, fail all pending requests, retain completed data, and offer one restart.
- On restart, reconcile database state before accepting commands and mark abandoned runs
  `interrupted`.
- Permission errors, vanished files, and transient metadata failures are warnings when the scan can
  continue.
- Database corruption, migration failure, or inability to persist a consistent run are fatal.
- No error path may silently report a partial run as completed.

## Implementation Milestones

### Milestone 0 - Contracts and Scaffold

Deliverables:

- Add `apps/windows` solution and projects.
- Add `super-duper-worker` crate.
- Write `worker-protocol-v1.md` before implementing commands.
- Add shared build instructions and pin expected .NET and Rust toolchains where practical.
- Create WPF shell using `ThemeMode="System"` and dependency injection.
- Establish Rust and .NET test commands in the repository documentation.

Exit criteria:

- WPF application launches and shows a worker-connected health state.
- `hello` negotiates protocol version 1.
- Rust and .NET test projects run from a clean checkout.

### Milestone 1 - Session/Run Persistence Repair

Deliverables:

- Introduce the session/run schema and forward migration strategy.
- Create a run before traversal.
- Scope file, duplicate-group, and directory data by run.
- Persist terminal states and reconcile interrupted runs at startup.
- Correct scan counters.

Exit criteria:

- Two runs with identical roots have different IDs and immutable independent results.
- Editing a session does not change historical run parameters.
- Cancellation/failure is durable and never appears as completion.
- Existing Rust workspace tests pass with updated semantics.

### Milestone 2 - Worker Scan Lifecycle

Deliverables:

- Implement the JSONL dispatcher, request correlation, events, and structured errors.
- Implement session commands and one-at-a-time run coordination.
- Implement real concurrent cancellation without a global state lock around the scan.
- Stream file hashing and narrow hash-cache locking.
- Log diagnostics to stderr/file while keeping stdout protocol-only.

Exit criteria:

- A test client can create a session, start a run, receive ordered phase events, cancel, and query
  the durable final status.
- A second start returns `scan_busy` without affecting the active run.
- Killing and restarting the worker marks the abandoned run interrupted.

### Milestone 3 - WPF Session and Progress Slice

Deliverables:

- Implement `WorkerClient`, worker lifecycle supervision, and typed protocol DTOs.
- Implement session list/create/edit and root selection.
- Implement start/cancel and progress UI.
- Implement application-level error and empty states.

Exit criteria:

- The complete session-to-scan workflow works without opening a terminal.
- UI remains responsive throughout scanning and cancellation.
- Closing/reopening the application restores sessions and completed/interrupted run status.

### Milestone 4 - Duplicate File Results

Deliverables:

- Add cursor-paged duplicate group and member queries with allow-listed sorts and filters.
- Add bounded paged collection and stale-request cancellation in C#.
- Implement the Duplicate Files master/detail view.
- Add copy path and Show in Explorer.

Exit criteria:

- Results are tied to the selected run.
- Sorting and filtering do not load all result rows.
- A synthetic database containing at least 100,000 groups remains responsive and memory use stays
  bounded by page/cache configuration rather than total row count.

### Milestone 5 - Exact Duplicate Folders

Status: Complete.

Deliverables:

- Implement the structural-candidate and content-verification algorithm.
- Store run-scoped folder groups and members.
- Suppress redundant nested matches.
- Implement the separate Duplicate Folders view.

Exit criteria:

- Root folder names may differ without preventing a match.
- Relative-path, multiplicity, extra-file, changed-content, link, and nested-suppression cases are
  covered by automated tests.
- Folder results never incorporate data from another run.

### Milestone 6 - MVP Hardening

Deliverables:

- Exercise fixed drives, removable drives, mapped drives, UNC paths, access errors, long paths, and
  files that change or disappear during scanning.
- Add performance instrumentation for each phase and result query.
- Add a repeatable developer smoke fixture and release build script.
- Document known limitations and recovery steps.

Exit criteria:

- All automated tests pass in release configuration on Windows 11 x64.
- A manual end-to-end scan can be created, monitored, cancelled, rerun, reopened, browsed, sorted,
  filtered, and revealed in Explorer.
- No destructive filesystem operation is exposed.

## Test Strategy

### Rust Core

- Session/run lifecycle and migration tests.
- Immutable result ownership across repeated and overlapping sessions.
- Cancellation in every pipeline phase.
- Accurate discovered/hashed/duplicate counters.
- Streaming-hash equivalence and cache hit/miss behavior.
- File identity tests preventing hard-link overcounting.
- Reparse-point and permission-warning behavior on Windows.
- Exact-folder property cases, including structure, relative names, multiplicity, content changes,
  extra files, nested roots, and suppression.

### Worker Protocol

- Golden request/response/event transcripts.
- Request correlation under interleaved progress events.
- Unknown method, invalid fields, invalid state, oversized frame, and malformed JSON handling.
- Single-scan enforcement and cancellation race tests.
- EOF, graceful shutdown, crash, restart, and interrupted-run reconciliation.
- Paging stability when sort values tie.

### C# Core and Infrastructure

- View-model state transitions using a fake worker client.
- WorkerClient framing, cancellation, response correlation, stderr capture, and process exit.
- Paged collection bounds, cache eviction, query generation, and stale-response rejection.
- Formatting and validation for paths, sizes, statuses, and filters.
- Explorer service success and failure behavior behind an interface.

### WPF Smoke Tests

- Application launch and worker connection.
- Create/edit/select session.
- Start/cancel/restart scan.
- Switch sessions while another session scans.
- Browse and sort both result tabs.
- Empty, loading, warning, failed, cancelled, interrupted, and completed states.
- Keyboard navigation, high DPI, light/dark system theme, and basic accessibility names.

## Performance Guardrails

Initial guardrails should be measured and revised from real hardware rather than treated as final
benchmarks:

- Worker progress event rate: at most 10 per second.
- Durable progress database writes: at most 2 per second per active run.
- Result page size: start at 200 rows and tune from measurements.
- WPF page cache: bounded, with no relationship to total result count.
- Result query target on a warm local database: 100 ms for typical first-page sorts/filters.
- No full-file-sized allocation during hashing.
- No filesystem, SQLite, process stream, or Shell API work on the WPF dispatcher thread.

## MVP Definition of Done

The MVP is complete when all of the following are true:

- A user can manage named sessions and their scan roots entirely in the Windows application.
- A run is durable from its start and has an accurate terminal state.
- Progress is active, meaningful, throttled, and does not freeze the UI.
- Cancellation works without terminating the UI or corrupting stored results.
- File duplicate results and exact folder duplicate results are separately queryable and displayed.
- Selecting a historical run always shows only that run's immutable results.
- Large synthetic result sets do not cause whole-dataset materialization.
- A selected result can be revealed in File Explorer.
- All Rust and .NET automated tests pass.
- The application performs no deletions and installs no Explorer extension.

## First Post-MVP Decisions

After the MVP is stable, evaluate these in order:

1. Safe deletion review and Recycle Bin integration through `IFileOperation`.
2. Similar-folder scoring and presentation distinct from exact folders.
3. Asynchronous Shell thumbnails with bounded caching.
4. MSIX identity and the minimal native `IExplorerCommand` extension.
5. Export, auto-mark strategies, and broader platform support.
