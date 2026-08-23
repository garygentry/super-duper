# Windows Roadmap Session Handoff

Use this document as the kickoff context for each new coding session. At the end of every completed
slice, update the checkpoint, verification totals, immediate next step, and decision log here before
committing. Every session that changes the worktree must commit all of its completed, in-scope work,
including this handoff update, before handing back to the user. Keep this document concise; the
linked plans remain authoritative.

## Session objective

Continue the Windows post-MVP roadmap from the current worktree. Reassess the boundary, implement
only the next smallest coherent and locally verifiable slice, verify in proportion to risk, update
the authoritative documentation and this handoff, and commit the slice as one focused commit before
ending the session. Do not leave completed session work uncommitted. Repeat across future sessions
until the roadmap is complete, without treating evidence-gated work as locally complete.

## Current checkpoint

- Branch: `wpf-poc`
- Latest completed slice: `Clear resolved Recycle Bin page-error announcement text` (this session's commit)
- Worktree after that commit: clean
- MVP Milestones 0-6: implemented and code complete
- Milestone 7 cloud safety: accepted
- Milestone 8 read-only foundation: implemented; independent operator/performance gates remain open
- Milestone 10 review/rule slices: accepted
- Milestone 11: non-deleting preflight, durable operation contract, separately gated native executor,
  and acceptance evidence tooling are implemented; production execution remains disabled and the
  milestone is not complete
- Milestone 12 live-state overlay: later work; do not pull it into a Milestone 11 slice implicitly

The latest slice clears the stale assertive page-error payload after an exact read-only retry
successfully commits its requested page. The error notification version is deliberately unchanged,
so resolving the error does not publish a misleading second assertive notification. It does not
replay, resolve, or otherwise change the durable operation or execution boundary.

## Immediate next step

Start by auditing the repository and the authoritative plans. Reassess whether another small,
locally implementable Milestone 11 read-only/recovery contract gap exists. Prefer a bounded test or
correctness slice that strengthens existing behavior without enabling execution. If the audit shows
that remaining Milestone 11 work requires physical, provider, controlled-failure, representative-
performance, constants, TOCTOU, Undo, or recovery-resolution evidence/decisions, do not manufacture
a code slice or claim completion. Clearly identify the gate and move only to the next roadmap item
whose prerequisites and product boundary permit local implementation.

No specific unreviewed implementation is pre-authorized by this handoff. The next session must
derive the smallest slice from current code, tests, plans, and history rather than continuing by
momentum.

## Required startup audit

Before editing:

1. Run `git status --short`, inspect recent history, and inspect the complete latest commit diff.
2. Read `AGENTS.md` and these authoritative documents:
   - `ROADMAP.md`
   - `docs/windows-mvp-plan.md`
   - `docs/windows-post-mvp-ux-plan.md`
   - `docs/windows-recycle-bin-acceptance.md`
3. Confirm that the checkpoint above still matches `HEAD` and the worktree.
4. Inspect the code and existing tests for the proposed slice before choosing it.
5. State why the chosen slice is the next smallest coherent local step and which gates it does not
   attempt to close.

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

- Focused `RecycleOperationViewModelTests`, Debug: 13/13 passed
- Focused `RecycleOperationViewModelTests`, Release: 13/13 passed
- Full serialized Debug matrix:
  - Core: 77/77 passed
  - Infrastructure: 56 passed, 5 intentional environment-gated skips
  - WPF smoke: 3/3 passed
- Full serialized Release matrix: same totals and dispositions
- `git diff --check`: passed
- Rust behavior: unchanged; Rust matrix not rerun
- Performance profile: not rerun

Use proportional verification for the next slice. Run focused tests while iterating, then the
relevant full matrix before commit when shared Core/WPF/Infrastructure behavior changes. Run Rust
tests only when Rust behavior changes. Do not run physical or performance campaigns unless the slice
and prerequisites genuinely call for them.

## Completion loop

For each session:

1. Audit current state and preserve unrelated work.
2. Select one smallest coherent slice.
3. Implement only that slice.
4. Add regression coverage for the intended contract and important stale/error/cancellation paths.
5. Update the relevant authoritative plan/acceptance documentation without overstating acceptance.
6. Run proportional verification and `git diff --check`.
7. Review the final diff against every boundary above.
8. Commit one focused commit.
9. Update this handoff's checkpoint, immediate next step, verification baseline, and decision log as
   part of that commit.
10. Confirm `HEAD` contains every completed in-scope change from the session and the worktree is
    clean. A session with completed changes is not finished until its commit succeeds.
11. Report the commit, verification, skipped gates, and best next step.

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
| 2026-08-22 | this session | Clear the resolved assertive Recycle Bin page-error payload after a successful exact retry without publishing another error notification. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
