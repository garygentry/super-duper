# UIR-08 integration regression and acceptance matrix

Date: 2026-09-14

Scope: `local_code` integration regression and acceptance-matrix preparation across UIR-04 through
UIR-07. Baseline `a37bbf9` (UIR-07b). UIR-08 is `in_progress`: the locally available matrix and
current automated visual review pass, while the prepared native/operator actions below remain unrun
and separately authorized. This is not UIR-09 user acceptance or Windows release acceptance.

## Current integrated result

The Rust workspace and Windows solution were built and tested from the same checkout in Debug and
Release. Rust used a new `CARGO_TARGET_DIR` under `artifacts/uir08/target`. Windows used paired worker
executables from that target and fresh `--artifacts-path` directories. Test child `TEMP`/`TMP` paths
were task-owned. Real Recycle Bin, provider, cloud-root and performance opt-ins were cleared.

| Check | Current result |
|---|---|
| Rust workspace build, Debug / Release | passed |
| Rust workspace tests, each configuration | **226 passed, 10 ignored**, 0 failed |
| Windows solution build, Debug / Release | passed, **0 warnings / 0 errors** |
| Core tests, each configuration | **220 passed**, 0 skipped |
| Infrastructure tests, each configuration | **76 passed, 5 skipped**, 0 failed |
| Loaded-STA WPF methods, each configuration | **3 passed**, 0 skipped |
| Current WPF capture regression, Debug / Release | **3 passed** each; **191 PNGs** in each capture set |
| Latest fictional fixture build | passed, **0 warnings / 0 errors**; build only, not launched |

Final Windows TRX files are under `artifacts/uir08/results-final`; current capture TRX files are under
`artifacts/uir08/results-captures`. Captures are under `artifacts/uir08/captures-{debug,release}`.
Representative wide/narrow Files, Folders, Review preferences, History warnings, Performance device/
comparison, compact long-scan details, terminal interruption and Light/Dark 150%-text states were
visually reviewed. No new clipping, overlap, false state or raw-sample/time-series claim was found.
The 96-DPI bitmaps and loaded-STA geometry checks are automated evidence, not physical DPI or reader
acceptance.

The first Windows build attempt was unable to read the installed Windows SDK inside the sandbox and
failed before compilation. Its `artifacts/uir08/dotnet` output remains retained. The same source ran
in fresh `dotnet-approved` paths with SDK access, then the committed-shape Windows matrix ran in fresh
`dotnet-final` paths. No failed result was overwritten or relabeled.

## A17 cross-layer regression added

`SavedScanRepeatTests.ReopenedCacheAndEditedSetupCreateNewRunsWithoutRewritingHistory` now uses one
disposable worker-owned history database and persistent hash cache across a worker restart and five
runs. It proves:

- an unchanged repeat creates a new run and reports both partial and full qualified cache hits;
- one later run freshly discovers an added file, omits a deleted file and rejects changed content
  whose size and modified time were preserved;
- that run retains a qualified full-cache hit for unchanged content, a miss/read for new or changed
  content, and exactly the two currently equal members;
- the original and unchanged-repeat memberships remain intact, the original immutable parameters and
  manual decision remain intact, an edited exclusion affects only its run, and **Re-read candidate
  content** reports no full-cache hits before restoring the current two-member result.

The first strengthened assertion compared different valid Windows path spellings and failed while
the two-member result was present. That TRX remains under `artifacts/uir08/results-a17-focused`.
The corrected exact filename/membership assertion passed once in Debug and Release under
`results-a17-focused-2`; the full final Windows matrix then passed in both configurations.

## Acceptance-matrix disposition

States below describe this gate only. `current_local_pass` is not a substitute for the native/user
evidence named in the remaining column, and no row is promoted to final redesign acceptance here.

