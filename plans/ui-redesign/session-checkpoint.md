# Current redesign session checkpoint

Updated 2026-09-08. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: this UIR-03c commit, parent `c91c458`. Resolve its hash from Git.
- Completed gates: UIR-00/01/02. **UIR-03 remains in_progress**; UIR-03a/b/c are implemented.
- UIR-03c: shared screen typography/insets/fields/actions/rows/cards, bounded long-name headers,
  interim Files filter/totals disclosures, actual MainWindow focus generation guards and
  View progress focus. Public production worker ownership and shutdown remain unchanged.
- The populated fixture uses shipping MainWindow/views and fake services: old completed run,
  independent active run, long names, 25 file groups/two copies and warnings. Actual programmatic
  keyboard focus, warning close/result return, file selection and nonzero file/progress scroll
  retention, one-query same-run reuse and delayed optional-pane failure pass. A standalone desktop
  host shares the data; offscreen startup/shutdown passed. No physical desktop walkthrough ran.
- Debug/Release matrix passed: Rust build/test (226 passed, 10 ignored each); Windows build/test
  (170 Core, 75 Infrastructure with five operator-only skips, four WPF test methods each).
  Final WPF resource/focus changes also passed focused Debug/Release reruns. See
  [UIR-03c evidence](evidence/uir-03c-populated-shell.md) for commands, failures and limits.
- **Exact next slice: UIR-03d, local_code** — fix the composed shell viewport blockers before
  A01/A02/A09 desktop acceptance. Files can lose its grids at 900x600; normal-size comparison and
  narrow History remain cramped. Preserve the 900x600 minimum and shared readable sizes; verify
  reachable rows/actions and focus/scroll retention. Keep full S03 Results redesign in UIR-05.
  Use [the prepared walkthrough](evidence/uir-03-desktop-walkthrough.md) and
  `scripts/Invoke-UiRedesignFixture.ps1` (build only by default; `-Show` explicitly opens it).
- Limits: no A03 layout pass, physical keyboard, Narrator/NVDA, light/dark/high contrast, Windows
  text enlargement or physical monitor DPI acceptance. Fixture captures are 96-DPI system-brush
  renders; folder/review/performance representative data still need later coverage. UIR-03 cannot
  close on automated evidence alone. UIR-04 monitoring/Scan again follows UIR-03; A08/A16/A17 remain.
- Runtime: operator WPF 36316 and worker 17612 stayed at `artifacts/windows-x64`; untouched.
  Outputs/logs/TRX/captures/temp state are ignored under `artifacts/uir03c`. Owned integration
  workers and offscreen fixture windows exited. Re-audit processes next session.
- Boundaries: production deletion disabled; engine/worker/cache/protocol/query ceilings unchanged;
  persistent qualified reuse and long-scan requirements retained. SOP10 consumed/complete;
  Windows post-MVP release validation parked. No external blocker to the next local layout slice.
