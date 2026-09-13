# Current redesign session checkpoint

Updated 2026-09-13. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: **UIR-05c, the commit containing this checkpoint**, based on `9340668`
  (UIR-05b). UIR-00/01/02/03 complete; **UIR-04 and UIR-05 in_progress** with their scheduled local
  implementation slices complete.
- [UIR-05c evidence](evidence/uir-05c-folder-comparison.md): adjustable 36/64 virtualized folder
  set/copy comparison, single wrapping columns and narrow set -> copies -> selected-copy navigation.
  Complete paths, exact relationship/descendant scope, Keep/Mark/Reset/Copy/Explorer, bounded reveal
  and Back/focus behavior use vertical-only scrolling. Selection is neutral; confirmed decisions
  preserve selected-copy identity. Draft/applied folder queries, Clear/default sort and distinct
  empty/loading/error/unavailable states are truthful. UIR-05a/b remain protected.
- Verification: full Core **210 passed**; **three loaded-STA WPF methods passed**; fixture build has
  zero warnings/errors. The loaded fixture verifies adjustable wide layout, narrow navigation,
  long local/fictional UNC paths, all actions, toolbar stress, Light/Dark and 150% text enlargement.
  There are 105 accepted captures under `artifacts/uir05c/captures-accepted-reviewed`; accepted TRX files and
  retained corrective iterations are under `artifacts/uir05c/results`.
- **Exact next local slice: UIR-06a dedicated Review overview and revision-aware validation status.**
  Read S04, capability/validation guidance and directly linked review-plan, review-group, preflight
  revision/survivor/overlap and return-navigation tests. Compose worker-owned combined totals with
  separate bounded Files/Folders review queries and exact-set return links. Make the non-deleting
  notice and stale/current validation state explicit. Do not pull rule authoring/reversal, execution,
  recovery resolution or UIR-07 forward.
- Retain UIR-04a rescan/restart/history evidence, UIR-04b freshness/terminal contracts, UIR-04c compact
  monitoring and all UIR-05 query/comparison/decision semantics. Full A17/integration/operator
  validation remains later. UIR-06 must preserve 200-row pages, existing cache ceilings and revision
  ownership; it must not materialize a global flat plan.
- Retain UIR-03 scoped acceptance, Narrator, Dark/Desert/live text-size, 150%/175% transition/focus/
  selection and prior paired Debug/Release plus Rust 226 passed/10 ignored. No campaign was rerun.
- NVDA and physical 200% remain `unrun_unavailable` for UIR-08/A11; do not troubleshoot Windows,
  force scaling or install software. Full native/user acceptance remains later.
- Runtime: PID 67748/session 1 re-audited responsive at the exact older UIR-03f fixture executable
  and left untouched. UIR-04b/04c/05a/05b/05c fixtures are built, not running; UIR-05c used isolated
  outputs. Re-audit before reuse and never overwrite running outputs. No production app/worker or
  state was touched; test windows close.
- Boundaries: production deletion disabled; engine/cache/protocol/query ceilings and survivor/
  revision/overlap protections unchanged. SOP10 complete; release validation parked; no campaign authority.
- Update evidence, plan/checkpoint/handoff, review/test and commit each coherent slice. At handoff,
  print a complete copyable continuation prompt tailored to the actual commit and next slice.
