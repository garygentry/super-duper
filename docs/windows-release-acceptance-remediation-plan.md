# Windows Release Acceptance Remediation Plan

## Status

Final operator acceptance on Windows 11 x64 failed on 2026-08-16 against `wpf-poc` commit
`e977bb27169db74bd155b769bc198e0d1a2784c7`.

Milestones 0–6 remain implemented, and the complete Debug/Release Rust and .NET matrix plus
`Verify-WindowsRelease.ps1` passed on an interactive desktop. The Windows MVP is not release/code
complete until the acceptance failures in this plan are fixed, regression-tested, and the full
operator workflow passes without workarounds.

This is a bounded remediation milestone. It does not authorize post-MVP deletion, similar-folder
scoring, thumbnails, packaging, shell extensions, export, or any mutation of scanned files.

## Acceptance Baseline

The acceptance run established the following working behavior:

- session creation, scanning, progress, cancellation, durable terminal states, and restoration;
- run-scoped duplicate-file paging, sorting, filtering, and member browsing;
- exact-folder filtering and member browsing;
- fixed-drive, long-path, access-warning, changed/vanished-file, hard-link, and reparse hardening;
- all five scan-phase timings and all four result-query timings without path/filter leakage;
- keyboard navigation, system dark theme, resizing, and a non-destructive WPF surface;
- worker-startup and database-open failures that preserve the database and show recovery context;
- restart reconciliation of an abandoned run to `interrupted`, followed by a successful new run.

The following release gates failed:

