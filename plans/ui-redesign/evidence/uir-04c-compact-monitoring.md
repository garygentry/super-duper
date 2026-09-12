# UIR-04c compact scan monitoring and exact details

2026-09-12. Baseline `163afce` (UIR-04b), implemented on `codex/ui-redesign`.
Local A08/A16 presentation is implemented; UIR-04 retains later integration/native/operator validation.

## Behavior

- The compact S02 summary presents phase, elapsed, warnings/Cancel, update freshness, qualified ETA,
  measured phase work and sampled filename/parent before three collapsed native disclosures.
  Filename and parent each occupy a stable trimmed line. Extended-length prefixes are removed only
  from the display; the exact worker path remains selectable and horizontally scrollable in Diagnostics.
- Hash bars use `HashPipelineResolvedBytes / HashPipelineCandidates.LogicalBytes`, not discovery,
  full-read bytes or file counts. The worker's existing `remainingKnownWork` presence establishes
  that candidate totals are known (`telemetry/progress.rs::remaining_work`); initial zero counters
  do not. Unknown totals are indeterminate, known zero totals explicitly have no percentage.
  Decimal arithmetic handles the full unsigned 64-bit byte range without multiplication overflow.
- All four folder substages use their own completed/total values and readable names. Zero and
  unavailable substages remain explicit. A substage reset changes its label and denominator.
  A lifecycle phase arriving before its matching detailed snapshot cannot borrow the previous bar.
  Reaching 100% never marks a run completed or disables Cancel.
- Work preserves six funnel rows with exact logical bytes, candidate/resolved/remaining work,
  folder substage counts, partial/full physical byte totals, read outcomes and recent/cumulative
  rates with their windows. Hash reuse shows separate partial/full hits, misses, errors and stores,
  retains the lookup hit rate and explains zero hits, qualification and non-unique lookup counts.
  Diagnostics preserves exact path/message, phase duration, sample/telemetry diagnostics, active
  device availability and exclusions, with a contextual Performance reminder.
- Existing disclosures retain their native controls across updates and same-run tab navigation.
  Exact-path focus, selection and horizontal/outer scroll survive ordinary progress and clock ticks.
  Terminal views hide the bar without collapsing its space, label unknown phase work historical
  instead of waiting for more updates, label retained metrics historical and
  retain the UIR-04b terminal path/focus/elapsed behavior. ETA reasons, receipt freshness, latest-only
  dispatch, five-second ordinary and prompt lifecycle/phase MostRecent announcements are unchanged.

No Rust, protocol, cache, infrastructure, query ceilings, production state or deletion wiring changed.
Production deletion remains disabled. SOP10 and consumed SOP9/other campaign boundaries are unchanged.
No physical scan/provider/performance campaign or operator acceptance was requested or run.

## Verification and review

All new builds, tests, captures and temporary files use `artifacts/uir04c`. Git used a per-command
safe.directory override; no global Git configuration changed. Initial sandbox restore could not read
NuGet.Config; approved access ran the same isolated tests with the existing package cache.

| Check | Result / retained output |
|---|---|
| Initial Core | 192 passed, one old rounded-byte expectation failed; updated to exact bytes (`results/core-initial.trx`) |
| Core with compact regressions | 199 passed (`results/core-monitoring.trx`) |
| Final Core after known-total review | 199 passed (`results/core-final.trx`) |
| Initial WPF | Two passed, one failed: ProgressBar Value defaulted to two-way on a read-only projection. Fixed with explicit OneWay (`results/wpf-initial.trx`) |
| WPF after binding correction | Three methods passed (`results/wpf-bindings.trx`) |
| WPF review run | Two passed, one existing Files fixture scroll assertion failed after native focused-row restoration (`results/wpf-final.trx`) |
| Deterministic navigation setup | Three methods passed (`results/wpf-final-retention.trx`); fixture focuses the navigation tab before deliberately scrolling its former row out of view. Actual row-focus and unchanged-offset assertions remain; no Files product change |
| Final native layout verification | Three methods passed (`results/wpf-layout-final.trx`); bounded disclosure-animation settling and explicit exact-path reachability precede captures/offset comparisons |
| Final terminal review | Core 200 passed (`results/core-terminal-final.trx`), three WPF methods passed (`results/wpf-terminal-final.trx`); stopped unknown work is historical, never waiting for updates |
| Fictional desktop fixture | Built in `fixture`, zero warnings/errors; not launched |
| Review | Product/test diff, exact known-total source anchor, representative captures and `git diff --check` |

