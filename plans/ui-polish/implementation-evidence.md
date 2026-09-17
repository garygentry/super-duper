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

## Bounded native recovery from adeb9f5 (2026-09-16)

The checkout was clean at `adeb9f5` on `codex/ui-redesign`. Fresh `@oai/sky` initialization and
`list_apps` succeeded. The existing Debug build was launched with the validated `.uidev` sidecar
pointing at `polish-journey-2a06e3b5820646c680fd5e7b5a4aa411`; no production state was used.
The Infrastructure DLL SHA256 was
`20EC9F21210D18CFFD34AEA5BB64DF160FB4AE9C0EC6C8F0D24BAE6772FE9DC6`.

Native screenshot and accessibility evidence showed restored scan 7, both saved canonical roots,
enabled Scan again and no unknown-drive warnings. ENG02's native after-check therefore passes.
Clicking Add folder or drive opened the actual Windows chooser. However, targeting its returned
Folder field failed twice with `element 142 is not available in cached app state for
SuperDuper.Windows.exe`, including after a fresh accessibility observation. A screenshot-directed
click did not establish Folder focus (the helper reported Search Box); no path text was typed.
Alt+N followed by capture returned entirely black app and chooser frames. Fresh `list_apps` /
`get_window` selection and one JavaScript reset, reinitialization and fresh selection still returned
black frames. Discovery remained responsive. This is unavailable native capture/control evidence,
not proof that the app crashed or that the desktop was locked. No further native input was issued.

Folder selection, clipboard/reveal verification, Stop, Keep/Reset, Folders, Review/check/rules
confirmation/Escape, History/context return, keyboard/focus, resize/enlarged text and physical
high contrast remain unrun in this attempt. No new scan or file mutation occurred. Screenshots
were inspected directly in the Computer Use tool output; black frames are not visual acceptance.
No product source changed and no unchanged build/test matrix was replayed.

Owned app 6464 and worker 11540 were verified by executable path in session 2. Sandbox process
termination was denied; the normally privileged cleanup succeeded under existing authorization.
The Debug sidecar was removed. The previously opened Explorer window was not controlled.

## Native continuation from d784f48 (2026-09-16)

Clean checkout on `codex/ui-redesign`; preserved remote-tracking `origin/wpf-poc` remains
`deefa40ebe607b785b395a29a6282e8b417a9b14` (no local `wpf-poc` ref in this clone).
Fresh Computer Use discovery, launch and capture succeeded using the same isolated Debug state
`polish-journey-2a06e3b5820646c680fd5e7b5a4aa411`. Native screenshots below refer to directly
inspected tool output, not background WPF renders. The helper's accessibility snapshot often
lagged the screenshot by one action; settled re-observation and screenshot-derived targets were
used rather than assuming stale accessibility text was current.

- Folder chooser: screenshot-directed focus, typed disposable Backup documents path, Select Folder
  returned the exact added root to Setup. Removed that redundant setup entry and saved the original
  two-root configuration; no source file was changed. The helper reported Search Box focus despite
  the visible caret and successfully entered text in Folder.
