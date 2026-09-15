# Planning, implementation and testing procedure

Status: **UIR-00 through UIR-09 complete**. The operator accepted the final scoped workflow on
2026-09-14 after the UIR-08 local matrix and available current-screen native walkthrough passed.
NVDA and physical 200% remain unavailable/unrun for A11, not passed or waived. Stay on
`codex/ui-redesign`; no redesign gate remains.
This plan owns the redesign scope. The old release-validation ledger remains parked and retains
all safety/evidence gates. Its open execution criteria are not absorbed or marked passed here.

## Finite gate ledger

| Gate | State | Outcome and scope | Entry / completion |
|---|---|---|---|
| UIR-00 | complete | Preserve current work and create dedicated branch | `wpf-poc` at `deefa40`; `codex/ui-redesign` created from it |
| UIR-01 | complete | Findings, direction, specifications, concept, capability mapping, validation and procedure | Package internally checked; prototype limitations recorded; no WPF implementation |
| UIR-02 | complete | High-level direction accepted; operator feedback incorporated | D13-D15; no native or full prototype walkthrough acceptance inferred |
| UIR-03 | complete | Shared visual resources, semantic navigation, selected/active run context, scoped loading | UIR-03a/b/c/d/e/f implemented; [scoped shell acceptance](evidence/uir-03-shell-acceptance.md); available operator checks passed |
| UIR-04 | complete | Setup, Scan again, persistent reuse explanation, live monitoring/details and terminal summaries | Local a/b/c, integrated A17 and available current-screen setup/Progress native observations passed in UIR-08 |
| UIR-05 | complete | File/folder results, compact filters, list/detail comparison, decisions and path actions | Local a/b/c and corrected-fixture native file/folder decisions, paths, focus and both viewports passed in UIR-08 |
| UIR-06 | complete | Dedicated Review, existing rule workflow, whole-plan validation and evidence access | Local a/b plus native totals, validation meanings, Location preferences and corrected folder Open set passed in UIR-08 |
| UIR-07 | complete | History/open-run, contextual warnings and performance detail | Local a/b plus native History identity, warnings, Performance and return focus passed in UIR-08 |
| UIR-08 | complete | Full integration, layout/theme/keyboard/accessibility/scale and long-scan/rescan regression, available physical evidence | [Local matrix and A01-A17 map](evidence/uir-08-integration-regression.md) plus four-step native walkthrough passed after fixture correction; NVDA/physical 200% remain unavailable/unrun |
| UIR-09 | complete | User workflow acceptance, final package and durable handoff | [Operator accepted the final scoped workflow](evidence/uir-09-final-acceptance.md); no critical redesign defect or further redesign gate remains |

UIR-04 and UIR-05 are logically independent after UIR-03 but may be executed sequentially in one
workspace. This is not authorization to spawn agents or create additional branches. Implementation
gates are named `local_code` scopes; each entry lists the findings/acceptance criteria justifying
the work, so future agents need not mine historical accepted slices for artificial reopen reasons.

## Working method

UIR-03a (2026-09-08) establishes named destinations behind the retained seven tabs, independent
active progress/cancellation while browsing history, dated selected/monitoring context, shared
shell/recovery styles, history generation guards, setup readiness and demand-loaded Performance.
158 Core tests and four loaded-STA WPF tests pass in isolated Debug outputs. This is partial
implementation, not native acceptance or UIR-03 completion.

UIR-03b (2026-09-08) composes four primary areas, separates highlighted History from the opened
workspace run, and adds explicit Open scan. Files/Folders/Review/Performance load on demand and
retain one run's bounded state. Current warnings preserve the opened run across saved scans;
stopped-run summaries remain separate from active progress/cancellation. Stale file-rule and
Review continuations are guarded. 170 Core tests and four loaded-STA WPF tests pass in isolated
Debug outputs. See [evidence](evidence/uir-03b-workspace-navigation.md).

UIR-03c (2026-09-08) adopts shared screen resources and bounded long-name headers, adds a
populated MainWindow fixture and a standalone fictional desktop host, and verifies actual
programmatic keyboard focus, warning return/result navigation, native file/progress scroll retention
and a delayed failing pane. Focus delivery rejects stale navigation generations. Interim Files
disclosures reduce crowding; they do not accept the final S03 search/totals/list-detail design.
Debug/Release Rust builds/tests and Windows builds/tests pass: 226 Rust tests (10 ignored),
170 Core, 75 Infrastructure (five operator-only skips), and four WPF methods per configuration.
See [evidence and remaining layout failures](evidence/uir-03c-populated-shell.md).

