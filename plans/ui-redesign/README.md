# Windows UI redesign

Status: high-level direction accepted with long-scan/rescan feedback, updated 2026-09-08. Product
implementation has not started. The package records the accepted direction and its limits; it is not a claim of
operator usability acceptance or Windows release acceptance.

## Start here

1. Read [initial findings](discovery/initial-findings.md) for the evidence and limitations.
2. Read [product direction](product-direction.md) for the selected approach and scope.
3. Open [the interactive concept](prototype/index.html) locally in a browser. All data is fictional,
   all interactions are local, and the concept never scans, reads user files, or deletes anything.
4. Use [screen and interaction specifications](screen-specification.md) and
   [visual and accessibility guidance](design-system.md) to specify the WPF work.
5. Consult [capability mapping](capability-map.md) before assuming a new backend feature is needed.
6. Execute [the staged procedure](execution-plan.md), using [validation](validation.md) as the
   acceptance checklist. [Decisions](decisions.md) distinguishes recommendations from approval.
7. Read [long scans and repeat scans](scan-and-rescan-experience.md) for monitoring, diagnostics and
   persistent hash reuse. Use the [Codex session guide](codex-session-guide.md),
   [current checkpoint](session-checkpoint.md) and [kickoff prompt](session-kickoff-prompt.md) across tasks.

The prototype is a design aid, not a browser-based replacement for WPF. The Markdown specifications
are authoritative if its intentionally small fictional dataset or simplified interactions differ.

## Branch and preservation contract

- Preserved development branch: `wpf-poc` at `deefa40` (full hash available from Git).
- Its parent is `52126cd`, the accepted SOP10 checkpoint. `deefa40` preserves the pre-existing
  operator README changes without rewriting their contents.
- Dedicated branch: `codex/ui-redesign`, created from `deefa40` and currently checked out.
- All redesign planning, prototype, product implementation, tests, and checkpoint changes belong
  on this branch. Do not switch branches, merge back, rebase, or delete it before implementation
  is complete unless the user explicitly changes this instruction. Continue on this same branch
  after compaction, restart, and future tasks.
- Other pre-existing branches are left intact. Preservation does not mean combining their divergent
  histories. The complete current checkout and its outstanding tracked edit are preserved together.
- No remote publication is claimed. Local committed work is recoverable through these branch refs.

## Current checkpoint

| Gate | State | Evidence / next action |
|---|---|---|
| UIR-00 Preserve and isolate | complete | `deefa40` on `wpf-poc`; dedicated branch created |
| UIR-01 Evaluation and specification | complete | This package, local prototype checks, and discovery record |
| UIR-02 Direction feedback | complete | User accepted the high-level direction; long-scan/rescan requirements captured |
| UIR-03 Shell and context | ready | Next local implementation gate; see compact session checkpoint |
| UIR-04 through UIR-09 | planned | See execution plan; no product implementation or acceptance claimed |

The next step is UIR-03 shell/context implementation. User feedback accepts the high-level direction;
it does not establish a completed prototype walkthrough or native usability acceptance.
Small visual preferences can evolve during implementation.
Execution enablement and physical/provider campaigns retain their separate authority requirements.

## Completion definition

Redesign completion means the scoped WPF workflows are implemented, meaningful automated checks
pass, required interactive desktop validation has evidence, the user has accepted the workflow,
and all work is committed on `codex/ui-redesign`. It does not mean the parked Recycle Bin release
stream is complete. A polished review-only build must state its execution limitation clearly.