Seven new Core methods cover known/unknown/zero candidate work, logical versus physical denominators,
100% while still active, all four folder substages and resets, lifecycle/snapshot phase separation,
64-bit byte totals, separate cache outcomes, exact diagnostics UNC/extended path display handling and stopped runs without measured totals.
The full Core suite retains UIR-04b clock/rejection/terminal/cancellation/coalescing behavior.

Loaded-STA native checks exercise 900x600 and 1180x760 with discovery, measured/unknown/zero hashing,
unknown folders, all four measured folder substages, zero folder work and finalizing. They inspect
bar Value/indeterminate/UIA labels, summary bounds, closed disclosures, exact bytes/cache values,
long-path focus/selection/horizontal and page scroll, disclosure retention and navigation. Existing
clock/minimize/restore/four-terminal-state/MostRecent and scrollbar/theme/text-size assertions remain.

Capture review found one early exact-details image taken during Fluent expansion. The verifier now
waits for stable geometry for 250 ms, bounded by three seconds, before inspecting/capturing disclosures.
Final captures are in `captures-terminal-final`; settled pre-copy-review captures are also retained
in `captures-layout-final`. Earlier captures remain in `captures-bindings` and
`captures-final-retention` without being represented as final settled-layout evidence. Reviewed views
include compact hashing/unknown/zero work, folder verification, terminal historical summary and
settled exact-path/reuse details. These are native fixture checks, not physical screen-reader or
operator acceptance. Retain prior real-worker/Debug/Release/Rust results without rerunning campaigns.

Representative commands (TEMP/TMP set to `artifacts/uir04c/temp`):

```powershell
dotnet test apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj --artifacts-path artifacts/uir04c/dotnet
dotnet test apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj --artifacts-path artifacts/uir04c/wpf
dotnet build apps/windows/tools/SuperDuper.Windows.RedesignFixture/SuperDuper.Windows.RedesignFixture.csproj --artifacts-path artifacts/uir04c/fixture
```

Set `SUPER_DUPER_UIR04B_CAPTURES` to an isolated capture folder for the shared long-scan/compact fixture.
The variable name is retained so prior fixture procedures continue to work.

## Runtime and next slice

Read-only process audits found only fictional PID 67748/session 1, responsive at the exact older
`artifacts/uir03f/fixture/bin/SuperDuper.Windows.RedesignFixture/debug_win-x64/SuperDuper.Windows.RedesignFixture.exe`.
The CIM audit was denied; Get-Process supplied the path/session/responsiveness (`runtime-audit.json`).
The older fixture was left untouched. UIR-04b/04c fixtures are built, not launched; test windows closed.
No production app/worker was observed or modified. Re-audit paths before any future reuse/launch.

Next local slice: **UIR-05a compact file-results query controls and filtered totals (A03/A06/A13)**.
Read S03 and directly linked DuplicateFiles query/view/tests. Keep path search and three filtered
totals visible; preserve draft/apply/Enter/chips/clear semantics, exact size units, all existing
filters, selected-run identity, durable decisions, paging/query ceilings and focus. Use isolated
delayed-query/long-path fixtures. Adjustable list/detail comparison remains UIR-05b, including full A03.

Full A17 changed/preserved-mtime membership, missing-root/fallback/interrupted-reuse coverage remains
UIR-07/08. UIR-04 integrated/native/operator workflow validation remains pending. NVDA and physical
200% stay `unrun_unavailable` for UIR-08/A11, not passed or waived; do not troubleshoot Windows.
