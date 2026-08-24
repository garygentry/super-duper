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
- Latest completed slice: `Accept WPM12 external invalidation` (this session's commit)
- Worktree after that commit: clean
- MVP Milestones 0-6: implemented and code complete
- Milestone 7 required fail-closed cloud safety: accepted; both unavailable opt-in policies are
  reviewed deferred follow-ons
- Milestone 8 read-only criteria: accepted; representative query and three physical accessibility
  gates remain open
- Milestone 9: all four criteria are accepted; bounded folder relationships, responsive
  single-folder reveal, current-page parent-grouped selection, and no-double-schedule evidence are
  complete
- Milestone 10 review/rule criteria: all accepted; no uncovered acceptance criterion remains
- Milestone 11: non-deleting preflight, durable operation contract, separately gated native executor,
  and acceptance evidence tooling are implemented; production execution remains disabled and the
  milestone is not complete
- Milestone 12: bounded external deletion/modification invalidation is accepted; dirty/overflow and
  coalescing remain independent, while in-app reconciliation depends on Milestone 11 outcomes
- Milestone 13: planned; bounded warning/Activity foundations can advance independently
- Milestone 14: planned; required scope and the operator-accepted/production-enabled completion
  contract are accepted; four reviewed follow-ons are deferred

The latest slice accepts `WPM12-external-invalidation`. Schema v12 stores an idempotent validation
ledger and separate latest-state overlay while immutable scan rows and recorded manual/rule choices
remain unchanged. The worker accepts only 1–200 explicit IDs from one selected set/current member
page, has no cursor, repeats revision and ownership checks at commit, and classifies excluded paths
before metadata/identity validation. Missing/changed files invalidate working Keep/Remove choices;
a restored Present file retains actionable prior intent until a fresh decision or Undecided.

Core/WPF freeze only bound rows, never fetch another page, cancel and reject stale contexts, expose
actionable live state, and provide stable automation, Alt+V, explicit cancellation, announcements,
dispatcher responsiveness, and grid-focus restoration. Focused storage/protocol/Core/loaded-STA
tests, full Debug/Release matrices, and real disposable Debug/Release smoke proved external deletion
and modification, excluded-placeholder safety, immutable history, bounded ownership, restart,
stale/cancelled response rejection, and fresh-decision recovery. The gate is `locally_exhausted`;
watchers, overflow/coalescing, Activity, provider/performance/physical campaigns, mutation, and every
production execution path remain untouched.

## Immediate next step

Advance only `WPM12-watcher-overflow`. Define the bounded dirty-root/overflow protocol and visible
reconciliation-required state without event coalescing, filesystem mutation, or production deletion.

Verifier: injected overflow storage/protocol/Core/STA tests plus real non-mutating Debug/Release
smoke prove a durable/visible dirty state and bounded explicit reconciliation request, with no silent
trust, full-result binding, watcher-event UI fan-out, or immutable-history rewrite.

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
- Controlled access-denied, disconnect, capacity, and path-swap outcomes
- Representative large-plan operation performance
- Independent representative Milestone 8 query performance
- Final freshness, confirmation, admission, and batch constants
- `FOFX_ADDUNDORECORD`
- Residual Shell TOCTOU
- Required Milestone 14 keyboard/accessibility, state, query-instrumentation, Release-scale, and
  end-to-end cloud-safety gates

Missing evidence is `open` or `not_run`, never a pass. Milestone 11 remains incomplete while its
required gates are open.

## Latest verification baseline

The latest external-invalidation slice was verified as follows:

- `Verify-WindowsExternalInvalidation.ps1` passed 3 focused schema/storage tests, 1 worker-protocol
  test, 2 Core page/cancellation tests, and 1 loaded-STA dispatcher/automation/focus test, plus XAML
  and PowerShell parsing, diff hygiene, and every production lock;
- the full Debug Rust workspace passed 82 Core/scan/storage tests, 32 FFI tests, and 14 worker tests;
  the same 3 explicit operator performance profiles remained ignored; optimized Release passed the
  3 focused storage/migration and 1 worker-protocol validation tests;
- full Debug/Release solution matrices each passed 103 Core, 66 Infrastructure, and 3 loaded-STA
  tests; the same 5 explicitly gated provider/physical Shell tests were skipped in each;
- real Debug and Release non-mutating worker/WPF smoke passed one-ID external deletion invalidation
  and restart, external modification/focus/actionable state, immutable history, restored-file sticky
  invalidation and fresh decision, unchanged/restored fixtures, and unchanged production locks;
- targeted Rust/.NET formatting, PowerShell/XAML parsing, `git diff --check`, and the production-lock
  audit passed;
- watcher, overflow/coalescing, provider, physical-accessibility, Recycle Bin/Shell-mutation,
  performance, Activity, and later-milestone campaigns were deliberately skipped.

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
| 2026-08-23 | `98d5558` | Implement and accept WPM11 recovery-review persistence/protocol with schema-v11 append-only observations, derived state, supersession, bounded paging, restart reconstruction, and matching non-UI client contracts while preserving every production lock. | Advance only WPM11-recovery-review-ui; keep automatic inspection/inference, replay, campaigns, Milestone 12 mutation, and production wiring out of scope. |
| 2026-08-23 | `d7b62d7` | Implement and accept the bounded accessible WPM11 recovery-review UI with exact safe retries, explicit append-only correction, approved copy/navigation, focus/automation/announcements, and every production lock preserved. | Advance only WPM11-ambiguous-start; do not substitute another evidence, performance, provider, mutation, or production gate. |
| 2026-08-23 | `16f6996` | Run and accept WPM11 ambiguous-start with disposable durable-start process loss, restart reconstruction, real WPF Option A observations/supersession, exact immutable-evidence verification, and retained passing/failing bundles. | Advance only WPM9-folder-relationships; preserve all execution locks and do not substitute another campaign or later milestone. |
| 2026-08-23 | `29b4256` | Implement and accept bounded side-by-side WPM9 folder relationship cards from immutable paged data with common/differing path context, per-copy/recoverable metrics, stable automation, keyboard/focus behavior, and unchanged physical de-duplication. | Advance only WPM9-explorer-responsiveness; keep parent grouping, thumbnails, review mutation, deletion, later milestones, and production wiring separate. |
| 2026-08-23 | `e578fd3` | Implement and accept responsive single-folder Explorer reveal over one immutable-page member with background native work, bounded actionable state, stale-context rejection, stable automation, Alt+E/double-click access, focus restoration, and real Debug/Release success/failure smoke. | Advance only WPM9-parent-grouping; keep open-all spawning, thumbnails, review mutation, deletion, later milestones, and production wiring separate. |
| 2026-08-23 | `5f79de8` | Implement and accept bounded current-page parent-grouped Explorer selection with deterministic one-call-per-parent background work, aggregate success/actionable partial failure, cancellation/stale-page rejection, Alt+G, stable automation, focus restoration, and real Debug/Release smoke. | Advance only WPM12-external-invalidation; exclude watchers, overflow/coalescing, Activity, deletion, and production wiring. |
| 2026-08-23 | this session | Implement and accept the schema-v12 bounded selection/visible-page external validation overlay with deletion/modification invalidation, sticky prior intent, exclusion-before-access, immutable history, restart, cancellation/stale-context rejection, stable keyboard/automation/focus state, and real Debug/Release smoke. | Advance only WPM12-watcher-overflow; exclude event coalescing, Activity, mutation, provider/physical/performance campaigns, later gates, and production wiring. |
