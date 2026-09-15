# UIR-08 integration regression and acceptance matrix

Date: 2026-09-14

Scope: integration regression, acceptance-matrix disposition and current native walkthrough across
UIR-04 through UIR-07. Baseline `a37bbf9` (UIR-07b). UIR-08 is `complete`: the locally available
matrix, automated visual review and available native/operator actions pass after correcting three
gaps in the in-memory acceptance fixture. This is not UIR-09 final user acceptance or Windows release
acceptance.

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
| Initial fictional fixture build | passed, **0 warnings / 0 errors** |
| Corrected native fixture build | passed, **0 warnings / 0 errors** under `artifacts/uir08-native-followup/fixture-corrected` |
| Focused corrected-fixture loaded-STA regression | **1 passed**, 0 skipped, 0 failed on retained rerun |

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

## Available native walkthrough

The operator completed the authorized four-step batch on 2026-09-14. The first current-screen pass
found three reproducible acceptance-fixture defects: file decision buttons reloaded unchanged fixed
data, Folders had no populated result handler, and Review's folder **Open set** consequently had no
target to open. These were fixture-only gaps: production view models and worker contracts were not
changed. `ShellFixtureData` now keeps in-memory file/folder decisions and review revisions coherent,
supplies 25 two-copy folder sets with local and UNC paths, and supports exact Review-to-Folders links.
The loaded-STA regression exercises visible Keep/Remove/Reset transitions for files and folders and
the Review folder link. Its first full-method run reached the new assertions and later failed one
pre-existing synthetic Enter-focus assertion in `FileQueryLayoutFixture`; a fresh isolated rerun
passed. Both TRX results are retained under `artifacts/uir08-native-followup/test-results`. The
corrected fixture built cleanly in a fresh output, and the operator reported the focused 1180x760
and 900x600 retest **all pass**.

| Step | Final state | Operator observation |
|---|---|---|
| 1. 1180x760 keyboard journey | `pass_after_fix` | Scan again/setup and active-scan separation passed; Progress details and contextual Performance passed; file decisions, populated folder decisions/paths and Review folder Open set passed after the fixture correction; History highlight/Open/warnings/Performance passed. Result-set/copy focus was retained, and History Performance returned to the current highlighted scan entry. |
| 2. 900x600 focused journey | `pass_after_fix` | The operator initially reported the same fixture gaps as at 1180x760, then reported the corrected file/folder decisions, exact paths and Review folder Open set all pass. No remaining essential horizontal-scroll, clipping, overlap, assistance or misleading-state defect was reported. |
| 3. Narrator | `pass` | One Files selection/decision, active Progress details and History warning/Performance return passed; labels, bounded announcements and focus were acceptable. Narrator was stopped afterward. |
| 4. Appearance and monitors | `pass` | Dark, the available named contrast theme, 150% Windows text and existing 150%/175% monitor transitions passed with no reported focus/selection, clipping, overlap or contrast defect. Changed settings were restored. |

Computer Use crashed the ChatGPT host twice while attempting the walkthrough, so the operator
completed the observations manually. That host failure is not counted as product acceptance evidence
or a Super Duper defect. The restarted fixture title was confirmed before the manual batch.

## Acceptance-matrix disposition

States below describe this gate only. `available_native_pass` records the operator's current-screen
evidence without substituting for explicitly unavailable hardware/software or UIR-09 final acceptance.

