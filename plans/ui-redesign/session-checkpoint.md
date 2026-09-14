# Current redesign session checkpoint

Updated 2026-09-14. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: **UIR-08 local integration regression and acceptance-matrix preparation,
  the commit containing this checkpoint**, based on `a37bbf9` (UIR-07b). UIR-00/01/02/03 complete;
  **UIR-04 through UIR-08 in_progress**. UIR-04a/b/c, UIR-05a/b/c, UIR-06a/b and UIR-07a/b are
  locally implemented; current integrated automation passes.
- [UIR-08 evidence](evidence/uir-08-integration-regression.md) maps every A01-A17 row to current local,
  retained native and remaining evidence. Rust Debug/Release each pass **226 with 10 ignored**. Windows
  Debug/Release builds have zero warnings/errors; each passes **220 Core, 76 Infrastructure and three
  loaded-STA WPF methods**, with five explicit physical/provider/deletion skips.
- Current visual regression passes three WPF methods per configuration and produces **191 Debug + 191
  Release captures** under `artifacts/uir08/captures-{debug,release}`. Representative layout, Light/Dark
  150%-text, long-scan/terminal, Results, Review, History and Performance states were visually reviewed.
  The latest fictional fixture built cleanly under `artifacts/uir08/fixture-approved`; it was not launched.
- The A17 real-worker regression now retains one database/cache across restart and five runs: unchanged
  reuse; combined add/delete/same-size changed content with preserved modified time; exclusion; forced
  re-read; exact new membership; and immutable prior membership/settings/decision. The first assertion-
  path-spelling failure is retained; corrected focused and full Debug/Release matrices pass.
- **Exact next action: prepared UIR-08 current-screen native walkthrough, after explicit authority.** Use
  the evidence file's four-step latest-fixture procedure for keyboard/focus, 1180x760/900x600 access,
  Narrator, Dark/available contrast/150% text and available 150%/175% monitor transitions. Record each
  step pass/fail/unrun/unavailable. Do not bundle a real-worker A17, long-scan, provider or performance run.
- NVDA and physical 200% remain `unrun_unavailable`, not passed or waived. Do not troubleshoot Windows,
  force scaling or install software. The five Windows skips and ten Rust ignored profiles remain skipped;
  no consumed campaign or parked release-validation work ran.
- Preserve UIR-07b exact-run contextual Performance from History/Progress/Summary, deterministic return
  focus, partial/full cache and actual-read summaries, explicit unavailable/qualified comparison, 25/6/64
  bounds and no raw sample/time-series claim. Preserve all UIR-07a highlighted/opened/active identity,
  immutable settings, 500-run History, 25-row/five-page warnings and exact return-focus contracts.
- Preserve UIR-06a worker-owned combined totals, separate 200-row/five-page Files/Folders queries, exact
  links, validation states and **Check marked copies** versus **Check these copies**. Preserve UIR-06b
  ordered roots, virtual preview scope/revision/signature, confirmation/provenance, fresh-preview
  enforcement, **Reverse rule application**, later manual overrides and Reset semantics. Retain all
  UIR-04/05 monitoring, rescan, query, comparison, decision, navigation, reveal and focus contracts.
- Runtime: PID 67748/session 1 was re-audited responsive at the exact older UIR-03f fixture executable and
  left untouched. UIR-08 test workers/windows exited; no other app/worker remained. No production app,
  database, cache, log, user file or state was touched.
- Boundaries: production deletion disabled; `DisabledRecycleOperationCapabilityExecutor`, `CanSubmit=false`,
  `executorEnabled=false`, absent execution action, recovery/SOP boundaries and safety locks unchanged.
  Update native evidence/plan/checkpoint/kickoff/handoff after the authorized walkthrough; review/test and
  commit each coherent slice; print a complete updated copyable continuation prompt at every handoff.
