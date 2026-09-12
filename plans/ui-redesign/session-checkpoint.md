# Current redesign session checkpoint

Updated 2026-09-12. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: **UIR-05a, the commit containing this checkpoint**, based on `9c528fb`
  (UIR-04c). UIR-00/01/02/03 complete; **UIR-04 and UIR-05 in_progress**.
- [UIR-05a evidence](evidence/uir-05a-file-query-controls.md): exposed compact path/Apply/Filters/
  Clear and three filtered totals; removable applied chips; exact B/KiB/MiB/GiB/TiB size conversion.
  All existing filters remain. Paging, facet sorting and rule scope use the accepted query snapshot;
  drafts enter only through Apply/Enter. Delayed/failing replacement retains rows/totals/chips;
  Clear restores default sorting. Late queries, focus and durable decisions remain protected.
- Verification: full Core **207 passed**; **three loaded-STA WPF methods passed**. Isolated results,
  captures and retained failures are in `artifacts/uir05a` and its evidence. Native long-path/delay/
  Enter/chip/Escape/sort fixtures and retained theme/text-size/toolbar/monitoring/UIA checks run.
  The header uses ordinary page scrolling to preserve enlarged-text comparison access; controls
  and totals are outside disclosures. Full A03 comparison/layout measurement is not passed.
- **Exact next local slice: UIR-05b adjustable file-results list/detail comparison and full A03.**
  Read S03 and directly linked view/comparison/focus tests. Measure >=60% usable list/detail height
  at 1180x760; at 900x600 retain essential paths/decisions without horizontal scroll, with narrow
  list/detail and Back to sets where needed. Preserve UIR-05a query snapshots, units/chips, selected
  run, decisions, bounded pages/cache and focus. UIR-05 folder/decision/path work remains open.
- Retain UIR-04a's isolated real-worker four-run rescan/restart/history result, UIR-04b freshness/
  terminal contracts and UIR-04c compact monitoring. UIR-04c's 200 Core/three WPF methods remain
  retained; current suites include their behavior. Full A17/integration/operator validation is later.
- Retain UIR-03 scoped acceptance, Narrator, Dark/Desert/live text-size, 150%/175% transition/focus/
  selection and prior paired Debug/Release plus Rust 226 passed/10 ignored. No campaign was rerun.
- NVDA and physical 200% remain `unrun_unavailable` for UIR-08/A11; do not troubleshoot Windows,
  force scaling or install software. Full native/user acceptance remains later.
- Runtime: PID 67748/session 1 re-audited responsive at the exact UIR-03f fixture executable and
  left untouched. UIR-04b/04c/05a fixtures are built, not running. UIR-05a fixture build has zero
  warnings/errors. Re-audit before reuse and never
  overwrite running outputs. No production app/worker or state was touched; test windows close.
- Boundaries: production deletion disabled; engine/cache/protocol/query ceilings and survivor/
  revision/overlap protections unchanged. SOP10 complete; release validation parked; no campaign authority.
- Update evidence, plan/checkpoint/handoff, review/test and commit each coherent slice. At handoff,
  print a complete copyable continuation prompt tailored to the actual commit and next slice.