| ID | UIR-08 state | Current evidence | Remaining evidence / disposition |
|---|---|---|---|
| A01 | `current_local_pass`, `available_native_pass` | Core/Shell context and loaded-STA current screens; operator passed selected/opened/active, History highlight/Open and both current viewports | Final redesign acceptance remains UIR-09 |
| A02 | `current_local_pass`, retained scoped native pass | Delayed/out-of-order/error generation tests; optional pane and worker-exit coverage; retained UIR-03 delayed-folder operator pass | No new failure was injected in this native batch |
| A03 | `current_local_pass`, `available_native_pass` | 1180x760 measured 69.7% Files comparison; operator passed 900x600 vertical-only file/folder path and decision access after fixture correction | Final redesign acceptance remains UIR-09 |
| A04 | `current_local_pass`, `available_native_pass_after_fix` | File/folder decision identity, late rejection, Reset, rule reversal/manual override tests; operator passed visible file/folder Keep/Mark/Reset changes after fixture correction | Final redesign acceptance remains UIR-09 |
| A05 | `current_local_pass`, current native review | Distinct empty/loading/filter/cancelled/failed/interrupted/unavailable fixtures and current captures; no misleading state was reported in the available native batch | Individual injected states remain automated/retained evidence |
| A06 | `current_local_pass`, `available_native_pass_after_fix` | Visible labels, complete local/UNC/extended paths and vertically reachable detail; operator passed current file/folder path access | Final redesign acceptance remains UIR-09 |
| A07 | `current_local_pass`, `available_native_pass` | 500-run History paging, immutable parameters/decisions and explicit Open scan; operator passed current History highlight/Open | Final redesign acceptance remains UIR-09 |
| A08 | `current_local_pass`, `available_native_pass` | Controlled >48-hour clocks and terminal states; operator passed current Progress phase/activity/elapsed/warning/detail interpretation | No real multi-day observation was run; do not infer it |
| A09 | `current_local_pass`, `available_native_pass` | Shared resources/captures plus operator pass in Dark, available contrast, 150% text and both viewports | Final redesign acceptance remains UIR-09 |
| A10 | `current_local_pass`, `available_native_pass_after_fix` | Worker-owned combined totals/revision/preflight and bounded Review pages; operator passed Review meanings and corrected folder Open set | Final redesign acceptance remains UIR-09 |
| A11 | `current_local_pass`, `available_native_pass` | Loaded-STA checks plus operator keyboard, Narrator, contrast/text and 150%/175% monitor-transition pass | NVDA and physical 200% remain `unrun_unavailable` |
| A12 | `current_local_pass`, `available_native_pass` | Exact contextual Performance/warning navigation and bounds; operator passed current History/Progress returns | Final redesign acceptance remains UIR-09 |
| A13 | `current_local_pass` | Full Rust/.NET query/page/cache/update bounds, 100,000-group bounded regressions and WPF virtualization run in both configurations | Ten operator/performance profiles remain intentionally ignored; no new campaign authorized |
| A14 | `current_local_pass`, `available_native_pass` | Setup validation/races plus operator pass for Scan again setup, saved meanings and active-run blocking | Final redesign acceptance remains UIR-09 |
| A15 | `current_local_pass` | Full matrix and source audit retain disabled executor injection, `CanSubmit=false`, worker `executorEnabled=false` and no production execution action | Production execution remains out of scope |
| A16 | `current_local_pass`, `available_native_pass` | Controlled freshness/no-progress/disconnect/failure and exact diagnostics; operator passed current Progress details and Narrator behavior | No real long-duration observation was run |
| A17 | `current_local_pass`, `available_native_pass` | Strengthened five-run real-worker regression plus operator pass for Scan again/reuse/history interpretation | No physical/full-drive campaign was run |

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
- The available current-screen keyboard, Narrator, contrast-theme, text-size and 150%/175% monitor
  actions passed. No shipping WPF smoke, real long-duration scan, provider, representative performance
  or production-state action was run.

## Native action disposition

The prepared four-step batch is complete. Do not repeat it without a documented reopen condition.
UIR-09 final workflow acceptance and durable completion assessment are next. A real-worker native
A17 rescan, physical long-duration scan, provider/performance work and production execution remain
separate and are not prerequisites manufactured for UIR-09.

## Runtime and safety boundary

PID 67748 was absent. PID 63908 from the interrupted first attempt was found to have a valid titled
window only from the interactive/elevated context, confirming the earlier zero-handle result was a
cross-context visibility limitation rather than a fixture crash; it was replaced after explicit
operator authority. The initial walkthrough fixture PID 64588 was gracefully closed after the reported
defects. Corrected in-memory fixture PID 15072 was left open for the successful focused retest; re-audit
before reuse. The final non-interactive audit found PID 15072 responsive at the exact corrected-fixture
path; its title/handle were hidden again by the already diagnosed cross-context visibility limitation.
No Super Duper worker or production app was started.
No production database, cache, log, user file, recovery outcome or scan state was opened or changed.
`DisabledRecycleOperationCapabilityExecutor`, `CanSubmit=false`, `executorEnabled=false` and the
absence of a production execution action remain unchanged.
