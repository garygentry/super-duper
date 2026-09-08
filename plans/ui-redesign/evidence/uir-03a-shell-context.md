# UIR-03a — shell/context foundations

Date: 2026-09-08. Scope: `local_code`, partial A01/A02/A09 and preserved A15.
Baseline: `422c3e7`; implementation is in this record's commit on `codex/ui-redesign`.
UIR-03 is **in_progress**, not complete.

## Changes and evidence

- Named destinations replace numeric shell routes/index bindings. Seven existing destinations
  remain reachable until UIR-03b groups the four primary areas.
- Selected results retain their run during active progress/cancellation/terminal lifecycle.
  Progress monitors the global active run across saved-scan selection; View progress does not
  retarget results. Selected header includes ID/date/state/location count; monitoring has its
  own ID/name/date/state, including after the active strip disappears at completion.
- Shared shell/recovery typography/actions/status use native focus and system brushes. The strip
  shows name, phase, existing elapsed display and warnings. Footer labels global engine status.
  Long-duration formatting remains UIR-04 work.
- Setup is usable independently of slow optional panes. Start waits for history readiness and
  matching setup identity. A failed setup load cannot expose/start the previous definition after
  error dismissal. History rejects late superseded/cancelled pages/errors; context changes clear
  preceding result targets. Performance loads on navigation; redundant sequential shell loads
  were removed. Other components retain their generations, page/cache limits and worker ownership.

## Isolation and verification

Git audit: clean `codex/ui-redesign`; `wpf-poc` remains at
`deefa40ebe607b785b395a29a6282e8b417a9b14`. Command-scoped safe.directory was needed for the
sandbox identity; no global Git configuration changed.

All commands use `--artifacts-path C:/Users/gary/workspace/super-duper/artifacts/uir03a/dotnet`.
Tests additionally use `--results-directory C:/Users/gary/workspace/super-duper/artifacts/uir03a/results`
and `--logger 'trx;LogFileName=<name>.trx'`.

| Command / scope | Result |
|---|---|
| `dotnet build apps/windows/src/SuperDuper.Windows/SuperDuper.Windows.csproj` | Exit 0; Debug WPF build, zero warnings/errors |
| `dotnet test apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj` | Final exit 0; 158 passed, 0 failed/skipped; `core-final.trx` |
| `dotnet test apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj` | Final exit 0; 4 passed, 0 failed/skipped; `wpf-final.trx` |
| `git diff --check` | Passed |

New Core tests cover older results during active progress/completion, cross-session cancellation,
delayed optional results, one result request per selection, demand-loaded Performance, late history
success/error, stale setup error and current setup failure after dismissal. Existing Shell, progress,
warning, review/preflight, collection and disabled-operation regressions pass in the full Core suite.

The shell WPF test embeds shipping shell markup/resources in a loaded STA presentation host with
fictional data and empty screen slots; no App startup, MainWindow shutdown or worker runs. Existing
loaded screen tests run separately. It reorders tabs and verifies semantic binding both ways, View
progress, selected/monitoring labels, focus style and header/action width. Its nonactivating offscreen
window closes after testing. With `SUPER_DUPER_UIR03_CAPTURES` set to the task's `artifacts/uir03a/captures`,
it writes `shell-1180x760.png` and `shell-900x600.png`. Both shell-only 96-DPI renders were inspected;
they use host theme defaults, not a full populated-screen/Fluent/theme/physical DPI matrix.

Initial failures retained: `core-focused.trx` had 30 passes/1 failure because the assertion preceded
coalesced progress delivery; it now awaits delivery without changing production cadence. WPF setup
needed approved Windows SDK metadata access outside the sandbox, then a missing test System.IO import
was fixed. `wpf-regression.trx` / `wpf-shell.trx` record presentation-host namespace/resource failures.
Empty screen slots resolved that host boundary. `wpf-shell-v2.trx` caught an assertion omitting WPF
margins; corrected accounting passes in `wpf-shell-v3.trx` and the final suite. These were fixture
defects, not reasons to weaken native acceptance or safety/performance contracts.

Operator WPF PID 36316 and worker PID 17612 remained present at the end at their original
`artifacts/windows-x64` paths. No interruption, production database access, real worker launch,
Rust/cache changes or physical/provider/performance campaign occurred.

## Remaining acceptance and next slice

UIR-03b: four-area composition, explicit History Open scan with separate highlighted/workspace run,
and deferred file/folder/review loads retaining bounded same-run state. Verify delayed navigation,
cross-session warnings, lifecycle/worker-exit selection stability and focus through the new layout.
Shared resource adoption across all screens and A01/A02/A09 desktop walkthrough remain open.
No integrated Rust/.NET Debug/Release matrix, physical keyboard/Narrator/NVDA/high-contrast/multi-
monitor checks or user acceptance ran. Production executor injection, `CanSubmit == false` and worker
`executorEnabled:false` were inspected and remain unchanged. A08/A16/A17 remain required for
UIR-04/07/08 with existing persistent cache and progress cadence preserved.
