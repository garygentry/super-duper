# Current redesign session checkpoint

Updated 2026-09-11. [Execution plan](execution-plan.md) owns gate status.

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
- **Exact next slice:** on the corrected UIR-03f fictional fixture, repeat Dark, Desert, then actual
  Windows Accessibility text size 100% -> 150% -> 100% while the app stays open, keeping display
  scale at 100%. Check Files rows/actions, Progress/Summary, warnings and Folders' empty message
  at both sizes. Record actual settings/results and restore original settings. The third supplied
  screenshot did not independently confirm the numeric text percentage. Desert is observed,
  not accepted. Retain unrelated passes; do not replay them without a reopen reason.
- Pending: corrected physical theme/text/contrast acceptance and physical 100/150/200% monitor-DPI
  observations. Offscreen 96-DPI renders and injected text events are not physical acceptance.
  UIR-03 cannot close on automation alone. Fix evidenced defects and assess it before UIR-04.
- Interim Files/Folders stacked scrolling layouts replace their split adjustments; final adjustable
  S03 comparison, compact search/totals and A03 no-horizontal-scroll/60% requirements stay UIR-05.
  Later folder/review/performance coverage and A08/A16/A17 monitoring/rescan work remain scheduled.
- Runtime: path-verified old fictional PID 71248 closed normally; corrected PID 67748/session 1 is
  open from `artifacts/uir03f/fixture`. Re-audit exact paths before reuse/launch. Use isolated
  `artifacts/uir03f` outputs. Production state untouched. No real worker scan/deletion performed.
- Boundaries: production deletion disabled; worker ownership, engine/cache/protocol/query ceilings
  and survivor/revision/overlap protections unchanged. SOP10 complete; Windows release stream parked.
