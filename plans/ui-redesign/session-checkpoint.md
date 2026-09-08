# Current redesign session checkpoint

Updated 2026-09-08. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserved baseline: `wpf-poc` at `deefa40`.
- Latest implementation: the UIR-03b commit containing this checkpoint, parent `9c028ca`;
  previous implementation `3775d42` (UIR-03a). Resolve the latest hash from Git.
- Completed gates: UIR-00/01/02. **UIR-03 remains in_progress**, with UIR-03a/b implemented.
- UIR-03b: four primary areas (Scan / Results / Review / History), semantic subnavigation,
  explicit Open scan independent of highlighted History, and demand-loaded Files/Folders/Review/
  Performance retaining one run's bounded state. Stopped-run summaries remain independent of
  global active progress/cancellation. Current warnings can change saved-scan/history context
  without retargeting the opened workspace; the header names that workspace and Start names its
  saved-scan target. Late file-rule and Review loads/revision errors cannot overwrite another run.
- Verification: 170 Core tests and four WPF tests pass in isolated Debug outputs; the WPF build
  succeeds. Shell-only 1180x760/900x600, 96-DPI renders inspected. The loaded shell test now uses
  the actual ShellViewModel with fake services, tests semantic regrouping/reordering, Open scan,
  logical focus and active progress. See [evidence](evidence/uir-03b-workspace-navigation.md).
- **Exact next slice: UIR-03c, local_code** — finish shared resources on the composed primary
  screens; build a populated isolated shell fixture for A01/A02/A09 and verify actual shell
  focus handlers, warning return, same-run pane reopen/scroll retention, long names and narrow
  layout. Prepare the desktop walkthrough and the integration acceptance boundary. UIR-04
  monitoring/Scan again follows UIR-03; A08/A16/A17 remain required.
- Limits: screen contents largely retain their existing presentation. Shell captures use empty
  screen slots and system theme defaults. Actual keyboard, Narrator/NVDA, theme/high-contrast,
  physical DPI, populated shell walkthrough and integrated Debug/Release Rust/.NET matrix remain
  pending. No native acceptance or UIR-03 completion claimed.
- Runtime: operator WPF PID 36316 and worker PID 17612 remain at `artifacts/windows-x64`.
  No interruption, production database access or real scan. Task outputs/TRX/captures are ignored
  under `artifacts/uir03b`; fixture windows close after tests. Re-audit processes next session.
- Boundaries: production deletion disabled; worker/cache/protocol/collection ceilings unchanged;
  persistent qualified rescan and long-scan requirements retained; no pause/resume or run-content
  diff; SOP10 consumed and complete, Windows release validation parked.
- Open local blockers: none. Continue on this checkout/branch using the [session guide](codex-session-guide.md).
