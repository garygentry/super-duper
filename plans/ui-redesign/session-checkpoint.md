# Current redesign session checkpoint

Updated 2026-09-14. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: **UIR-07b, the commit containing this checkpoint**, based on `32d0ad0`
  (UIR-07a). UIR-00/01/02/03 complete; **UIR-04, UIR-05, UIR-06 and UIR-07 in_progress**.
  UIR-04a/b/c, UIR-05a/b/c, UIR-06a/b and UIR-07a/b are locally implemented.
- [UIR-07b evidence](evidence/uir-07b-contextual-performance.md): one action from highlighted History,
  active Progress or stopped-run Summary loads the exact product-run-verified Performance snapshot.
  Close restores exact origin focus without retargeting an older opened Results/Review run. Abandoning
  contextual detail through the tabs clears its temporary run override; direct Performance uses the
  stable opened run.
- Performance composes at most six phase rows, partial/full cache and actual-read summaries, explicit
  unavailable host/device fields, one selected-device current/peak detail from 64 virtualized rows and
  25 qualified comparison rows. Differing volume/device, scan-input or build context is not like-for-like.
  Persisted summaries are explicitly not raw samples or time-series data.
- Verification: full Core **220 passed**; **three loaded-STA WPF methods passed**; isolated fixture build
  has zero warnings/errors. There are 125 captures under `artifacts/uir07b/captures-final-2`; representative
  exact context, device and comparison states were visually reviewed. Final TRX files are under
  `artifacts/uir07b/results-final`. Retained failed probes were not overwritten.
- **Exact next local slice: UIR-08 local integration regression and acceptance-matrix preparation.**
  Read the UIR-08 ledger row, A01-A17 matrix, current native evidence and linked integrated Debug/Release,
  layout/theme/keyboard/accessibility/scale and long-scan/rescan procedures. Run locally available
  automated checks and inventory remaining evidence honestly before requesting any separate native action.
  Do not infer NVDA/physical 200% availability, troubleshoot Windows or start physical/provider/performance work.
- Preserve UIR-07a explicit highlighted/opened/active identity, immutable settings, bounded 500-run
  History pages, 25-row/five-page warnings, retained accepted pages, stable result navigation and exact
  History/Progress/Summary return focus. Retain UIR-06a combined totals, separate 200-row/five-page
  Files/Folders queries, exact links, validation states and the two distinct Check actions.
- Retain UIR-06b ordered roots, virtual preview scope/revision/signature, confirmation/provenance,
  fresh-preview enforcement, **Reverse rule application**, later manual overrides and Reset semantics.
  Retain all UIR-04/05 monitoring, rescan, query, comparison, decision, reveal and focus contracts.
- Retain UIR-03 scoped acceptance, Narrator, Dark/Desert/live text size, 150%/175% transition/focus/selection
  and prior Debug/Release/Rust evidence. NVDA and physical 200% remain `unrun_unavailable` for UIR-08/A11;
  full A17/native/operator acceptance remains later.
- Runtime: PID 67748/session 1 was re-audited responsive at the exact older UIR-03f fixture executable and
  left untouched. UIR-07b outputs are isolated and not running; test windows closed. No production app,
  worker, database, cache, log, user file or state was touched.
- Boundaries: production deletion disabled; `DisabledRecycleOperationCapabilityExecutor`, `CanSubmit=false`,
  `executorEnabled=false`, absent execution action, recovery/SOP boundaries and all safety locks unchanged.
  Update evidence/plan/checkpoint/kickoff/handoff, review/test and commit each coherent slice; print a complete
  updated copyable continuation prompt at every handoff.
