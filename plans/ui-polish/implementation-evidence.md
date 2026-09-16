# Polish implementation and verification

Branch: `codex/ui-redesign`. Implementation authorized after the accepted operator interview.
This record distinguishes real-worker journeys, rendered WPF checks and native input.

## Implemented scope

- Compact native navigation, collapsible saved scans, contextual Start/Scan again emphasis,
  quiet healthy footer, actionable connection recovery and context-aware completed-scan notice.
- Versioned local display preferences: selector, last Files/Folders mode and named disclosures.
  Private development state stays beside its private database; normal state is per-user.
- Locations-first setup, visible validation errors, compact policy disclosure and separate saved
  history management; summary-first monitoring, warnings and Performance.
- Shared accessible vector action icons, consistent file/folder binary-size input, compact copy
  decisions and responsive comparison navigation.
- Review totals and whole-plan check before details, Files/Folders decision switch, staged
  preferred-location editing/preview/application/reversal. Removal remains disabled.

## Defects caught during integration

| Finding | Cause and correction | Regression evidence |
|---|---|---|
| Detail expansion was not saved | WPF Loaded is a direct event; window event registration did not observe descendant sections. Traverse the declared logical tree, including inactive tab content, and attach each named section once. | Real-worker journey asserts active Setup and inactive History disclosures survive app recreation. |
| Selected tab made the whole page bold | FontWeight on TabItem inherited into its content. Remove the selected-item setter; keep the selected underline. | Actual Setup/Results captures before and after; retained keyboard/automation tab structure. |
| Enlarging text could leave the rail open | Width changes alone did not observe text scaling. Observe the window font dependency property too; preserve the user's stored preference while adapting. | Compact/wide journey checks; enlarged-text WPF matrix. |
| Remembered preference stage could hide confirmation | A restored collapsed stage could override workflow state. Keep the relevant Apply/Reverse stage open while its confirmation is active. | Staged preference surface checks and real rule journey. |
| Narrow copy grid became a sliver | Width-only breakpoint did not activate at a 900-pixel window, while paths/status consumed the detail pane. | Reopened UX04/UX05; responsive correction and strict usable-copy-row checks under verification. |

The extended real-data journey reproduced an engine readiness defect: a successful preflight was
still reported current after a subsequent live check found a marked file changed and invalidated
its working decision. `storage/preflight.rs` compared only manual review revisions; live validation
correctly preserves that revision and the immutable original scan. The bounded correction now
consults newer invalidating evidence for the exact checked targets/survivors,
without letting unrelated files stale the check or a later metadata-only observation revive it.
Affected watcher-overflow history also invalidates the check. Operation preparation and confirmation
use the same guard inside their existing immediate transactions. Seven targeted preflight storage
tests pass, including actual-file target/survivor changes, reconciliation, overflow, changes during
checking, immutable history, and stale operation admission. The timestamp unit test also passed.
Evidence: `artifacts/ui-dev-session/polish-journey-1e421f2a889f49be9f7e406f89bcf3b2`.

Test-harness assumptions about button text, hidden tabs, asynchronous rule readiness, rule-root
ordering and duplicate error text were corrected without relaxing product behavior requirements.

## Evidence so far

- Rust Debug workspace tests passed; explicit campaign/profile tests remain ignored.
- The full Debug workspace rerun after the final freshness/admission guard also passed.
- Core Debug: final rerun 224 passed (`artifacts/ui-polish-verification/core-final.trx`).
  Two queue-count tests now isolate elapsed display time using the existing manual clock;
  their exact queue assertions and real asynchronous progress-delivery scheduling remain intact.
  Infrastructure: 79 passed and five expected skips in sandbox, plus
  the remaining read-only eligibility test passed independently in the normal VM context (1/1).
- Actual locked-copy testing uses whole-plan content validation; metadata-only file-page
  validation can still observe metadata through a sharing lock and does not claim content access.
  Journey `5dacd0524f5b46f48b2c7e50f27fed84` passed changed/locked/missing/add/overlap cases;
  its final Stop/rescan setup assertion was premature while History was loading and now waits
  for the production command's readiness. This is not yet a full journey pass.
- Earlier real-worker baseline passed: 232 groups, over 200 members in one group, manual Files
  and Folders decisions, combined plan check, app/worker restart, unchanged source/corpus hashes.
  See `artifacts/ui-dev-session/polish-journey-a1c6e9fc4bce495cbff968c37e559696`.
  This run later failed a test-only rule-root ordering assumption; it is not a full journey pass.
- Earlier initial full journey passed at
  `artifacts/ui-dev-session/polish-journey-5958b25329674692a0bc1389c392ee2e`.
  Its transparent-background PNGs are invalid visual evidence and must not be used for acceptance.
- Fresh corpus now has 671 actual copies, including a ZIP built from actual tracked documents.
  Each run records hashes, source paths, archive-entry provenance and independent worker state.
  File mutations occur only in a separate disposable subtree.
