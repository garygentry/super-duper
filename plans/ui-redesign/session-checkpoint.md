# Current redesign session checkpoint

Updated 2026-09-12. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: `5f83705` (UIR-03f; prior `ae8d7d1`). **UIR-00/01/02/03 complete; UIR-04 ready,
  not started.** [Scoped shell acceptance](evidence/uir-03-shell-acceptance.md) maps evidence and limits.
- Operator confirms file-group selection survived monitor moves, after confirming focus remained.
  Available shell checks pass: nine earlier scoped checks, scrollbar correction, Narrator, corrected
  Dark/Desert empty layout/live 100% -> 150% -> 100% text changes, and display transitions at
  150%/175% without noticeable clipping. Do not replay accepted checks without a reopen reason.
- **Unrun requirements retained for UIR-08/A11:** NVDA is not installed; Windows did not offer 200%.
  Both remain `unrun_unavailable`, not passed or waived; 175% does not replace 200%. Full native
  screen/reader/theme/DPI and long-scan acceptance are not supplied by this shell fixture. The
  operator excludes Windows troubleshooting; do not force custom scaling or install software.
- UIR-03f fixes native Fluent inheritance/semantic brushes, Windows UISettings-driven text resources,
  empty-folder/header overlap and enlarged action layout. Paired worker and Windows Debug/Release
  integration passed: 170 Core, 75 Infrastructure/five operator-only skips, three WPF methods each
  (all original shell assertions retained in the shared STA). Fixture build/--verify passed.
  UIR-03c Rust tests remain 226 passed/10 ignored, not rerun. This closure is documentation only.
- **Exact next slice: UIR-04a setup and Scan again (A06/A14/A17).** Read the startup route, execution
  plan, selected screen specification and `scan-and-rescan-experience.md`, then linked code/tests.
  Open current saved setup through Scan again; explain qualified persistent hash reuse versus
  re-reading candidate content; save valid edits on Start and create a new dated run while preserving
  earlier immutable results. Preserve dirty-edit and single-active-run contracts. Use focused Core/
  loaded-STA tests and isolated outputs; any real-worker rescan fixture must be small, nonpersonal,
  with separate persistent cache/history. Subsequent UIR-04 slices cover A08/A16 long-scan monitoring.
- Interim Files/Folders stacked scrolling replaces split adjustments; full adjustable S03, compact
  search/totals and A03 no-horizontal-scroll/60% remain UIR-05. Review/performance and remaining
  A08/A16/A17 coverage retain their later gates. Redesign/user acceptance is not complete.
- Last runtime: fictional PID 67748/session 1 at `artifacts/uir03f/fixture`, path-verified responsive
  during this walkthrough; old PID 71248 closed normally. Re-audit before reuse/launch; do not
  overwrite running outputs. Production state untouched. No real scan/deletion in this closure.
- Boundaries: production deletion disabled; ownership, engine/cache/protocol/query ceilings and
  survivor/revision/overlap protections unchanged. SOP10 complete; Windows release stream parked.
- Every coherent slice needs evidence, synchronized plan/checkpoint/handoff, review and commit.
  At session handoff, print the updated copyable continuation prompt from the session guide.
