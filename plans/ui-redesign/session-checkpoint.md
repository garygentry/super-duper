# Current redesign session checkpoint

Updated 2026-09-12. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: `5f83705` (UIR-03f; prior code `ae8d7d1`). UIR-00/01/02 complete;
  **UIR-03 remains in_progress**, UIR-03a/b/c/d/e/f implemented, UIR-04 dependent on acceptance.
- UIR-03f responds to operator screenshots showing unreadable Dark, Desert's empty-folder/header
  overlap, and title-only Windows text enlargement. Shared styles now inherit native Fluent
  templates and semantic dynamic brushes; a Windows UISettings source updates typography on the
  WPF dispatcher and is disposed on window close. Persistent context has full tooltips when elided.
  Folders uses a scrolling page with bounded native tables/cards and separate empty-state layout.
  File decision/path buttons wrap at enlarged sizes; warning action width accommodates its label.
  See [UIR-03f evidence](evidence/uir-03f-theme-text-and-empty-state.md).
- Focused Light/Dark x 100%/150% text x both-size checks pass, including an 80-DIP toolbar allowance,
  actual scroll-viewport bounds, action reachability, empty-header exclusion, focus/selection and
  live background-thread text events. Existing navigation/delayed-pane/scrollbar checks remain.
  Paired worker and Windows Debug/Release builds/tests pass: 170 Core, 75 Infrastructure/five skips,
  three WPF methods each (all shell assertions retained in the shared STA). Fixture build/--verify pass.
- Retained operator evidence: nine scoped walkthrough passes, accepted Progress/Summary scrollbar
  correction, and Narrator Files rows/active Progress/warning Close/return (labels, selection/status,
  focus return, no excessive repeat speech). Narrator executable version `10.0.22621.5262`.
  NVDA is not installed: `unrun_unavailable`, not passed or waived. No installation requested.
- **Operator recheck 2026-09-12:** "all 3 pass" confirms Dark readability, Desert empty-folder/header
  separation, and live Windows text size 100% -> 150% -> 100% with reachable buttons at both fixture
  sizes, display scale held at 100%. Close the three UIR-03f reported defects. Original OS-setting
  restoration was requested but not separately confirmed. Do not repeat these or earlier passes
  without a reopen reason. See the UIR-03f evidence for scope.
- **Display observation:** three monitors initially at 100%; only one accepts scaling changes.
  The operator confirms 150% and 175% were tested: the fixture adjusts fine across scaled/unscaled
  monitors with no noticeable clipping. These scoped transition/layout checks pass; 175% is extra
  coverage. Windows did not offer 200%: `unrun_unavailable`, not passed or waived. The operator
  excludes Windows troubleshooting; do not force custom scaling or investigate the OS limitation.
- **Exact next slice:** confirm file-selection/keyboard-focus retention across monitor moves,
  then assess UIR-03 with 200% and NVDA accurately recorded as unavailable/unrun. Do not infer
  complete physical-matrix acceptance or replace 200% with 175%. Restore original settings.
  UIR-03 stays in progress; UIR-04 remains dependent on its acceptance.
  This update is documentation only; no new code/build/test/runtime action.
- Interim Files/Folders stacked scrolling layouts replace their split adjustments; final adjustable
  S03 comparison, compact search/totals and A03 no-horizontal-scroll/60% requirements stay UIR-05.
  Later folder/review/performance coverage and A08/A16/A17 monitoring/rescan work remain scheduled.
- Runtime: path-verified old fictional PID 71248 closed normally; corrected PID 67748/session 1 is
  open from `artifacts/uir03f/fixture`. Re-audit exact paths before reuse/launch. Use isolated
  `artifacts/uir03f` outputs. Production state untouched. No real worker scan/deletion performed.
- Boundaries: production deletion disabled; worker ownership, engine/cache/protocol/query ceilings
  and survivor/revision/overlap protections unchanged. SOP10 complete; Windows release stream parked.
