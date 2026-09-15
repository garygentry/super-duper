# Windows UI redesign

Status: **complete at UIR-09**, updated 2026-09-14. The operator accepted the final scoped workflow
after the UIR-08 integration and available-native pass. Product
implementation includes UIR-04a saved setup/Scan again, UIR-04b long-scan freshness/terminal presentation, UIR-04c compact monitoring/details, UIR-05a compact file queries/totals, UIR-05b adjustable file comparison/full local A03, UIR-05c adjustable folder comparison/remaining local folder criteria, UIR-06a dedicated Review overview/revision-aware validation, UIR-06b focused Location preferences, UIR-07a History/contextual warnings, UIR-07b contextual Performance and UIR-03a/b/c/d/e/f shell/context, shared resources, viewport verification and
the operator-reported scrollbar, theme, text-size and empty-folder fixes. The package records the
accepted direction, UIR-08 available-native pass and [UIR-09 final acceptance](evidence/uir-09-final-acceptance.md).
This is redesign completion, not Windows release or production deletion acceptance.

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
| UIR-03 Shell and context | complete | [Scoped shell acceptance](evidence/uir-03-shell-acceptance.md); available checks passed |
| UIR-04 | complete | Local setup/monitoring slices, integrated A17 and current native Scan again/Progress observations passed in UIR-08 |
| UIR-05 | complete | Local file/folder slices plus current native decisions, paths, focus and both viewports passed after the fixture correction |
| UIR-06 | complete | Local Review/preferences slices plus current native meanings, rules and corrected folder Open set passed |
| UIR-07 | complete | Local History/Performance slices plus current native identity, warning, Performance and return-focus observations passed |
| UIR-08 | complete | [Integration, A01-A17 map and available native evidence](evidence/uir-08-integration-regression.md) passed; NVDA/physical 200% remain unavailable/unrun |
| UIR-09 | complete | [Final scoped workflow accepted and durable completion assessed](evidence/uir-09-final-acceptance.md); no redesign gate remains |

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
NVDA and physical 200% stay unavailable/unrun for A11; no claim is substituted for them. UIR-04a now implements current saved setup, qualified persistent reuse
copy, Save/Discard/Stay and new dated runs with retained history. See its evidence for focused Core,
WPF and isolated real-worker checks. UIR-04b now adds multi-day elapsed/update freshness and terminal
activity (A08/A16). UIR-04c adds the compact summary, honest measured phase bars and expandable exact
details, with 200 Core and three loaded-STA WPF methods passed. UIR-04 retains later integration/operator
validation. UIR-05a now implements compact exposed query controls/totals, exact units and accepted
query snapshots with removable chips. Core 207 and three loaded-STA WPF methods passed; see its
evidence. UIR-05b adds a 36/64 adjustable file comparison, narrow set/copy/selected-copy navigation
and full local A03 verification: 69.7% usable height at 1180x760 and no horizontal scrolling for
essential paths/decisions at 900x600. UIR-05c applies the same adjustable/narrow comparison model to
Folders while completing local A04/A05/A06/A11/A15 coverage: neutral selection, worker-confirmed
named decisions, truthful draft/applied filtering and states, complete paths/descendant scope,
bounded reveal and focus restoration. UIR-06a adds separate bounded Files/Folders Review pages,
worker-owned combined totals, exact-set return links and current/stale/ready/blocked/needs-review
whole-plan status. UIR-06b moves the unchanged worker-owned preference workflow into Review, clearly
separates saved configuration, virtual preview and applied decisions, and uses **Reverse rule
application** while preserving later manual overrides. UIR-07a adds bounded 500-run History pages,
explicit highlighted/open/active identity, immutable recorded settings, contextual current/terminal
warning revisions, retained accepted pages and exact History/Progress/Summary return focus. UIR-07b
adds exact-run contextual Performance, qualified comparison, explicit unavailable/summary-only
telemetry and a selectable 64-device detail while retaining 25/6/64 bounds. UIR-08 now passes the
full local Rust and Windows Debug/Release matrix, maps every A01-A17 criterion, strengthens A17 with
one five-run restart/cache/new/deleted/changed/exclusion/reread regression, and retains 191 current
captures per configuration. Core remains 220; Infrastructure is 76 passed/five skipped; three WPF
methods pass in each configuration. The operator passed the current-screen keyboard, both-viewport,
Narrator, theme/text and 150%/175%-monitor walkthrough after three in-memory fixture gaps were corrected
and retested. The operator subsequently accepted the final scoped workflow at UIR-09. NVDA and physical
200% remain unavailable/unrun, not passed or waived; no corresponding evidence is inferred.
Small visual preferences can evolve during implementation.
Execution enablement and physical/provider campaigns retain their separate authority requirements.

## Completion definition

Redesign completion means the scoped WPF workflows are implemented, meaningful automated checks
pass, required interactive desktop validation has evidence, the user has accepted the workflow,
and all work is committed on `codex/ui-redesign`. It does not mean the parked Recycle Bin release
stream is complete. A polished review-only build must state its execution limitation clearly.

UIR-09 satisfies this definition with the explicit unavailable-evidence limits recorded above. Remain
on `codex/ui-redesign`; merge, push, release validation and production execution require later explicit
operator direction.
