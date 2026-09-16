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
