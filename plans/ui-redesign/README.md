# Windows UI redesign

Status: high-level direction accepted with long-scan/rescan feedback, updated 2026-09-12. Product
implementation includes UIR-04a saved setup/Scan again, UIR-04b long-scan freshness/terminal presentation, UIR-04c compact monitoring/details, UIR-05a compact file queries/totals, UIR-05b adjustable file comparison/full local A03 and UIR-03a/b/c/d/e/f shell/context, shared resources, viewport verification and
the operator-reported scrollbar, theme, text-size and empty-folder fixes. The package records the accepted direction and its limits; it is not a claim of
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
| UIR-03 Shell and context | complete | [Scoped shell acceptance](evidence/uir-03-shell-acceptance.md); available checks passed; NVDA/200% retained for UIR-08/A11 |
| UIR-04 | in_progress | [UIR-04a setup/Scan again](evidence/uir-04a-saved-scan-setup.md) and [UIR-04b elapsed/freshness/terminal](evidence/uir-04b-long-scan-monitoring.md) and [UIR-04c compact monitoring/details](evidence/uir-04c-compact-monitoring.md) locally implemented; later integrated/operator validation remains |
| UIR-05 | in_progress | [UIR-05a file query controls/totals](evidence/uir-05a-file-query-controls.md) and [UIR-05b adjustable file comparison/A03](evidence/uir-05b-file-comparison.md) locally implemented; next UIR-05c folder comparison/remaining decision-path verification |
| UIR-06 through UIR-09 | planned | Later implementation and final native/user acceptance remain |

On 2026-09-11 the operator [passed three initial checks](evidence/uir-03-desktop-walkthrough.md#2026-09-11-initial-operator-checks):
reviewed/active scan context and harmless History highlighting, delayed-folder responsiveness/error
isolation, and file selection/scroll retention plus scoped layout at both fixture sizes.
The [second batch also passed](evidence/uir-03-desktop-walkthrough.md#2026-09-11-navigation-follow-up-passed): explicit Open scan,
active warning Close/return focus and Progress scroll retention. The third batch passed completed-run
warning result focus and the keyboard-only journey, bringing the scoped total to eight, but reported
Progress/Summary scrollbar overlap. [UIR-03e](evidence/uir-03e-scan-scrollbar-clearance.md) fixes the
shared view and passes before/after geometry and Debug/Release integration. The operator confirms
the overlap is corrected and the remaining styling looks good: nine scoped passes plus the fix.
The scoped Narrator journey also passed. NVDA is not installed; its check remains unrun/unavailable.
The next screenshots exposed unreadable Dark, title-only text enlargement and Desert's overlapping
empty-folder message. [UIR-03f](evidence/uir-03f-theme-text-and-empty-state.md) corrects these with
native Fluent styles, Windows text-size events and accessible empty-state layout. On 2026-09-12,
the operator reported "all 3 pass": Dark, Desert's empty layout and live text-size changes at both
fixture sizes. Those reported defects are closed. Cross-monitor transitions at 150% and 175% adjust without
noticeable clipping. Windows did not offer 200%; that case and NVDA remain unavailable/unrun.
The operator also confirmed keyboard focus and file-group selection survive monitor moves.
[UIR-03 is complete for its scoped shell requirements](evidence/uir-03-shell-acceptance.md).
NVDA and 200% stay unavailable/unrun requirements for UIR-08/A11; no complete native matrix or
release acceptance is claimed. UIR-04a now implements current saved setup, qualified persistent reuse
copy, Save/Discard/Stay and new dated runs with retained history. See its evidence for focused Core,
WPF and isolated real-worker checks. UIR-04b now adds multi-day elapsed/update freshness and terminal
activity (A08/A16). UIR-04c adds the compact summary, honest measured phase bars and expandable exact
details, with 200 Core and three loaded-STA WPF methods passed. UIR-04 retains later integration/operator
validation. UIR-05a now implements compact exposed query controls/totals, exact units and accepted
query snapshots with removable chips. Core 207 and three loaded-STA WPF methods passed; see its
evidence. UIR-05b adds a 36/64 adjustable file comparison, narrow set/copy/selected-copy navigation
and full local A03 verification: 69.7% usable height at 1180x760 and no horizontal scrolling for
essential paths/decisions at 900x600. Core 207 and three WPF methods pass. Next: UIR-05c adjustable
folder comparison and remaining folder decision/path verification. Re-audit runtime before
reuse; do not troubleshoot Windows scaling.
Small visual preferences can evolve during implementation.
Execution enablement and physical/provider campaigns retain their separate authority requirements.

## Completion definition

Redesign completion means the scoped WPF workflows are implemented, meaningful automated checks
pass, required interactive desktop validation has evidence, the user has accepted the workflow,
and all work is committed on `codex/ui-redesign`. It does not mean the parked Recycle Bin release
stream is complete. A polished review-only build must state its execution limitation clearly.
