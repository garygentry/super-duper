# UIR-03d — populated viewport access

2026-09-08. Baseline `aef8839`; implementation is this record's commit on
`codex/ui-redesign`. Scope: `local_code`, the viewport prerequisite for A01/A02/A09.
**UIR-03 remains in_progress.** Desktop acceptance and the full S03/A03 Results layout are not passed.

## Changes and tradeoffs

- Files uses a vertically scrolling page with independently bounded native grids (180–300 DIPs).
  Filters, totals, long selected-set context and validation messages cannot consume the grid rows'
  entire allocation. The comparison heading and set navigation use the available width; validation
  and copy-page commands wrap below the grid. Shared 14-DIP text, 32-DIP actions, 36-DIP minimum rows,
  content insets and the 900x600 window minimum remain unchanged.
- This interim stacked page replaces the old horizontal splitter between the vertically stacked
  grids. The final adjustable list/detail composition, collapsed rail, compact search/totals and
  no-horizontal-scroll essential path/decision workflow remain explicitly in UIR-05. The current
  technical tables still require horizontal scrolling; this slice does not claim A03's 60% rule.
- The existing member grid's star column could shrink other requested column widths, including
  decision and Explorer buttons. Explicit minimum column widths and wider action cells keep the
  full controls reachable through native horizontal scrolling. No decision command or target changed.
- History has a vertically scrolling page with a bounded run list (144–300 DIPs) and warning grid
  (144–260 DIPs). Header/actions no longer compete for one narrow line. Warning Close docks correctly
  at the right; paging wraps, and warning text/action/example columns retain usable minimum widths.
- The shared fake-service fixture now populates displayed selected roots, long relative paths,
  drive labels, location counts and the selected set's two undecided/remaining copies. Previously
  it supplied complete paths but left the displayed root/relative-path fields empty. No real paths
  are inspected; the fixture remains entirely in-memory.

## Focus, geometry and retention evidence

The existing loaded-STA WPF suite still reports four methods. The populated scenario now checks
requested 1180x760 and 900x600 sizes, plus 900x600 with 80 DIPs reserved for the standalone host's
wrapping toolbar. The latter is an overhead allowance, not a capture of the actual toolbar.

- Both Files disclosures open and closed: bounded grids, realized first rows and all set/page/
  validation commands are reachable. Geometry intersects the actual clipping ancestors and window;
  button success requires its complete bounds, not merely `IsVisible` or an automation ID.
- The bound 25-group page does not realize all 25 rows. Native virtualization and the existing
  collection/query bounds are retained. This is a targeted layout regression, not a scale campaign.
- The comparison's Keep/Remove/Undecided and Copy path/Show in Explorer buttons are brought into
  the viewport through their actual native columns and checked for clipping, without making decisions.
- Actual Next set and Validate page Click handlers return keyboard focus to the group/member grids;
  the focused row/grid is also visibly reachable. Existing MainWindow Open scan, View progress,
  warning-result navigation, stale-focus rejection and RunHistoryView warning-close tests remain.
- Same-run navigation retains selected group, nonzero group-grid scroll and nonzero outer-page
  scroll; the original progress-scroll and one-query same-run reuse assertions remain. The delayed
  failing optional-folder query still leaves Files and the opened historical context usable.
- History's run rows, warning action, Close and paging/cancellation controls remain reachable at
  both sizes and with the toolbar allowance. Programmatic warning focus and return are checked.

Final Debug/Release renders are under ignored `artifacts/uir03d/captures-integration-{debug,release}`.
They are RenderTargetBitmap captures at 96 DPI using system brushes and an explicit offscreen window
background. Requested outer sizes differ from the drawn client content because native chrome is not
captured. Normal/narrow comparison, action and History/warning captures were inspected; scrolling
intentionally exposes different parts of the page. They are not physical theme/DPI acceptance.

## Verification and isolation

| Check | Result |
|---|---|
| Paired Rust worker build, Debug / Release | Passed; no Rust/protocol/shared-contract changes |
| Windows solution build, Debug / Release | Passed, zero warnings/errors |
| Core tests, each configuration | 170 passed |
| Infrastructure tests, each configuration | 75 passed; five operator-only skips |
| WPF tests, each configuration | Four passed, including the expanded populated scenario |
| Standalone launcher | Build-only default passed; hidden `--verify` PID 48404 exited 0 |

Windows build/test used `--artifacts-path artifacts/uir03d/dotnet`, the paired
`-p:WorkerExecutableSource=<repo>/artifacts/uir03d/target/<profile>/super-duper-worker.exe`, then
`test --no-build --no-restore`, with `-c Release` for Release. Integration TEMP/TMP were
`artifacts/uir03d/temp`; `SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS`,
`SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS` and `SUPER_DUPER_EXPECTED_CLOUD_ROOT` were cleared
only in the test process environment. Logs are `dotnet-{debug,release}-{build,test}.log`; TRX
results use `debug-integration` / `release-integration` prefixes under `artifacts/uir03d/results`.

The Debug worker was freshly built with `cargo build -p super-duper-worker --locked` and
`CARGO_TARGET_DIR=<repo>/artifacts/uir03d/target`. Release used the same command with `--release`
and the existing isolated `artifacts/uir03c/target` compiler cache, then copied the paired executable
to `artifacts/uir03d/target/release`; its log is `rust-release-build.log`. Initial focused WPF runs
used the already verified UIR-03c Debug worker source (the fake-service WPF scenario never starts it).
The 226-passed/10-ignored Rust test baseline from UIR-03c is retained, not rerun or relabeled here.

The default sandbox could not read installed Windows SDK metadata. SDK-enabled execution was
approved with isolated outputs. Failed evidence remains in `results`: initial/layout runs found old
splitter assertions, reachable found its old contiguous tab-index expectation, geometry/scroll/
diagnostic exposed shrinking action columns, and `wpf-final.trx` caught a transient disclosure extent
during retention measurement. The diagnostic measured unchanged steady-state extents; the verifier
now requires a stable layout for 250 ms (bounded by three seconds) after closing disclosures before
comparing offsets. It does not relax the one-DIP offset assertions. Subsequent integration passes
include this settling check and populated long paths. Earlier captures remain separately named.

Git started clean at `aef8839`; command-scoped `safe.directory` handled sandbox ownership without
global configuration changes. Operator WPF 36316 and worker 17612 at `artifacts/windows-x64` stayed
untouched. Owned fixture and test workers exited. No app shutdown, production database access,
physical scan, provider action, deletion or consumed campaign ran. Production worker ownership,
active progress/cancellation, survivor/revision/overlap protections, engine/query/cache boundaries
and disabled production deletion are unchanged. A08/A16/A17 remain required in their later gates.

## Exact next slice

**UIR-03 desktop acceptance: operator A01/A02/A09 walkthrough**, using
[the prepared procedure](uir-03-desktop-walkthrough.md) and
`scripts/Invoke-UiRedesignFixture.ps1 -Show`. The local viewport prerequisite is implemented and
verified; request the specific operator walkthrough now. Record actual observations and defects,
then assess UIR-03 against its acceptance requirements before advancing to UIR-04.

Physical keyboard, Narrator, NVDA, light/dark/high contrast, Windows text enlargement and physical
100/150/200% monitor/DPI checks remain **unrun**. Programmatic keyboard focus and fake-service
renders cannot substitute for those results. Representative folder/review/performance population
and full S03 Results work remain in later gates. No UIR-03 completion or release acceptance is claimed.
