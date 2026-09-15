# Current redesign session checkpoint

Updated 2026-09-14. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest slice: **UIR-08 available-native walkthrough and acceptance-fixture correction, the commit
  containing this checkpoint**, based on `f356045` (UIR-08 local integration). **UIR-00 through
  UIR-08 are complete**; UIR-09 final user acceptance and durable completion package is next.
- [UIR-08 evidence](evidence/uir-08-integration-regression.md) records the operator's four-step batch.
  Steps 1 and 2 initially exposed fixed file decisions, empty folder Results and a no-op Review folder
  **Open set**. The in-memory fixture now retains worker-confirmed file/folder decisions and revisions,
  supplies 25 two-copy folder sets with local/UNC paths and supports exact Review-to-Folders links.
  The operator reported the corrected 1180x760 and 900x600 retest **all pass**.
- Step 1's Scan again/setup, active Progress details/Performance, corrected file/folder decisions and
  paths, Review/Location preferences and History identity/warnings/Performance passed. Step 2's narrow
  access passed. Step 3 Narrator and Step 4 Dark/available contrast/150%-text plus available 150%/175%
  monitor transitions passed; settings were restored. Computer Use crashed the ChatGPT host twice, so
  the operator completed the observations manually; this is not a Super Duper defect.
- NVDA and physical 200% remain `unrun_unavailable`, not passed or waived. Do not troubleshoot Windows,
  force scaling or install software. No real long-duration scan or physical/provider/performance/
  deletion campaign ran, and no consumed identity was replayed.
- Proportionate correction checks: the corrected fixture builds with zero warnings/errors. The focused
  loaded-STA WPF method's first run passed the new assertions but later hit the retained synthetic
  Enter-focus flake; a fresh isolated rerun passed. The prior full UIR-08 baseline remains Rust
  Debug/Release 226 passed/10 ignored and Windows Debug/Release 220 Core, 76 Infrastructure and three
  WPF methods with five explicit skips, plus 191 captures per configuration.
- Preserve UIR-04 through UIR-07 contracts: saved setup/reuse, honest long-scan state, bounded file/folder
  queries and decisions, worker-owned Review totals/revisions, Location preference provenance/reversal,
  highlighted/opened/active History identity and exact contextual warning/Performance return focus.
- Runtime: PID 67748 was absent. PID 63908's apparent missing window was a cross-context visibility
  limitation and it was replaced under explicit authority. Initial fixture PID 64588 was gracefully
  closed. Corrected in-memory fixture PID 15072 was left open after the successful retest; the final
  non-interactive audit found it responsive at the exact corrected path, with its title/handle hidden
  by the known cross-context limitation. Re-audit before reuse. No worker, production app, database,
  cache, log, user file or scan state was touched.
- Production safety remains unchanged: `DisabledRecycleOperationCapabilityExecutor`, `CanSubmit=false`,
  `executorEnabled=false`, no production execution action, and all recovery/SOP boundaries remain.
- **Exact next action: UIR-09 final user workflow acceptance and durable completion assessment.** Review
  the UIR-08 evidence and scope, confirm no open critical redesign defect, request explicit operator
  final acceptance, then update the final package and commit it on this branch. Do not repeat the
  completed native batch or manufacture physical/provider/performance/production work.
