# Current redesign session checkpoint

Updated 2026-09-11. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: `ae8d7d1` (UIR-03e; baseline `8d2354c`, prior code `b20341a`).
- Completed gates: UIR-00/01/02. **UIR-03 remains in_progress**; UIR-03a/b/c/d/e are implemented.
- UIR-03e fixes the operator-reported Progress/Summary outer-scrollbar overlap by moving the
  existing page inset inside the shared ScrollViewer, preserving the empty-state inset. The
  regression fails before the fix (12-DIP overlap) and passes for both tabs, top/bottom, both sizes
  and minimum size with toolbar allowance. Operator confirms "Scroll overlap looks good now";
  this defect is closed. See [UIR-03e evidence](evidence/uir-03e-scan-scrollbar-clearance.md).
- UIR-03d: scrolling Files/History pages with bounded native grids, full-width comparison context,
  wrapping commands and minimum widths for complete decision/path/warning actions. The 900x600
  minimum and shared readable resources remain. Interim stacked Files replaces its split adjustment;
  full adjustable S03, compact search/totals and A03 no-horizontal-scroll/60% requirements stay UIR-05.
- Corrected populated fake-service fixture includes displayed long relative paths and selected roots,
  25 groups/two copies, independent old/active runs and warnings. At 1180x760, 900x600 and 900x600
  with 80-DIP toolbar allowance, tests verify clipped row/action bounds, real Next set/Validate page
  focus handlers, warning focus/return, selected group and nonzero page/grid scroll retention.
  Existing progress-scroll, one-query same-run reuse, stale-focus and delayed-pane checks remain.
  Disclosure layout settles before exact offset comparisons; failed evidence is retained.
- Latest verification (UIR-03e): paired Rust worker builds and Windows Debug/Release builds/tests pass: 170 Core,
  75 Infrastructure with five operator-only skips, four WPF methods each. No Rust/shared contract
  changed; UIR-03c's 226-passed/10-ignored Rust tests remain the retained baseline, not a new run.
  Corrected standalone fixture build passes; isolated output/logs/captures are `artifacts/uir03e`.
- **Operator evidence:** after a requested fixture reset and orientation, the operator reported
  "All 3 checks pass": reviewed/active context and harmless History highlighting, delayed-folder
  responsiveness/error isolation, and file selection/scroll retention plus scoped layout at both
  fixture sizes. The second batch also passed: explicit Open scan round trips, active warning
  Close/return focus and Progress scroll retention. The third batch passed completed-run warning
  result focus and the keyboard-only journey (eight scoped passes), but reported the Progress/
  Summary scrollbar overlap. After its correction, "other styling looks good" passes the interrupted
  expanded-controls/shared-style check (nine scoped passes plus the scrollbar correction).
- Narrator: operator reports the Files rows, active Progress and warning Close/return checks pass,
  including labels/selection/status, focus return and no excessive repeated speech. Installed
  Narrator executable version: `10.0.22621.5262` (read-only metadata). NVDA availability unanswered.
- **Exact next slice:** establish NVDA availability and repeat the same reader journey, then complete remaining physical accessibility
  observations in [the walkthrough](evidence/uir-03-desktop-walkthrough.md). Original-defect environment:
  both sizes, standard Windows theme, 100% display scaling. Scrollbar correction is operator-verified;
  separate text size and remaining reader/theme/DPI checks are pending. Retain nine scoped passes.
  Record actual observations, fix evidenced defects and assess UIR-03 before advancing to UIR-04.
- Unrun: NVDA, light/dark/high contrast, Windows text enlargement and
  physical 100/150/200% monitor/DPI checks. Renders are 96-DPI system-brush fixtures, not physical
  acceptance. Representative folder/review/performance coverage remains in later gates. UIR-03
  cannot close on automation alone. UIR-04 monitoring/Scan again follows; A08/A16/A17 remain required.
- Runtime: after integration passed, old fictional fixture 58144 was path-verified and closed
  normally. Corrected fixture PID 71248/session 1 is open from `artifacts/uir03e/fixture`; the
  scrollbar recheck and remaining styling passed. Re-audit before reuse/launch.
  Production state untouched; this confirmation update changes documentation only.
- Boundaries: production deletion disabled; ownership/engine/worker/cache/protocol/query ceilings
  and survivor/revision/overlap protections unchanged. SOP10 consumed/complete; Windows post-MVP
  release validation parked. The remaining UIR-03 prerequisite is operator desktop evidence.