| Area | Reproduced failure | Acceptance impact |
|---|---|---|
| Rerun | `Start scan` stays disabled after completion/cancellation until setup is edited or saved | Blocks the required immutable rerun workflow |
| Explorer reveal | File and folder actions return `Value does not fall within the expected range.` for `\\?\` paths | Blocks both mandatory result surfaces |
| Normal shutdown | Some normal closes leave a headless WPF process and worker | Violates bounded child-process ownership |
| Recovery-screen shutdown | Closing a startup/database failure screen throws from `MainWindow.OnClosing` | Converts a recoverable startup failure into an application crash |
| Unexpected worker exit | UI stays in `Scanning`/`Discovering`, offers only `Dismiss`, and has no restart action | Fails the documented in-app recovery contract |

Minor follow-up findings are redundant blank root editor rows, terminal `Cancelled` with a displayed
last phase of `Finalizing`, and unnamed session/grid container automation peers. They must not delay
the release blockers, but should be closed before the final accessibility spot check.

## Scope And Working Rules

- Add a failing regression test before or with each fix. Do not rely on another manual-only pass.
- Keep `SuperDuper.Windows.Core` free of WPF, process, Shell, and SQLite dependencies.
- Keep SQLite and hash-cache ownership in the Rust worker.
- Preserve immutable run ownership, cursor paging, bounded caches, and stale-query rejection.
- Keep worker stdout protocol-only; recovery/process diagnostics remain on stderr/local logs.
- Do not change the worker protocol unless the recovery design proves a protocol addition necessary.
- Do not weaken error handling by hiding Shell, process-exit, database, or shutdown failures.
- Do not commit or push unless the operator explicitly requests it.

## Workstream 1 - Lock In Regression Coverage

Add the narrowest failing tests that reproduce the acceptance findings before refactoring.

### Terminal command state

- Extend `ShellSessionWorkflowTests` to drive `running -> completed` and
  `running -> cancelling -> cancelled` lifecycle events.
- Assert `StartRunCommand.CanExecute` becomes true again without changing setup fields.
- Cover failed and interrupted terminal states so every terminal transition restores edit/rerun
  capability consistently.

### Explorer reveal

- Add infrastructure coverage for ordinary DOS paths, `\\?\C:\...` paths, and
  `\\?\UNC\server\share\...` paths.
- Separate testable Shell-path normalization/PIDL preparation from the native invocation if needed;
  do not alter the persisted/displayed canonical path merely to satisfy Explorer.
- Upgrade WPF smoke to invoke both file and folder reveal, wait for completion, and fail if either
  detail error surface becomes visible. A final operator check must still verify the intended item
  is selected in Explorer.

### Shutdown and recovery

- Add real-process WPF coverage that closes an idle connected app, a result-loaded app, and each
  startup-failure screen; require exit code 0 and no worker survivor.
- Repeat the normal-close case enough times to catch the observed intermittent orphan behavior.
- Add worker-exit tests that assert pending requests fail, active UI state is cleared or marked
  unavailable, and a visible restart action is offered.

Exit criteria:

- Every acceptance failure has a deterministic failing test or a documented interactive assertion
  where Windows Shell selection cannot be asserted reliably in-process.
- Existing tests remain green except for the newly introduced tests that intentionally demonstrate
  the unfixed behavior during development.

## Workstream 2 - Restore Terminal Rerun State

Repair terminal lifecycle notification ordering and command invalidation.

- Ensure setup mutability and `CanStart` notifications are restored before or together with the
  shell's final `StartRunCommand` invalidation.
- Avoid requiring a no-op setup edit/save to refresh command state.
- Preserve the selected historical run and completed results while a new immutable run starts.
- Decide whether terminal phases remain visible as last-known phases or become a clearer terminal
  label; do not rewrite persisted lifecycle history solely for presentation.

Exit criteria:

- Completed, cancelled, failed, and interrupted runs all permit an immediate new run when the saved
  session has a reachable root.
- A rerun creates a new run ID, leaves prior history immutable, and never mixes result ownership.

## Workstream 3 - Repair Native Explorer Reveal

Characterize the failing HRESULT before choosing the smallest fix.

- Confirm behavior for file and folder members with ordinary and extended-length canonical paths.
- Convert extended DOS/UNC syntax to a Shell-compatible parsing form at the Shell boundary when
  required; retain long-path correctness in the scanner and database.
- Verify COM/apartment and absolute-parent/relative-child PIDL requirements for
  `SHOpenFolderAndSelectItems` rather than falling back to command-line `explorer.exe` quoting.
- Return actionable errors with the attempted local path when the Shell API genuinely fails.

Exit criteria:

- `group010` file members and both `original-set`/`renamed-set` folders reveal the intended item.
- Reveal works after restart and on a supported path longer than 260 characters.
- File and folder reveal failures are caught by automated smoke instead of being reported as an
  invocation-only pass.

## Workstream 4 - Make Shutdown Deterministic

Replace the re-entrant `async void` close sequence with one explicit, idempotent shutdown owner.

- Do not call `Window.Close()` from inside a still-active `Closing` event.
- Allow one asynchronous cancel/worker-EOF sequence, then schedule final window/application
  shutdown after the original close callback has returned.
- Make application/service disposal idempotent so `OnExit` cannot race or double-dispose the worker.
- Preserve the bounded worker grace period and terminate only the owned child process tree when the
  protocol EOF shutdown does not complete.
- Define behavior for idle, completed, active/cancelling, already-exited-worker, startup-failure,
  and database-failure states.

Exit criteria:

- Every close path exits the WPF process without an unhandled exception.
- No `super-duper-worker` child remains after the app exits.
- Repeated normal closes pass without retry or forced cleanup by the test harness.

## Workstream 5 - Add Real Unexpected-Worker Recovery

The current client is effectively single-use after process exit. Introduce the smallest clear
ownership boundary that can replace/restart it without rebuilding unrelated view models.

- Surface a typed unexpected-exit notification from infrastructure to the shell.
- Fail pending operations visibly and leave the UI in a coherent failed/recovery state, not a stale
  scanning phase.
- Offer one explicit restart action that creates a fresh connection, negotiates protocol V1, reloads
  sessions/history, and lets worker startup reconcile active rows to `interrupted`.
- Ensure completed history and result pages remain available after recovery.
- A failed restart must return to the recovery screen with executable and log locations; it must not
  loop or silently retry.

Exit criteria:

- Killing only the owned worker during a disposable scan produces a visible failure and enabled
  restart action.
- Restart marks the abandoned run `interrupted`, preserves completed history, and permits a new run
  to complete normally.
- Closing before, during, or after restart obeys the deterministic shutdown contract.

## Workstream 6 - Close Minor UX And Accessibility Findings

- Keep at most one blank root editor row, or focus the existing blank row when `Add path` is chosen.
- Ensure Save/Normalize still removes blank and nested roots before persistence.
- Clarify terminal status versus last phase for cancelled/interrupted runs.
- Add meaningful automation names to the session list, main tab control, run grid, file/folder group
  grids, member grids, and unlabeled setup edit controls without changing stable automation IDs.

Exit criteria:

- Keyboard and screen-reader inspection can identify each primary navigation, input, result, paging,
  and action control.
- Minor editor/status behavior no longer creates misleading state.

## Verification Matrix

Run from the repository root, in this order:

```powershell
cargo test --workspace
cargo test --workspace --release
cargo build --workspace --release
dotnet build apps/windows/SuperDuper.Windows.sln
dotnet test apps/windows/SuperDuper.Windows.sln
dotnet build apps/windows/SuperDuper.Windows.sln --configuration Release
dotnet test apps/windows/SuperDuper.Windows.sln --configuration Release
./scripts/Verify-WindowsRelease.ps1
```

Do not use `-SkipWpfSmoke` for code-complete verification. Treat retry-only product passes as
failures until diagnosed; record environment-only restrictions separately and rerun in the proper
interactive context.

Then repeat operator acceptance with a new isolated database/cache and retained smoke fixture:

1. create, complete, cancel, immediately rerun, and restore sessions/runs;
2. verify historical run isolation across the rerun;
3. verify file sorting/filtering/paging/members and actual Explorer selection;
4. verify folder filtering/members and actual Explorer selection;
5. close idle, completed, recovery, and interrupted/restarted states with no surviving process;
6. kill only the disposable scan's worker, use the in-app restart action, and complete a new run;
7. verify light/dark theme, keyboard/accessibility names, resizing, and available safe path types;
8. confirm no scanned-file mutation action is exposed.

## Code-Complete Gate

The Windows MVP may be called code complete only when all of the following are true:

- every critical/high acceptance defect above is closed with regression coverage;
- the full Debug/Release matrix and real Release WPF smoke pass in one environment-valid run;
- immediate rerun, restoration, run isolation, both result surfaces, and actual Explorer reveal pass;
- unexpected-worker restart and every shutdown path pass without forced test cleanup;
- diagnostics/privacy/rotation checks pass and no SQLite ownership boundary is violated;
- the UI remains non-destructive and no post-MVP feature has entered scope;
- the final Git audit shows only intentional source/documentation changes and the preserved `.vs`
  cache remains untracked.

Do not mark this plan complete merely because the existing automated matrix passes. The failed
operator gates are part of the definition of done.
