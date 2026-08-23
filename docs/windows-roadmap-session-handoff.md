# Windows Roadmap Session Handoff

Use this document as the kickoff context for each new coding session. At the end of every completed
slice, update the checkpoint, verification totals, immediate next step, and decision log here before
committing. Every session that changes the worktree must commit all of its completed, in-scope work,
including this handoff update, before handing back to the user. Keep this document concise; the
linked plans remain authoritative.

## Session objective

Continue the Windows post-MVP roadmap from the current worktree by advancing one named closure-ledger
gate or one explicitly named coherent gate group. Verify in proportion to risk, update the closure
ledger, authoritative documentation, and this handoff, and commit the bounded slice as one focused
commit before ending the session. Do not leave completed session work uncommitted. Do not replace a
blocked or exhausted gate with progressively narrower speculative work merely to keep a session
active.

## Current checkpoint

- Branch: `wpf-poc`
- Latest completed slice: `Accept WPM11 recovery-review persistence` (this session's commit)
- Worktree after that commit: clean
- MVP Milestones 0-6: implemented and code complete
- Milestone 7 required fail-closed cloud safety: accepted; both unavailable opt-in policies are
  reviewed deferred follow-ons
- Milestone 8 read-only criteria: accepted; representative query and three physical accessibility
  gates remain open
- Milestone 9: planned/not started except accepted no-double-schedule evidence; three criteria open
- Milestone 10 review/rule criteria: all accepted; no uncovered acceptance criterion remains
- Milestone 11: non-deleting preflight, durable operation contract, separately gated native executor,
  and acceptance evidence tooling are implemented; production execution remains disabled and the
  milestone is not complete
- Milestone 12: planned; external invalidation, dirty/overflow, and coalescing lanes can advance
  independently, while in-app reconciliation depends on Milestone 11 production outcomes
- Milestone 13: planned; bounded warning/Activity foundations can advance independently
- Milestone 14: planned; required scope and the operator-accepted/production-enabled completion
  contract are accepted; four reviewed follow-ons are deferred

The latest slice accepts `WPM11-recovery-review-persistence`. Schema v11 adds one append-only
operator-observation table, exact idempotent mutation, bounded current/history paging, supersession,
restart reconstruction, three derived review states, and matching non-UI worker/Core/
Infrastructure contracts. All five approved observations are supported. Complete review remains
`review_complete_with_unresolved_evidence`; every original item `unknown`, batch `ambiguous`,
operation `recovery_required`, and recovery row remains exactly unchanged. The implementation uses
only SQLite and cannot inspect live source/provider/content/Recycle Bin state or invoke any
operation transition. WPF has no review controls yet. `WPM11-production-wiring` remains separately
blocked, and all production execution locks remain unchanged.

## Immediate next step

Advance only `WPM11-recovery-review-ui`. Its persistence/protocol dependency is accepted, and the
specific uncovered criterion is the absence of the accessible operator workflow over schema v11.
Implement bounded Core/WPF review status, per-item observation/supersession, durable evidence copy,
Recycle Bin navigation, and fresh-scan navigation with keyboard/focus/announcement coverage.

Verifier: affected Core/Infrastructure/WPF Debug/Release tests and real non-mutating smoke cover all
states/transitions, stale responses, paging, append-only correction, keyboard/focus/announcements,
and forbidden-action absence. Explicit non-goals: no automatic live filesystem/provider/Recycle
Bin inspection or inference; no recovery/physical/performance campaign; no outcome mutation,
operation replay, execution, or Milestone 12 mutation; no Recycle Bin production wiring; and no
change to `CanSubmit:false`, disabled production injection, or `executorEnabled:false`. Other ready
lanes are not substitute authority for that session.

## Required startup audit

Before editing:

1. Run `git status --short`, inspect recent history, and inspect the complete latest commit diff.
2. Read `AGENTS.md` and these authoritative documents:
   - `ROADMAP.md`
   - `docs/windows-mvp-plan.md`
   - `docs/windows-post-mvp-ux-plan.md`
   - `docs/windows-recycle-bin-acceptance.md`
   - `docs/windows-roadmap-closure-ledger.md`
3. Confirm that the checkpoint above still matches `HEAD` and the worktree.
4. Identify the exact authorized gate ID from the closure ledger and confirm its dependencies are
   satisfied. During the bootstrap only, inventory gates instead of selecting implementation.
5. Inspect code and tests only for the authorized gate, then state the evidence that permits edits,
   the verifier that closes it, and which gates remain outside the slice.

If newer commits exist, treat Git and the authoritative plans as truth, then update this document
before committing the next slice.

## Non-negotiable boundaries

- Production Recycle Bin execution remains disabled.
- Keep `RecycleOperationViewModel.CanSubmit` false.
- Keep production injection on `DisabledRecycleOperationCapabilityExecutor`.
- Keep every worker response `executorEnabled:false`.
- Do not add a **Move to Recycle Bin now** action.
- Do not inspect the live filesystem to resolve ambiguous outcomes.
- Do not replay or resolve recovery-required operations without an explicitly reviewed product
  workflow.
- Do not weaken acceptance or performance thresholds.
- Do not rerun performance profiles merely until green; retain passing and failing evidence.
- Do not claim physical hardware, provider, Narrator/NVDA, high-contrast, multi-monitor DPI,
  recovery-resolution, or representative-performance acceptance without qualifying evidence.
- Keep physical/operator/provider/performance gates distinct from locally implementable work.
- Do not edit product code without a named `local_code` gate and a reproducible defect, failing
  test, or specific uncovered acceptance criterion.
- Do not add a test-only slice unless it closes a named criterion or protects a concrete defect
  discovered while advancing that criterion.
- Audit a surface once per gate. If no local gap is found, record `locally_exhausted` and do not
  repeat the audit until new evidence, code, or a reviewed decision changes the boundary.
- After two consecutive attempts with the same blocker and no new evidence, stop the goal and
  preserve the diagnostic result instead of trying adjacent speculative changes.
- Missing hardware, provider fixtures, operator access, or product decisions are stopping
  conditions, not authorization to substitute unrelated work.
- Do not search unrelated TODOs, refactor opportunistically, or pull a later milestone forward to
  keep a goal active.
- Keep `super-duper-core` UI-agnostic.
- Keep WPF views in the executable, application contracts/view models in Core, and process/native
  concerns in Infrastructure.
- Do not alter unrelated formatting or unrelated user changes.

## Open evidence and decision gates

These remain open unless a later committed slice cites qualifying evidence or an explicit reviewed
decision:

- Physical Narrator and NVDA acceptance
- Windows high-contrast acceptance
- Physical 100/150/200% multi-monitor DPI acceptance
- Real-provider no-hydration and provider-outcome campaigns
- Controlled access-denied, disconnect, capacity, path-swap, and process-loss outcomes
- Representative large-plan operation performance
- Independent representative Milestone 8 query performance
- Final freshness, confirmation, admission, and batch constants
- `FOFX_ADDUNDORECORD`
- Residual Shell TOCTOU
- WPM11 accessible recovery-review WPF implementation
- Physical ambiguous-start/process-loss inspection after the UI gate accepts
- Required Milestone 14 keyboard/accessibility, state, query-instrumentation, Release-scale, and
  end-to-end cloud-safety gates

Missing evidence is `open` or `not_run`, never a pass. Milestone 11 remains incomplete while its
required gates are open.

## Latest verification baseline

The latest code slice was verified as follows:

- focused schema-v11 storage tests and recovery-review worker protocol tests: passed;
- focused Infrastructure JSON/non-UI client contract test: passed;
- Debug `cargo build --workspace` and `cargo test --workspace`: passed (124 tests passed; three
  explicit performance profiles ignored);
- serialized Debug .NET solution tests: passed (141 passed; five explicit provider/Shell tests
  skipped);
- authoritative `Verify-WindowsRelease.ps1`: passed Release Rust tests/build, Release .NET
  build/serialized tests, publish validation, worker protocol smoke, WPF automation, and shutdown
  smoke;
- targeted Rust formatting and `git diff --check`: passed;
- full-row snapshots prove original operation/batch/item/recovery evidence unchanged; nonexistent-
  path protocol fixtures and method-surface audit prove no live inspection or operation transition;
- physical accessibility, provider, Shell-mutation, representative-performance, and controlled
  recovery campaigns: deliberately not run.

Use proportional verification for the next slice. Run focused tests while iterating, then the
relevant full matrix before commit when shared Core/WPF/Infrastructure behavior changes. Run Rust
tests only when Rust behavior changes. Do not run physical or performance campaigns unless the slice
and prerequisites genuinely call for them.

## Completion loop

For each session:

1. Audit current state and preserve unrelated work.
2. Select one ready named gate or coherent gate group from the closure ledger.
3. Confirm its dependencies, evidence for edits, completion check, and explicit non-goals.
4. Implement only that gate. If a prerequisite is missing or the gate is locally exhausted, update
   the ledger and stop without manufacturing a code slice.
5. Add regression coverage required by the named criterion; do not enumerate unrelated edge cases.
6. Update the ledger and relevant authoritative plan/acceptance documentation without overstating
   acceptance.
7. Run proportional verification and `git diff --check`.
8. Review the final diff against every boundary above.
9. Commit one focused commit.
10. Update this handoff's checkpoint, immediate next gate, verification baseline, and decision log
    as part of that commit.
11. Confirm `HEAD` contains every completed in-scope change from the session and the worktree is
    clean. A session with completed changes is not finished until its commit succeeds.
12. Report the commit, gate disposition, verification, skipped gates, blocker or next ready gate,
    and any user decision required.

## Handoff decision log

| Date | Commit | Completed slice | Next boundary |
|---|---|---|---|
| 2026-08-22 | `ec8da88` | Announce only committed Recycle Bin result pages; cover backward repeatability and stale/cancelled silence. | Re-audit for the next locally implementable Milestone 11 read-only/recovery gap; do not cross an evidence or recovery-design gate. |
| 2026-08-22 | `96a49af` | Preserve the committed Recycle Bin recovery page and cursor history across a failed forward fetch; allow exact retry. | Re-audit for another local read-only contract gap; stop at physical/provider/performance or recovery-resolution gates. |
| 2026-08-22 | `a476af3` | Cover failed backward recovery paging after the bounded cache evicts an older page; preserve the committed page and exact retry. | Re-audit for another local read-only contract gap; stop at physical/provider/performance or recovery-resolution gates. |
| 2026-08-22 | `a022588` | Clear stale Recycle Bin UI Automation announcement text when the selected run context changes without publishing an empty notification. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `35ee281` | Separate assertive Recycle Bin operation/page failures from polite committed-result notifications. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `45f6a09` | Exercise the separate Recycle Bin success/error channels through loaded WPF automation peers. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `d971119` | Announce the exact first committed unknown-result range during Recycle Bin reconstruction. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `306972e` | Retry the exact failed read-only Recycle Bin detail request, including the initial recovery page, without retrying the operation. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `d0b0362` | Commit the matching forward or backward cursor-history transition after a successful exact Recycle Bin detail retry. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `bd7efa6` | Retry a failed read-only Recycle Bin operation-summary reconstruction request for the exact selected completed run without allowing a stale retry to replace newer context. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `bd9e983` | Clear the resolved assertive Recycle Bin page-error payload after a successful exact retry without publishing another error notification. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `14649ea` | Prove a late failed exact page retry cannot replace newer run context, publish stale feedback, expose another retry, or mutate cursor history. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `37894b9` | Prove a late failed exact operation-summary retry cannot replace newer operation/page context, publish stale feedback, expose another retry, or change navigation. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `ce54d1a` | Announce a successful read-only operation reconstruction that finds no stored operation intent, without exposing retry or changing the execution boundary. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `c34eebb` | Cover an exact read-only operation-summary retry that resolves an earlier load failure to no stored operation intent. | Replace further open-ended gap discovery with a finite roadmap closure audit. |
| 2026-08-23 | `8ff0b60` | Add the closure-ledger scaffold and tighten roadmap execution, edit authority, anti-spin, and stop rules. | Populate and accept every remaining Milestone 7-14 gate without product-code changes, then authorize only the first dependency-ready gate. |
| 2026-08-23 | `775417a` | Populate and accept the Milestone 7-14 closure inventory, reconcile statuses, and expose the critical path and independent lanes. | Review WPM7-opt-in-policy-scope + WPM14-required-scope + WPM14-completion-contract; do not substitute another ready gate. |
| 2026-08-23 | `d10ce7d` | Accept the required/deferred Milestone 7/14 scope and operator-accepted plus production-enabled completion contract without authorizing Recycle Bin production wiring. | Advance only WPM11-recovery-workflow to an explicit product/safety decision boundary; do not substitute another ready gate. |
| 2026-08-23 | `a2f3b6c` | Prepare the complete WPM11 ambiguous-recovery decision package without selecting or implementing a model. | Keep WPM11-recovery-workflow as the only authorized gate until the user chooses Option A, Option B with the explicit per-item waiver, or a named revision. |
| 2026-08-23 | `6d0182a` | Accept WPM11 recovery Option A and create its exact persistence/protocol, accessible WPF, and controlled process-loss dependency chain without product-code changes. | Advance only WPM11-recovery-review-persistence; keep WPF, campaigns, live inference, replay, and production wiring out of scope. |
| 2026-08-23 | this session | Implement and accept WPM11 recovery-review persistence/protocol with schema-v11 append-only observations, derived state, supersession, bounded paging, restart reconstruction, and matching non-UI client contracts while preserving every production lock. | Advance only WPM11-recovery-review-ui; keep automatic inspection/inference, replay, campaigns, Milestone 12 mutation, and production wiring out of scope. |
