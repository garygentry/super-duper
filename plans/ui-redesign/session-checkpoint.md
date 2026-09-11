# Current redesign session checkpoint

Updated 2026-09-11. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: `b20341a` (UIR-03d). This update records operator evidence from `deeedb4`.
- Completed gates: UIR-00/01/02. **UIR-03 remains in_progress**; UIR-03a/b/c/d are implemented.
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
- Verification: paired Rust worker builds and Windows Debug/Release builds/tests pass: 170 Core,
  75 Infrastructure with five operator-only skips, four WPF methods each. No Rust/shared contract
  changed; UIR-03c's 226-passed/10-ignored Rust tests remain the retained baseline, not a new run.
  Standalone launcher build-only and hidden `--verify` pass. See
  [UIR-03d evidence](evidence/uir-03d-viewport-access.md) for commands, captures, failures and tradeoffs.
- **Operator evidence:** after a requested fixture reset and orientation, the operator reported
  "All 3 checks pass": reviewed/active context and harmless History highlighting, delayed-folder
  responsiveness/error isolation, and file selection/scroll retention plus scoped layout at both
  fixture sizes. Visibility is established. See [scope and limits](evidence/uir-03-desktop-walkthrough.md#2026-09-11-initial-operator-checks).
- **Exact next slice:** explicit Open scan round trips, warning-return/result focus and Progress
  scroll retention; then full expanded-action/shared-style, physical keyboard and accessibility
  observations in [the walkthrough](evidence/uir-03-desktop-walkthrough.md). The first three checks
  do not accept the whole walkthrough. Do not repeat the passed subset without a reason.
  Record actual observations, fix evidenced defects and assess UIR-03 before advancing to UIR-04.
- Unrun: physical keyboard, Narrator/NVDA, light/dark/high contrast, Windows text enlargement and
  physical 100/150/200% monitor/DPI checks. Renders are 96-DPI system-brush fixtures, not physical
  acceptance. Representative folder/review/performance coverage remains in later gates. UIR-03
  cannot close on automation alone. UIR-04 monitoring/Scan again follows; A08/A16/A17 remain required.
- This session: documentation/evidence only; no product/test changes or build/test reruns.
  Verification: documentation link validation, final diff review and `git diff --check`.
- Runtime: process checks found no matching app/worker/fixture before launches. Existing fixture
  output was launched as 70232, then reopened fresh as 58144/session 1 for the requested reset
  after another check found no matching process. No process was stopped or production state used.
  Output remains `artifacts/uir03-desktop-fixture`; prior logs/temp: `artifacts/uir03-operator`.
  Re-audit before reuse/launch; use the documented launcher if a new build is needed.
- Boundaries: production deletion disabled; ownership/engine/worker/cache/protocol/query ceilings
  and survivor/revision/overlap protections unchanged. SOP10 consumed/complete; Windows post-MVP
  release validation parked. The remaining UIR-03 prerequisite is operator desktop evidence.