| ID | UIR-08 state | Current evidence | Remaining evidence / disposition |
|---|---|---|---|
| A01 | `current_local_pass`, retained scoped native pass | Core/Shell context and loaded-STA current screens; UIR-03 operator selected/active/highlight/Open checks | Latest-screen user journey remains UIR-08/09 native acceptance |
| A02 | `current_local_pass`, retained scoped native pass | Delayed/out-of-order/error generation tests; optional pane and worker-exit coverage; UIR-03 delayed-folder operator pass | Latest integrated failure explanation remains in the prepared walkthrough |
| A03 | `current_local_pass` | 1180x760 measured 69.7% Files comparison; 900x600 vertical-only set/copy/detail access; current Debug/Release WPF geometry and captures | Physical latest-screen narrow review remains unrun |
| A04 | `current_local_pass` | File/folder decision identity, late rejection, Reset, rule application/reversal and manual-override tests in both configurations | Current native user interpretation remains unrun |
| A05 | `current_local_pass` | Distinct empty/loading/filter/cancelled/failed/interrupted/unavailable fixtures and current captures | Current native state interpretation remains unrun |
| A06 | `current_local_pass` | Visible-label/copy assertions, complete local/UNC/extended paths and vertically reachable technical detail | Current native copy/path walkthrough remains unrun |
| A07 | `current_local_pass` | 500-run History paging, immutable parameters/decisions, explicit Open scan and restart tests | Current native History workflow remains unrun |
| A08 | `current_local_pass` | Controlled >48-hour clocks, phases, unknown ETA/work, warnings, cancellation and four terminal states in Core/WPF | No real multi-day/native observation was run; do not infer it |
| A09 | `current_local_pass`, retained native partial | Shared resources, Light/Dark, 100%/150% text, both viewports and recovery styling in current WPF; retained UIR-03 Dark/Desert/text fixes | Latest all-screen contrast/theme inspection remains unrun |
| A10 | `current_local_pass` | Worker-owned combined totals, overlap/alias/survivor/revision/preflight and separate bounded Review pages | Current native Review interpretation remains unrun |
| A11 | `current_local_pass`, retained native partial | Loaded-STA focus/automation/virtualization/theme/text checks; retained keyboard, Narrator and 150%/175% monitor transitions | Latest-screen keyboard/Narrator/contrast/DPI walkthrough unrun; NVDA and physical 200% are `unrun_unavailable` |
| A12 | `current_local_pass` | Exact contextual Performance/warning navigation, 25/6/64 bounds, unavailable values and comparison qualifiers | Current native Performance/warning interpretation remains unrun |
| A13 | `current_local_pass` | Full Rust/.NET query/page/cache/update bounds, 100,000-group bounded regressions and WPF virtualization run in both configurations | Ten operator/performance profiles remain intentionally ignored; no new campaign authorized |
| A14 | `current_local_pass` | Setup validation, Save/Discard/Stay, failed cloud detection, save/start races and single-active-run tests | Current native setup explanation remains unrun |
| A15 | `current_local_pass` | Full matrix and source audit retain disabled executor injection, `CanSubmit=false`, worker `executorEnabled=false` and no production execution action | Production execution remains out of scope |
| A16 | `current_local_pass` | Controlled freshness/no-progress/disconnect/failure, coalesced UIA cadence, minimized/restore, exact diagnostics and terminal silence | Real long-duration native observation remains unrun and separately authorized |
| A17 | `current_local_pass` for combined automated workflow | Strengthened five-run real-worker restart/cache/new/deleted/changed/exclusion/reread/history regression plus Rust signature/fallback/cancellation anchors | Current native Scan again interpretation remains unrun; no physical/full-drive campaign was run |

## Explicit skipped, unavailable and unrun states

- The five Windows skips in each configuration are the expected physical/integration opt-ins:
  registered cloud discovery, registered-provider no-transfer inspection, and three real Recycle Bin
  survivor/cancellation/locked-file cases. They were not enabled. They do not close redesign-native,
  provider or deletion acceptance.
- The ten Rust ignored cases in each configuration are the named SOP1/SOP6/SOP7/SOP8/SOP10 physical
  or scale profiles, two preference-rule performance profiles, representative Review performance,
  and the retained warning scale profile. They remain ignored; no consumed identity was rerun.
- NVDA is `unrun_unavailable` because it is not installed. Physical 200% display scaling is
  `unrun_unavailable` because Windows did not offer it. Neither is passed, waived or replaced by
  Narrator or 175%. No Windows troubleshooting, forced scaling or installation was performed.
- The latest UIR-08 fixture was not opened. No current physical keyboard, Narrator, contrast-theme,
  text-size or multi-monitor action was run. No shipping WPF smoke, real long-duration scan, provider,
  representative performance or production-state action was run.

## Smallest prepared native action — not run

After explicit authority, use the already built in-memory executable at
`artifacts/uir08/fixture-approved/bin/SuperDuper.Windows.RedesignFixture/debug_win-x64/SuperDuper.Windows.RedesignFixture.exe`.
Re-audit PID 67748 and the executable path first; do not
rebuild or stop the older fixture. Confirm the window title starts **FICTIONAL FIXTURE**. It has no
real worker, database, Explorer or deletion path.

Record one bounded available-native batch:

1. At 1180x760, 100% display scale, ordinary theme and 100% text, complete the keyboard-only path:
   Scan again/setup; active Progress details/Performance return; Files and Folders select/Keep/Mark/
   Reset/path detail; Review totals, **Check these copies** versus **Check marked copies**, Location
   preferences preview/apply/manual override/**Reverse rule application**; History highlight/Open,
   warnings and contextual Performance. Record actual focused control after every panel return.
2. Switch the fixture to 900x600 and repeat only Files/Folders exact path plus decision access,
   Review/Location preferences and History/Performance returns. Record any horizontal scrolling needed
   for an essential path or decision, clipping, overlap, assistance or misleading state.
3. With Narrator, repeat one Files row/copy decision (selection alone remains harmless), active
   Progress details and History warning/Performance return. Record labels, coalescing and focus.
4. Inspect the same three destinations in Dark, the available named contrast theme and 150% Windows
   text size, then move the open fixture through the available 150% and 175% monitor transitions.
   Record theme, text/display scale, monitor, focus/selection retention and defects; restore settings.

Report each step `pass`, `fail`, `unrun` or `unavailable`, with observed behavior rather than clicks.
Do not include NVDA or 200% in the runnable batch, troubleshoot Windows, or substitute their absence
as success. A real-worker native A17 rescan, physical long-duration scan, provider/performance work
and UIR-09 final acceptance are separate actions and need their own explicit authority.

## Runtime and safety boundary

PID 67748/session 1 was re-audited responsive at the exact older UIR-03f fictional fixture and left
untouched. All UIR-08 test workers and windows exited; no other Super Duper app or worker remains.
No production database, cache, log, user file, recovery outcome or scan state was opened or changed.
`DisabledRecycleOperationCapabilityExecutor`, `CanSubmit=false`, `executorEnabled=false` and the
absence of a production execution action remain unchanged.
