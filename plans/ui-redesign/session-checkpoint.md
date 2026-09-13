# Current redesign session checkpoint

Updated 2026-09-13. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: **UIR-06b, the commit containing this checkpoint**, based on `143ee77`
  (UIR-06a). UIR-00/01/02/03 complete; **UIR-04, UIR-05 and UIR-06 in_progress**. UIR-04a/b/c,
  UIR-05a/b/c and UIR-06a/b are locally implemented.
- [UIR-06b evidence](evidence/uir-06b-location-preferences.md): the existing worker-backed workflow
  now opens from a focused **Location preferences** panel in Review. Saved ordered-root configuration,
  **Virtual preview — not saved decisions**, and saved rule-application provenance are distinct.
  Scope/count confirmation, rule/review revisions, preview signature, fresh-preview enforcement and
  later manual overrides remain. The exact operation is **Reverse rule application**; Reset semantics
  and every survivor/overlap/manual provenance rule are unchanged.
- Review can initialize/synchronize preferences without opening the Files workspace, while one shared view
  model retains current-filter/selected-set scope and same-run edits. UIR-06a worker-owned combined
  totals, separate 200-row Files/Folders pages, independent five-page caches, exact-set links,
  retained accepted pages and current/plan-changed/ready/blocked/needs-review states remain.
- Verification: full Core **216 passed**; **three loaded-STA WPF methods passed**; final isolated
  fixture build has zero warnings/errors. The focused panel is virtualized, vertical-only and
  reachable at 1180x760, 900x600 and toolbar stress. There are 111 reviewed captures under
  `artifacts/uir06b/captures-accepted-reviewed-final`; final TRX files are under
  `artifacts/uir06b/results`. Retained failed probes document the corrected viewport assertion and copy plural.
- **Exact next local slice: UIR-07a History/open-run and contextual warning composition.** Read S05,
  capability/validation guidance and directly linked history/open-run, warning-page, return-navigation
  and focus tests. Preserve selected-versus-active identity, immutable historical results, 500-run
  history fetch, 25-row/five-page warning bounds and current/terminal revisions. Do not pull
  Performance detail, recovery resolution, execution, full A17 or UIR-08 integration forward.
- Retain all UIR-04 monitoring/rescan and UIR-05 query/comparison/decision/navigation/reveal/focus
  contracts. **Check marked copies** remains whole-plan; **Check these copies** remains visible-page.
  No global flat plan, worker/protocol/page/cache/revision ownership or production state changed.
- Retain UIR-03 scoped acceptance, Narrator, Dark/Desert/live text-size, 150%/175% transition/focus/
  selection and prior paired Debug/Release plus Rust 226 passed/10 ignored. No campaign was rerun.
- NVDA and physical 200% remain `unrun_unavailable` for UIR-08/A11; do not troubleshoot Windows,
  force scaling or install software. Full A17/integrated/native/operator acceptance remains later.
- Runtime: PID 67748/session 1 was re-audited responsive at the exact older UIR-03f fixture executable
  and left untouched. UIR-04b/04c/05a/05b/05c/06a/06b fixtures are built, not running; all new outputs
  are isolated. No production app/worker or state was touched; test windows closed.
- Boundaries: production deletion disabled; `DisabledRecycleOperationCapabilityExecutor`,
  `CanSubmit=false`, `executorEnabled=false`, absent execution action, recovery/SOP boundaries and
  all safety locks unchanged. Update evidence/plan/checkpoint/handoff, review/test and commit each
  coherent slice; print a complete updated copyable continuation prompt at every handoff.