UIR-03d (2026-09-08) resolves the local populated viewport blockers with scrolling Files/History
pages, bounded virtualized grids, stacked/wrapping context and action bars, and minimum widths for
complete comparison/warning actions. Shared readable sizes and the 900x600 minimum are preserved.
The corrected long-path fixture verifies clipped bounds, actual focus handlers, nonzero same-run
page/grid scroll and selection retention, including an 80-DIP desktop-toolbar allowance at minimum
size. Debug/Release paired worker builds and Windows integration pass (170 Core, 75 Infrastructure
with five operator-only skips, four WPF methods each). See [evidence](evidence/uir-03d-viewport-access.md).
The interim stacked Files layout replaces its vertical split adjustment; the full adjustable S03
layout, compact search/totals and A03 no-horizontal-scroll/60% requirements remain in UIR-05.

UIR-03e (2026-09-11) fixes the operator-reported Progress/Summary outer-scrollbar overlap by
moving the existing inset inside the shared ScrollViewer and preserving the empty-state inset.
The new geometry regression fails before the fix with a 12-DIP overlap, then passes for both tabs,
top/bottom, both sizes and the toolbar allowance. Paired worker and Windows Debug/Release integration
pass (170 Core, 75 Infrastructure/five skips, four WPF methods each). See [evidence](evidence/uir-03e-scan-scrollbar-clearance.md).

UIR-03f (2026-09-11) corrects operator-reported Dark styling, title-only text enlargement and
Desert's folder-empty/header overlap. Shared native Fluent bases and semantic dynamic brushes,
Windows text-scale events, bounded scrolling Folders and wrapping enlarged actions address the
reproduced A09/A11/A05 defects. See [evidence](evidence/uir-03f-theme-text-and-empty-state.md).
Final adjustable Files/Folders comparison remains UIR-05; the interim folder splitter is replaced.

**UIR-03 scoped shell acceptance remains complete.**
The operator confirmed file-group selection survived monitor moves, completing the available shell
walkthrough. See [the gate assessment](evidence/uir-03-shell-acceptance.md) for the evidence mapping
and precise scope. Dark/Desert/text-size defects are closed, and keyboard/Narrator/150%/175%
transition, focus and selection checks passed. Retain earlier scoped passes without repetition.
NVDA (not installed) and 200% (not offered by Windows) remain unavailable/unrun, not passed or
waived. Full A11 native accessibility/DPI is owned by UIR-08 in the acceptance matrix; record the
missing cases there until actual evidence or explicit later disposition. Do not force custom
scaling, troubleshoot Windows or install software under this assessment. This closes the scoped
shell implementation gate, not final native/user or release acceptance.

**UIR-04a implemented (2026-09-12).**
[UIR-04a evidence](evidence/uir-04a-saved-scan-setup.md) covers current saved setup through Scan again,
qualified persistent reuse versus candidate re-reading, valid save-before-start, new dated runs and
retained historical results/decisions. Save / Discard / Stay protects draft navigation, and pending
Start locks edits. Focused Core and loaded-STA WPF checks plus a small real-worker cache-reopen fixture
pass in isolated outputs. UIR-04 remains in progress; full A17 and native/user acceptance remain later.

**UIR-04b implemented (2026-09-12).** [Evidence](evidence/uir-04b-long-scan-monitoring.md) covers
readable multi-day elapsed time, monotonic accepted-update receipt freshness before dispatcher
coalescing, historical terminal activity/metrics, frozen terminal elapsed and prompt lifecycle
announcements. Full Core 193 passed; three loaded-STA WPF methods passed with controlled clock,
delayed progress, minimized/restored state and four terminal outcomes. Fixture built in isolated
outputs; no production runtime mutation or new physical acceptance.

**UIR-04c implemented (2026-09-12).** [Evidence](evidence/uir-04c-compact-monitoring.md) covers the
compact S02 summary, sampled filename/parent, measured candidate/substage work and expandable exact
Work / Hash reuse / Diagnostics. The worker's known-total signal distinguishes unknown from zero;
100% phase work never completes a run. Exact diagnostics, disclosure/focus/scroll and UIR-04b
freshness, terminal and coalesced announcements remain. Core 200 and three loaded-STA WPF methods passed;
settled captures and review are recorded in the evidence. No engine, worker, cache or production wiring changed.
UIR-04 remains in progress for later integrated A17/native/operator validation; local a/b/c are implemented.

