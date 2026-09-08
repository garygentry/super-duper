# UIR-03b - workspace composition and deferred panes

Date: 2026-09-08. Scope: `local_code`, partial A01/A02/A07/A09 and preserved A15.
Baseline: `9c028ca`, product baseline `3775d42`. Implementation is in this record's commit on
`codex/ui-redesign`. UIR-03 remains **in_progress**.

## Behavior

- Four primary areas replace seven peer tabs. Semantic Scan Setup/Progress/Summary,
  Results Files/Folders, Review, and History Scans/Performance remain independent of tab order.
  Shared navigation resources retain native focus styles with 14-DIP text and 32-DIP minimums.
- Highlighting or refreshing History cannot change the workspace run or its live-review observer.
  Open scan explicitly opens completed results or a stopped run's summary. Summary cannot cancel
  another active run; View progress and cancellation continue to target the independent active run.
- Files, Folders, Review and Performance load when opened. One run's existing bounded caches,
  filters and selected state survive navigation; run/status changes clear pane state and cancel
  outstanding loads. No per-run cache dictionary or collection/query-limit change was introduced.
  Once opened, panes retain existing revision/lifecycle monitoring responsibilities.
- Current warnings across sessions preserve the workspace run. Saved-scan setup/history may change
  to the warning owner; the workspace name and dated identity stay explicit, History identifies its
  own context, and Start names its saved-scan target. Late warning entry cannot override subsequent
  navigation. Explicit warning-result navigation opens its immutable run.
- Stale file loading cannot subsequently load the old run's preference rules. Review opening,
  revision refresh and loading dismissal guard their original lifetime/generation, preventing stale
  errors, totals or a permanently loading cleared pane. Worker truth and correctness rules remain.

## Verification and isolation

Git started clean at `9c028ca` on `codex/ui-redesign`; `wpf-poc` stayed at
`deefa40ebe607b785b395a29a6282e8b417a9b14`. Command-scoped `safe.directory` handled sandbox ownership;
no global Git settings changed. CIM process metadata was denied; read-only Get-Process confirmed
operator WPF PID 36316 and worker PID 17612 at `artifacts/windows-x64` at startup and final audit.
No operator process was stopped or attached, no production database opened, no real worker/scan
launched, and no consumed or provider campaign run.

Commands use `--artifacts-path C:/Users/gary/workspace/super-duper/artifacts/uir03b/dotnet` and tests
use `--results-directory C:/Users/gary/workspace/super-duper/artifacts/uir03b/results` plus
`--logger 'trx;LogFileName=<name>.trx'`. Final commands were `dotnet test` on
`apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj` and
`apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj`, each with
`--no-restore` and those artifact/result/logger arguments. Initial runs restored to the same isolated
outputs. Repeated checks followed changed code or fixture failures.

| Scope | Result |
|---|---|
| Core test project, final Debug run | 170 passed, 0 failed/skipped; `core-complete.trx` |
| WPF Smoke test project, final Debug run | 4 passed, 0 failed/skipped; `wpf-complete.trx`; builds shipping WPF/Core/Infrastructure |
| `git diff --check` | Passed after final formatting review |

The twelve added Core cases cover harmless highlight/refresh, on-demand query counts and same-run
retention, delayed old Open success/error, current warnings across sessions, delayed warning entry,
active exit without workspace retargeting, three stopped-run statuses during another active scan,
late Review load/error/revision refresh, cleared loading state and late preference-rule loading.
Existing independent progress/completion/cancellation, immutable warning navigation, revision,
survivor, overlap, query bounds and disabled operation tests remain green.

The loaded-STA shell test embeds shipping markup/resources with empty screen slots, now driven by
real ShellViewModel and linked fake services. It verifies four-area routing after reordering,
Files/Folders routing, explicit Open scan, context, logical focus on the trigger and destination,
active progress, native focus resources, and header/action width at 1180x760 and 900x600. Tests never
run App startup or MainWindow shutdown. Offscreen nonactivating windows close at test end.
`SUPER_DUPER_UIR03_CAPTURES` saves shell-only 96-DPI PNGs in `artifacts/uir03b/captures`; both sizes
were inspected. These are host theme defaults, not full populated Fluent/theme/DPI acceptance.

Initial issues: sandbox restore could not read user NuGet.Config; approved test execution read the
installed configuration/SDK dependencies while keeping outputs isolated. The first WPF build lacked
System.IO for linked test doubles; its explicit test-project import fixed compilation. Initial Core
runs passed 158 then 167 tests as coverage grew. An intermediate shell render exposed a mixed-encoding
separator introduced during editing; UTF-8 repair plus a loaded-context assertion fixed it. A transient
line-ending issue failed diff-check and was repaired. Retained TRX files record successful intermediate
runs; these issues do not excuse any pending acceptance evidence.

## Limits and exact next slice

UIR-03c: complete shared resource adoption on the composed primary screens and prepare a populated
isolated shell fixture for A01/A02/A09, with actual programmatic/keyboard focus handlers, warning
return, pane reopen/scroll retention, long-name/narrow states and desktop walkthrough evidence.
Current shell-only tests verify logical focus, not actual MainWindow focus delivery or physical
keyboard, Narrator/NVDA, high contrast, theme switching or monitor DPI. Screen content redesign and
responsive rail/list-detail behavior retain their later gate scope. The integrated Debug/Release
Rust/.NET matrix remains pending; Rust and worker contracts were unchanged here.

Production executor injection, `CanSubmit == false`, worker `executorEnabled:false`, cache reuse,
worker ownership, collection ceilings and progress cadence remain unchanged. Long-scan and persistent
rescan A08/A16/A17 are retained for UIR-04/07/08. No native acceptance or release-ready claim.
