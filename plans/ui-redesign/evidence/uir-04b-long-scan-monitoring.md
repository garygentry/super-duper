# UIR-04b long-scan monitoring

2026-09-12. Baseline `2642db9` (UIR-04a). UIR-04b is implemented and locally verified
for the elapsed/freshness/terminal portion of A08/A16. UIR-04 remains in progress.

## Behavior and boundaries

- Run elapsed time uses readable days/hours/minutes beyond 24 hours, including the existing
  active strip binding. Phase/ETA durations also retain days beyond 24 hours. Local time changes
  display elapsed time without manufacturing a worker frame, count, rate or announcement.
- Shell captures the local monotonic receipt timestamp before the existing 100-ms application
  gate. Only an accepted progress/lifecycle update advances freshness; rejected frames do not.
  A dispatcher delay therefore cannot make an old received update look fresh. Clock-only refreshes
  coalesce to one pending dispatcher callback and remain silent to screen readers.
- After 30 seconds without a received update, the text explicitly says this observation alone
  does not indicate failure. Worker-reported no-candidate-progress ETA, unavailable metrics,
  cancellation, actual unexpected worker exit and failed lifecycle/error evidence remain distinct.
- Terminal activity has a visible historical label and an accessible last-reported path name.
  The activity bar becomes hidden, reserving its layout space. Retained funnel/rates/cache values
  stay available with a historical caveat; the scope says **this scan** has stopped, so another
  active run is not contradicted. Phase transitions clear an unchanged carried-over activity path.
- Terminal elapsed time freezes at CompletedAt, or at the local stop observation when that value
  is unavailable (explicitly labeled). Late running/cancelling lifecycle or cancel replies cannot
  revive terminal animation. Reconciled durable terminal totals can still replace provisional data.
- Lifecycle/phase announcements are prompt, including cancellation/failure before detailed progress.
  Ordinary progress retains the five-second worker-monotonic cadence and MostRecent processing.
  Timer ticks and rejected frames do not announce; completion preserves focus on an inspected path.
- Existing six funnel rows, physical rate windows, candidate denominator, folder substage counts,
  cache projection, warning access and contextual Performance contracts are retained. Available ETA
  explicitly names the hash pipeline. This slice adds no global or per-file percentage.

No Rust, worker protocol, cache implementation, query limits, production state or deletion wiring
changed. Production deletion stays disabled. No physical/provider/performance campaign ran.

## Verification

All new build/test/temp/capture outputs are isolated under `artifacts/uir04b`. The sandbox's first
Core restore could not read the user's NuGet.Config; the same isolated command succeeded with
approved access. No machine configuration was changed. Final Core/WPF suites pass; the style-review
failure below is retained with its correction.

| Check | Result / retained output |
|---|---|
| Focused progress, lifecycle and application gate | 38 passed, `results/progress-initial.trx` |
| Initial full Core | 187 passed, `results/core-initial.trx` |
| Final full Core after review | 193 passed, `results/core-final.trx` |
| Loaded-STA WPF | Three methods passed in each of `results/wpf-initial.trx` and `results/wpf-final.trx`, including LongScanMonitoringFixture |
| Native style review | `results/wpf-theme-review.trx`: two passed, one failed because `DefaultProgressBarStyle` is not a resource in this theme. Removed the resource dependency and moved terminal visibility to a surrounding Border, preserving the original native control style |
| Final native WPF | Three methods passed, `results/wpf-native-final.trx`; new assertion prevents a local ProgressBar style override |
| Fictional desktop fixture | Build passed with zero warnings/errors, isolated `fixture` output; not launched |
| Review | Product/test diff, representative final captures and `git diff --check` |

The controlled clock tests cover 24-hour boundaries, 2d 7h 14m duration, UTC adjustments independent
of monotonic receipt age, long silent intervals without fabricated progress, malformed/wrong-run/
duplicate/regressing frames, accepted unchanged counters, fresh receipt with worker-reported no
candidate progress, delayed dispatcher receipt, all four terminal states, durable reconciliation,
late cancellation replies, timer queue bounds and disposal. Existing application-gate tests retain
the ten-per-second limit. Existing shell tests retain historical/active context and worker recovery.

The WPF fixture uses actual ScanProgressView bindings in an offscreen STA window at 900x600 and
1180x760. It verifies readable freshness/duration, all four terminal states, hidden activity bar,
historical labels, failure detail, actual path focus/selection/scroll retention, minimized/restored
latest-state rendering and MostRecent notification delivery. Captures are in `captures-initial`
and `captures-final`, with the final native-style captures in `captures-native-final`. Representative stale, completed-summary and failed-activity/error images were
inspected. These are fixture-backed native checks, not physical screen-reader/operator acceptance.

Representative commands (TEMP/TMP set to `artifacts/uir04b/temp`):

```powershell
dotnet test apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj --artifacts-path artifacts/uir04b/dotnet
dotnet test apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj --artifacts-path artifacts/uir04b/wpf
dotnet build apps/windows/tools/SuperDuper.Windows.RedesignFixture/SuperDuper.Windows.RedesignFixture.csproj --artifacts-path artifacts/uir04b/fixture
```

Retain UIR-04a's small real-worker reuse/restart result and the prior paired Debug/Release/Rust
checks without rerunning them. This Core/WPF slice does not require a new engine or integration
campaign. The full matrix remains UIR-08.

## Runtime and next slice

Read-only startup audit found PID 67748/session 1 responsive at
`artifacts/uir03f/fixture/bin/SuperDuper.Windows.RedesignFixture/debug_win-x64/SuperDuper.Windows.RedesignFixture.exe`.
It remains the older build and was left untouched. No production app/worker was observed.
The UIR-04b test windows closed; no new fixture was left running. Re-audit before reuse/launch.

Next local slice **UIR-04c** completes the compact S02 monitoring summary and expandable scan
details (A08/A16): place sampled filename/parent and qualified phase work up front, use existing
hash-candidate resolved logical work and folder-substage denominators for measured bars, explicitly
handle unknown/zero totals, and disclose exact Work / Hash reuse / Diagnostics values without
losing them during updates/navigation. Preserve this slice's freshness, terminal, focus and
announcement contracts. Prepare controlled phase/layout fixtures; no physical scan is needed.

Full A17 combined membership/change/fallback/interrupted-reuse coverage remains UIR-07/08.
Operator workflow acceptance and UIR-08 integration remain later. NVDA and physical 200% remain
`unrun_unavailable`, not passed or waived; do not troubleshoot Windows or install software.