**UIR-05a implemented (2026-09-12).** [Evidence](evidence/uir-05a-file-query-controls.md) covers
compact exposed path/query controls and three filtered totals, exact binary size units, removable
applied chips and complete retained filter semantics. Accepted-query snapshots isolate drafts from
paging, facet sorting and rule scope; delayed/failing replacements retain previous results. Clear
restores default sorting, late generations are rejected, and native focus/scroll/decisions remain.
Core 207 passed; three loaded-STA WPF methods passed with delayed long-path fixtures and retained
monitoring/theme/text-size checks. UIR-05a implements local A03/A06/A13 header/query work, not full A03.
No engine, protocol, query ceiling or production wiring changed. UIR-05 is in progress.

**UIR-05b implemented (2026-09-12).** [Evidence](evidence/uir-05b-file-comparison.md) covers the
adjustable 36/64 Files list/detail surface and full local automated A03 verification. The populated
1180x760 fixture measures 279.0/400.3 DIPs, or 69.7% usable comparison height. At 900x600 explicit
set, copy-list and selected-copy pages retain complete paths, Keep/Mark/Reset and path actions through
vertical-only scrolling, with Back to copies/sets and actual focus restoration. Single wrapping
virtualized columns eliminate grid horizontal scrolling; sorting remains server-owned. Core 207 and
three loaded-STA WPF methods pass. No engine, protocol, paging/cache ceiling, decision truth or
production wiring changed. UIR-05 remains in progress.

**UIR-05c implemented (2026-09-13).** [Evidence](evidence/uir-05c-folder-comparison.md) covers the
adjustable 36/64 Folders set/copy comparison and remaining local A04/A05/A06/A11/A15 behavior.
Narrow width uses explicit set, copy-list and selected-copy pages; complete paths, relationship and
descendant scope, five actions, bounded Explorer reveal and Back/focus behavior remain vertically
reachable without horizontal scrolling. Folder query drafts cannot retarget paging, failed
replacements retain accepted results, and default/filtered empty plus loading/error/unavailable
states remain distinct. Core 210 and three loaded-STA WPF methods pass; the fixture build is clean.
No engine, protocol, paging/cache ceiling, decision truth or production wiring changed. UIR-05a/b/c
are locally implemented; UIR-05 remains in progress for later integrated/native acceptance.

**UIR-06a implemented (2026-09-13).** [Evidence](evidence/uir-06a-review-overview.md) covers the
dedicated selected-scan Review overview, worker-owned combined totals, separate 200-group Files and
Folders pages with independent five-page caches, exact bounded return links and retained-page error
behavior. Whole-plan **Check marked copies** is distinct from visible-page **Check these copies**;
freshness and outcome separately expose current, plan-changed, ready, blocked and needs-review
states from worker preflight revisions/reasons. Core 214 and three loaded-STA WPF methods pass; 105
captures and the clean fixture build are retained. No worker/protocol/ceiling, rule, execution,
recovery-resolution or production boundary changed. UIR-06 remains in progress.

**UIR-06b implemented (2026-09-13).** [Evidence](evidence/uir-06b-location-preferences.md) covers the
focused **Location preferences** panel in Review. Saved ordered-root configuration, virtual preview
scope/rule/review revision/signature, exact application confirmation, application provenance,
**Reverse rule application** and later manual overrides retain the existing worker contracts. Review
synchronizes current worker revision without opening the Files workspace; stale preview and same-run edit
retention are covered. Core 216 and three loaded-STA WPF methods pass with 111 captures and a clean
fixture build. UIR-06a/b are locally implemented; UIR-06 remains in progress for later integrated/
native acceptance. No rule protocol, manual Reset, arbitrary undo, execution or recovery behavior changed.

**UIR-07a implemented (2026-09-13).** [Evidence](evidence/uir-07a-history-warnings.md) covers one
bounded 500-run History page with explicit Previous/Next navigation, immutable recorded settings,
highlighted-versus-opened-versus-active identity, contextual current/terminal warning revisions,
retained accepted pages and exact History/Progress/Summary return focus. Warning pages remain 25 rows
with five cached pages and stable completed-run result navigation. Core 218 and three loaded-STA WPF
methods pass with 114 reviewed captures and a clean fixture build. No worker/protocol/database,
Performance, recovery, execution or production boundary changed.

**UIR-07b implemented (2026-09-14).** [Evidence](evidence/uir-07b-contextual-performance.md) covers
one-action Performance detail from exact History/Progress/Summary runs with deterministic return focus.
The detail verifies product-run identity, composes six phase rows, partial/full cache and actual-read
summaries, selects one of 64 device rows for current/peak detail and retains 25 comparison rows with
explicit context qualifiers. Unavailable fields remain unavailable and the UI makes no raw-sample or
time-series claim. Core 220 and three loaded-STA WPF methods pass with 125 captures and a clean fixture
build. No worker/protocol/database, recovery, execution or production boundary changed.

