# Current redesign session checkpoint

Updated 2026-09-12. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: **UIR-05b, the commit containing this checkpoint**, based on `f06dc03`
  (UIR-05a). UIR-00/01/02/03 complete; **UIR-04 and UIR-05 in_progress**.
- [UIR-05b evidence](evidence/uir-05b-file-comparison.md): adjustable 36/64 virtualized file
  list/detail comparison, single wrapping columns, server-owned sort and narrow set -> copies ->
  selected-copy navigation with Back to copies/sets. Exact path and Keep/Mark/Reset/Copy/Explorer
  actions use vertical-only scrolling. Decision refresh preserves selected-copy identity by ID;
  loading remains selection-neutral. UIR-05a Apply/Enter/chips/Clear, exact units, filters and
  accepted-query snapshot remain protected.
- Verification: full Core **207 passed**; **three loaded-STA WPF methods passed**. At 1180x760 the
  comparison measures **279.0/400.3 DIPs (69.7%)**; at 900x600 essential paths/decisions and Back/
  focus flows pass with zero horizontal scrolling. Fixture build: zero warnings/errors. Isolated
  results, captures and retained failures/corrections are under `artifacts/uir05b` and its evidence.
- **Exact next local slice: UIR-05c adjustable folder-results list/detail comparison and remaining
  folder decision/path verification.** Read S03 and directly linked relationship/decision/reveal/
  focus tests. Complete remaining local A04/A05/A06/A11/A15 without pulling UIR-06 forward.
  Preserve UIR-05a query semantics and UIR-05b file layout/navigation, selected run, decisions,
  bounded pages/cache and focus. UIR-05 remains open until folder evidence is recorded.
- Retain UIR-04a's isolated real-worker four-run rescan/restart/history result, UIR-04b freshness/
  terminal contracts and UIR-04c compact monitoring. UIR-04c's 200 Core/three WPF methods remain
  retained; current suites include their behavior. Full A17/integration/operator validation is later.
- Retain UIR-03 scoped acceptance, Narrator, Dark/Desert/live text-size, 150%/175% transition/focus/
  selection and prior paired Debug/Release plus Rust 226 passed/10 ignored. No campaign was rerun.
- NVDA and physical 200% remain `unrun_unavailable` for UIR-08/A11; do not troubleshoot Windows,
  force scaling or install software. Full native/user acceptance remains later.
- Runtime: PID 67748/session 1 re-audited responsive at the exact UIR-03f fixture executable and
  left untouched. UIR-04b/04c/05a and UIR-05b fixtures are built, not running. UIR-05b fixture build
  has zero warnings/errors. Re-audit before reuse and never
  overwrite running outputs. No production app/worker or state was touched; test windows close.
- Boundaries: production deletion disabled; engine/cache/protocol/query ceilings and survivor/
  revision/overlap protections unchanged. SOP10 complete; release validation parked; no campaign authority.
- Update evidence, plan/checkpoint/handoff, review/test and commit each coherent slice. At handoff,
  print a complete copyable continuation prompt tailored to the actual commit and next slice.
