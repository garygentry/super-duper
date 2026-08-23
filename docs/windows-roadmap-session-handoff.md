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
- Latest completed slice: `Tighten Windows roadmap execution controls` (this session's commit)
- Worktree after that commit: clean
- MVP Milestones 0-6: implemented and code complete
- Milestone 7 cloud safety: accepted
- Milestone 8 read-only foundation: implemented; independent operator/performance gates remain open
- Milestone 9 status and remaining scope: must be reconciled by the closure-ledger bootstrap
- Milestone 10 review/rule slices: accepted
- Milestone 11: non-deleting preflight, durable operation contract, separately gated native executor,
  and acceptance evidence tooling are implemented; production execution remains disabled and the
  milestone is not complete
- Milestone 12 live-state overlay: later work; do not pull it into a Milestone 11 slice implicitly

The latest slice replaces open-ended local-gap discovery with a durable closure-ledger scaffold,
finite gate dispositions, dependency-based work selection, explicit locally-exhausted handling, and
anti-spin stop conditions. Product code and the production execution boundary are unchanged.

## Immediate next step

Populate and accept `docs/windows-roadmap-closure-ledger.md` before selecting another product-code
slice. Inventory every Milestone 7-14 acceptance criterion and still-open decision, cite existing
evidence, assign a stable gate ID/disposition/state/dependency/owner/next action/completion check,
reconcile Milestone 9 and optional/deferred scope, and produce a dependency-ordered queue. This is a
documentation and repository-audit slice only.

Do not change product code, add tests, run physical/provider/performance campaigns, or search for
another Milestone 11 retry/recovery edge case during the bootstrap. Stop and report any scope choice
that requires user review rather than silently selecting it. The first accepted ready gate in the
ledger becomes the only implementation authority for the following session.

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
- Ambiguous-recovery resolution workflow and physical recovery inspection

Missing evidence is `open` or `not_run`, never a pass. Milestone 11 remains incomplete while its
required gates are open.

## Latest verification baseline

The latest slice was verified as follows:

- Focused `RecycleOperationViewModelTests`, Debug: 17/17 passed
- Focused `RecycleOperationViewModelTests`, Release: 17/17 passed
- Full Core suite, Debug: 81/81 passed
- Full Core suite, Release: 81/81 passed
- `git diff --check`: passed
- Production behavior and Rust behavior: unchanged; Rust matrix not rerun
- Infrastructure/WPF behavior: unchanged; Infrastructure and WPF matrices not rerun
- Performance profile: not rerun

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
| 2026-08-23 | this session | Add the closure-ledger scaffold and tighten roadmap execution, edit authority, anti-spin, and stop rules. | Populate and accept every remaining Milestone 7-14 gate without product-code changes, then authorize only the first dependency-ready gate. |
