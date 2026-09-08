# Current redesign session checkpoint

Updated 2026-09-08. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: this UIR-03d commit, parent `aef8839`. Resolve its hash from Git.
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
- **Exact next slice: UIR-03 desktop acceptance — operator A01/A02/A09 walkthrough.** The local
  viewport prerequisite is verified. Use [the prepared walkthrough](evidence/uir-03-desktop-walkthrough.md)
  and `scripts/Invoke-UiRedesignFixture.ps1 -Show`; the launcher builds only without `-Show`.
  Request/record the operator's actual context, delayed-pane, focus, retention and layout observations.
  Assess UIR-03 acceptance from evidence; do not advance to UIR-04 prematurely.
- Unrun: physical keyboard, Narrator/NVDA, light/dark/high contrast, Windows text enlargement and
  physical 100/150/200% monitor/DPI checks. Renders are 96-DPI system-brush fixtures, not physical
  acceptance. Representative folder/review/performance coverage remains in later gates. UIR-03
  cannot close on automation alone. UIR-04 monitoring/Scan again follows; A08/A16/A17 remain required.
- Runtime: operator WPF 36316 and worker 17612 at `artifacts/windows-x64` remained untouched.
  Task outputs/state/logs/TRX/captures are ignored under `artifacts/uir03d`; standalone output is
  `artifacts/uir03-desktop-fixture`. Owned test workers and hidden fixture PID 48404 exited.
  Re-audit processes next session; do not reuse old PIDs as authority.
- Boundaries: production deletion disabled; ownership/engine/worker/cache/protocol/query ceilings
  and survivor/revision/overlap protections unchanged. SOP10 consumed/complete; Windows post-MVP
  release validation parked. The remaining UIR-03 prerequisite is operator desktop evidence.