- The latest actual Files narrow capture reopened UX04/UX05; earlier functional success does not
  establish adequate visual layout. Corrected captures and final matrix results will supersede it.

## Remaining acceptance

Latest completed verification supersedes earlier incomplete runs: WPF smoke passed 3/3 in
44 seconds, and real-worker journey `2a06e3b5820646c680fd5e7b5a4aa411` passed 1/1 in 73 seconds,
including the full Stop/cancel/rescan sequence. Debug solution build had zero warnings/errors.
Actual Light/Dark enlarged/narrow selected-copy captures were inspected. The final Review
singular/plural and concise-success-copy edit will be covered by the matching Release matrix.

Run the extended rules/filesystem-change journey against the corrected worker,
inspect corrected standard/narrow and theme captures, finish Debug/Release build/tests, and update
the issue closure ledger. Native mouse/keyboard acceptance remains unrun while the desktop is
locked. Background WPF control and real-worker evidence do not replace that final requirement.

## Final background acceptance (2026-09-15)

This section supersedes pending statements above; earlier incomplete runs remain diagnostic history.
Full Rust Debug and Release workspace tests pass with the final freshness/admission guard. Release
verification exposed two test timing assumptions: exact telemetry sequence ignored legitimate
heartbeat samples; a threaded coalescing test assumed producer calls fit in one emission slot.
Tests now allow those valid timings while asserting all five completed phases, zero flush errors,
exact counters, contiguous bounded frames, source revision and eventual latest substage. Production
behavior was unchanged by these final test corrections.

Windows Release build passed with zero warnings/errors; Core 224 passed, Infrastructure 80 passed
and five expected skips, WPF smoke 3/3 in 74 seconds. Artifacts are under
`artifacts/ui-polish-verification/release-final`. This covers final singular/plural and success copy.

The final fresh-state Release real-worker journey passed 1/1 in 23 seconds:
`artifacts/ui-dev-session/polish-journey-fe4440ba6f8a4767a7031676cde91baa`.
Its fresh real-file corpus is `polish-data-4ca8ef90e57e4966ad44f21cd111c2be` (671 copies).
Restart/preferences, scoped rule preview/apply/reverse, content locks, changed/missing/new files,
overlap, Stop/cancellation and successful rescan all passed. Narrow selected-copy, checked Review
(scrolled detail state) and final rescan captures were inspected; standard Review entry/readiness
is separately covered by the passing WPF matrix. No new product defect was found.
Standalone and packaged Release worker hashes match:
`F90D8A78565489E1C4F3B61BD7C85E0FFFC89E174A4E809D3BD049DC4F78A115`.
The journey records disposal of owned workers; final process/sidecar inspection found none remaining.

Only required native mouse/keyboard/dialog and physical high-contrast acceptance remains unrun.
Native window enumeration previously timed out and LogonUI PID 1220 remains present at final check.
P08 is blocked on an available desktop, not a new design decision. No unlocking/security workaround
was attempted. All independent implementation and background verification is complete.

## Native pass and follow-up (2026-09-16)

The operator confirmed desktop availability and continuing sandbox authority. Native capture/input
worked in Windows session 2 while LogonUI existed in session 1: that process alone was an incorrect
lock signal. list_apps exposed the app even when list_windows omitted it. Direct input opened the
folder picker and Escape returned, started scan 7 on the disposable mutation corpus, observed
completion navigation to Files, opened comparison, marked a copy, invoked Copy path and launched
Explorer at Backup documents. No files were deleted. Clipboard content and Explorer selected-file
verification remain unrun: Explorer capture returned `Computer Use app approval timed out`.
Subsequent app get_window_state and list_apps timed out; kernel reset and a fresh discovery attempt
also timed out. No further native calls were made. Owned app 7988/worker 10344 stopped and sidecar
removed. Final native acceptance remains incomplete because helper recovery failed, not because
operator authorization is missing. No unlock request is required to continue independent work.

ENG02: saved canonical `\\?\C:\...` roots appeared as unknown filesystem type and filled Setup with
redundant warnings. Classifier now strips the extended prefix only for a drive-root lookup, retaining
the original stored path. Extended UNC (including lowercase prefix) is recognized before potential
network reachability calls. Regression asserts local classification parity, preserved stored path,
no spurious warning and ordinary/extended UNC best-effort behavior. Verification follows below.

ENG02 verification: focused SessionDefinitionValidator tests passed 8/8 in Debug and 8/8 in Release.
Both full Windows solution builds passed with zero warnings/errors after the fix. Rust was unchanged.
The preceding full matrix remains valid for unchanged behavior; native after-check remains pending.
Final inspection found no owned SuperDuper app/worker and no Debug `.uidev` sidecar. Explorer was
opened by the authorized reveal action but not controlled or closed after its app-approval timeout.