**UIR-08 complete (2026-09-14).** [Evidence](evidence/uir-08-integration-regression.md)
maps A01-A17 to current and retained evidence. Rust Debug/Release each pass 226 tests with ten named
profiles ignored. Windows Debug/Release each build cleanly and pass 220 Core, 76 Infrastructure and
three loaded-STA WPF methods, with five physical/provider/deletion cases skipped. Current visual
regression produced 191 captures per configuration and representative states were reviewed. The A17
real-worker test now combines restart/persistent cache, unchanged reuse, added/deleted membership,
same-size changed content with preserved modified time, exclusion, forced re-read and immutable old
history/decisions across five runs. The operator passed the available 1180x760/900x600 keyboard,
Narrator, Dark/contrast/150%-text and 150%/175%-monitor batch after three acceptance-fixture gaps were
corrected and focused-retested. NVDA and physical 200% remain unavailable/unrun, not passed or waived.

**UIR-09 complete (2026-09-14).** [Final acceptance evidence](evidence/uir-09-final-acceptance.md)
records the operator's explicit acceptance after review found no open critical redesign defect. The
durable package preserves NVDA and physical 200% as unavailable/unrun limitations and keeps release,
recovery, physical/provider/performance and production-execution authority separate.

No redesign action remains. Stay on this branch and await explicit operator direction before merge,
publication, parked release-validation resumption, a new campaign or production execution.

Use the [multi-session guide](codex-session-guide.md), [compact checkpoint](session-checkpoint.md)
and [kickoff prompt](session-kickoff-prompt.md). UIR-04/07/08 also read
[the long-scan/rescan contract](scan-and-rescan-experience.md). One gate may span several commits;
record partial progress without calling the gate complete. UIR-05 can proceed independently after UIR-03.

1. At cold start audit Git, read this checkpoint and the shared handoff, and confirm the branch is
   `codex/ui-redesign`. If it differs, establish why before editing; do not silently switch or discard
   work. Preserve unrelated changes. Do not re-read historical acceptance logs without a cited need.
2. Select the next dependency-ready gate and the specific A-criteria it changes. Inspect only those
   linked surfaces/contracts/tests. Prepare runtime isolation before builds or app launches.
3. Implement a small coherent user journey, preserving worker and safety contracts. Prefer composing
   existing view models; refactor only where the named slice needs it. A broad new protocol feature
   goes back to scope review rather than entering through the visual redesign.
4. Run focused behavior tests and inspect representative WPF states. Update the evidence record.
   At integration boundaries run the required build/test matrix and scoped desktop checks.
5. Update this ledger, package README, compact session checkpoint, decisions when changed, and the shared handoff; commit the
   coherent in-scope slice. Continue if further work is authorized and dependency-ready.
6. Stop for actual missing authority/evidence or a design decision affecting user intent. Prepare
   a concrete reviewable result first; do not ask repeatedly about reversible implementation choices.
7. At completion remain on the redesign branch. Merge/push/release only on later user instruction.
8. At session handoff with remaining work, print the next session's copyable continuation prompt
   in the final response, tailored to the committed checkpoint as required by the session guide.

## Branch durability and checkpoints

`wpf-poc` is the preserved baseline, not the target for continuing edits. Do not use a stash as the
only record of completed work. Every completed gate has a bounded commit on `codex/ui-redesign`.
No branch switch is part of testing or release preparation. If isolated build output or fixture
directories are required, create them without changing the checkout's branch.

## Stopping criteria

Do not declare the redesign complete with only a prototype or passing unit tests. UIR-09 required
the actual scoped WPF behavior, coherent states, meaningful regression checks, physical accessibility
evidence required by the validation plan, and user acceptance. Distinguish:

- Planning complete: direction and operator feedback captured; local implementation ready.
- Implemented: code exists and relevant automated checks pass.
- Native acceptance pending: physical desktop/user checks remain.
- Redesign complete: scoped product is accepted and committed; production deletion remains disabled.

The redesign is complete at UIR-09 under this definition. This does not complete the parked
release-validation stream or the unavailable NVDA/physical-200% evidence.

The parked release-validation stream can still be incomplete after redesign completion. Do not
weaken its thresholds, infer provider or Recycle Bin authority, alter consumed evidence identities,
or make broad 'release ready' claims from this separate UI work.