- Scan 7 Files: Keep → Undecided (Reset) → Remove each visibly persisted for the backup overview.
  Copy path, native paste into the local search draft and Select All returned the exact complete
  canonical path, including `\\?\` and filename. Cleared the unsubmitted search draft afterward.
- Reveal: Explorer showed the correct Backup documents folder and highlighted Archived project
  overview, with one selected item. Its accessibility breadcrumbs identified the exact isolated
  journey tree. Alt+F4 closed this previously journey-owned Explorer window.
- Review: one marked file / 12.7 KB; explicitly confirmed Check completed with both validation
  items Ready and no changed/missing/unavailable/conflict items. Removal remained unavailable.
- Baseline scan 6 Folders: selected Backup archive/apps, read descendant scope, and observed
  Keep → Undecided → Remove. No filesystem deletion occurred.
- Keyboard Alt+H opened History with visible focus. Highlighting scan 1 left workspace scan 6
  unchanged; recorded locations expanded successfully. Performance showed highlighted scan 1;
  Return restored the same History selection and settled focus to OpenHighlightedPerformance.
  These scans have zero warnings and their Review warnings action was disabled.
- UX21 reproduction: Escape did not dismiss the native Yes/No Check marked copies dialog,
  confirmed by another observation. Shared confirmation changed to YesNoCancel, retaining default
  No and `result == Yes` admission. App and worker exited normally before rebuilding. Native
  after-check and remaining journeys follow in the next evidence entry.

## Reviewed fixes and native continuation (2026-09-16, from d784f48 plus preserved edits)

The preserved pending changes were reviewed independently. Preference comparisons now normalize
ordinary/extended DOS and UNC root spellings in validation, preview, missing-root reporting and
application, retain stored spelling, and give the first equivalent legacy rule root its original rank.
Review caught repeated trailing drive separators losing the drive-root slash; corrected with a
regression and negative checks for drive-relative, device namespace and Unix-style identities.
Shared native confirmation uses Yes/No/Cancel with No default and affirmative-only admission.
Inline Apply/Reverse routes Escape to the existing cancel command and focus restoration.

Before rebuilding, owned app 5380 and child worker 15012 were audited by executable path and parent,
stopped, and their Debug `.uidev` sidecar removed. Debug root storage regressions passed 4/4;
the normalization boundary unit test passed 1/1. Matching Debug worker and full Windows build passed
with zero Windows warnings/errors. Fresh background real-file journey passed 1/1 in 25 seconds at
`artifacts/ui-dev-session/polish-journey-e4149564922d459cb1761b190b1d8a09`, corpus
`polish-data-ca1a860ff5594311a8e09e6a03600c3c`. It asserts ordinary-root preferred rank, routed Escape,
unchanged cancelled decisions, exact application identity, restored focus, apply/reverse, restart,
mutations, Stop and rescan. This is loaded-STA real-worker evidence, not native input.
Debug worker SHA256: `ACA08E756369F4C54157E033CDC85C28DFDAB3887D2A9C4B46D81D341DC49471`.

Native Computer Use launched that freshly verified private state with app 10692/worker 12796.
Screenshots below were directly inspected tool output. Accessibility snapshots sometimes lagged one
action; settled observations verified focus rather than treating stale text as current.

- Scan 4 rule preview chose ordinary-path Current documents above canonical Backup documents,
  proposing one Keep and one Remove. Apply opened an exact scope confirmation; Escape dismissed it
  and settled focus returned to Apply. Enter reopened it, Tab reached Confirm, Enter applied it.
  Review visibly changed from two undecided copies to one marked and one kept.
- Reverse application 2 displayed its exact one-Keep/one-Remove scope. Escape dismissed it with
  decisions retained and settled focus on Reverse. Reopening and confirming restored two undecided
  copies and disabled Reverse. ENG03 and UX22 native after-checks pass.
- Marked the disposable backup overview, opened Check, observed Yes/No/Cancel with No default.
  Native Escape closed it, focus returned to Check, and the plan remained not checked. UX21 passes.
- Alt+S opened Scan with visible focus. To leave enough time for native cancellation, copied the
  existing built worker binary into the disposable mutation root, first 80 then 240 total copies
  (26,062,336 bytes each); source hash is retained in `native-stop-source.json`. Scan 7 completed
  in 12 seconds before the first cancellation input. Scan 8 received native Alt+C while hashing,
  showed Cancelling and then Cancelled (10 seconds). This is functional Stop evidence, not a drive
  performance campaign. The 240 explicitly owned copies were removed after app shutdown to recover
  disk space; those historical runs now refer to intentionally removed disposable copies.
- System-menu Size and native drag reached 900×600 (886×593 client capture); rail collapsed and
  Progress retained readable status/actions with vertical scrolling. Alt+H reached History, selecting
  completed scan 7 left opened scan 8 unchanged, and Alt+O explicitly opened scan 7. Settled compact
  Results displayed multiple meaningful copy rows, Back and checking/paging controls without overlap.
  A transient loading/empty overlay overlap was observed and recorded as UX23 for correction.
- Windows Settings launch returned `launched app did not expose a targetable window`; fresh apps
  and windows enumeration exposed only the app and Codex. Native contrast shortcut produced no
  theme change or targetable dialog. The normal Control Panel route then returned
  `Computer Use app approval timed out`. Native input stopped; no security or unlock workaround.
  Physical enlarged text and high contrast remain unrun; existing background theme evidence is not
  substituted. Control Panel/Settings were not subsequently controlled.

Owned app 10692/worker 12796 were re-audited, stopped, and sidecar removed. Follow-up process and
sidecar audit found none. No merge, push, release or production deletion was performed.
Workspace-wide formatting check reports pre-existing unrelated formatting differences; only the
two changed Rust files were formatted. Final Release results are recorded below.

Final verification for this slice:

- Focused Rust storage 4/4 and namespace/relative-root unit regression 1/1 pass in Debug and Release.
  Matching workers built in both profiles; unchanged full-workspace baseline was not replayed.
- Full Windows Debug and Release solution builds pass with zero warnings/errors, including UX23.
  Core tests pass 228/228 in each configuration. The delayed-response test models a bound empty
  value and verifies it is notified/hidden while loading and reappears for a truly empty response.
- Release WPF regression passes 3/3 in 48 seconds; artifacts under
  `artifacts/ui-polish-verification/p08-fixes`. Narrow selected-path render was inspected.
- Fresh final Release real-worker journey passes 1/1 in 21 seconds at
  `artifacts/ui-dev-session/polish-journey-f97c5e0da8234bb38bf50bef4f2915f7`, using fresh corpus
  `polish-data-2f5a4220ab1944d48632faf45c401283`. Includes ranking, both routed Escape cancellations,
  application/reversal, unchanged decisions, focus, restart, mutations, Stop and rescan.
  Rule-stage PNGs sample Fluent expander transitions and are not settled visual acceptance;
  the directly observed settled native stages above establish the corresponding visual result.
- Standalone/packaged Release workers match SHA256
  `583C33CC6F18D26F1171A668AA89D8EB5525EB6935D1DA4701A70A0C97831883`.
- Independent final review approved the source changes and preserved the native acceptance boundary.
  P08 remains open for physical 150% text/high contrast, remaining keyboard/focus/compact checks,
  and UX23's native after-check. No native success is inferred from these background results.
