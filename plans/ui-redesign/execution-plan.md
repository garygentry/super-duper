# Planning, implementation and testing procedure

Status: UIR-03 in progress; UIR-03a/b/c implemented; populated layout and desktop acceptance remain open. Stay on `codex/ui-redesign`.
This plan owns the redesign scope. The old release-validation ledger remains parked and retains
all safety/evidence gates. Its open execution criteria are not absorbed or marked passed here.

## Finite gate ledger

| Gate | State | Outcome and scope | Entry / completion |
|---|---|---|---|
| UIR-00 | complete | Preserve current work and create dedicated branch | `wpf-poc` at `deefa40`; `codex/ui-redesign` created from it |
| UIR-01 | complete | Findings, direction, specifications, concept, capability mapping, validation and procedure | Package internally checked; prototype limitations recorded; no WPF implementation |
| UIR-02 | complete | High-level direction accepted; operator feedback incorporated | D13-D15; no native or full prototype walkthrough acceptance inferred |
| UIR-03 | in_progress | Shared visual resources, semantic navigation, selected/active run context, scoped loading | UIR-03a/b/c implemented; [latest evidence](evidence/uir-03c-populated-shell.md). Integration matrix passes; populated layout and A01/A02/A09 desktop acceptance remain open; next UIR-03d below |
| UIR-04 | planned | Setup, Scan again, persistent reuse explanation, live monitoring/details and terminal summaries | UIR-03; A05/A06/A08/A14/A15/A16/A17 with controlled clock, lifecycle and rescan fixtures |
| UIR-05 | planned | File/folder results, compact filters, list/detail comparison, decisions and path actions | UIR-03; A03/A04/A05/A06/A11/A13/A15 and existing query/focus/page contracts |
| UIR-06 | planned | Dedicated Review, existing rule workflow, whole-plan validation and evidence access | UIR-05; A04/A10/A15 including revision, survivor, overlap, reversal and restart |
| UIR-07 | planned | History/open-run, contextual warnings and performance detail | UIR-03/04; A07/A12/A13/A15/A16/A17 and current/terminal warning boundaries |
| UIR-08 | planned | Full integration, layout/theme/keyboard/accessibility/scale and long-scan/rescan regression, physical evidence | UIR-04 through UIR-07; all A01-A17 verified with honest physical/skipped states |
| UIR-09 | planned | User workflow acceptance, final package and durable handoff | UIR-08; scope accepted, no open critical defect, all work committed on same branch |

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

**Next slice: UIR-03d, local_code** — resolve the populated shell viewport blockers before the
A01/A02/A09 desktop walkthrough: Files can have no usable grids at 900x600, comparison is crowded
at 1180x760, and narrow History is cramped. Preserve readable resources and the 900x600 minimum;
verify reachable rows/actions and same-run focus/scroll after the layout correction. Keep the full
S03 result workflow in UIR-05. Use the prepared [fictional walkthrough](evidence/uir-03-desktop-walkthrough.md).
Physical keyboard/Narrator/NVDA/theme/high-contrast/text enlargement/multi-monitor DPI remain
unrun; record the exact missing checks and request operator action only after local blockers are
fixed. The automated integration matrix is now recorded, but does not close UIR-03 by itself.

Use the [multi-session guide](codex-session-guide.md), [compact checkpoint](session-checkpoint.md)
and [kickoff prompt](session-kickoff-prompt.md). UIR-04/07/08 also read
[the long-scan/rescan contract](scan-and-rescan-experience.md). One gate may span several commits;
record partial progress without calling the gate complete. Run UIR-04 next after UIR-03.

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

Do not declare the redesign complete with only a prototype or passing unit tests. UIR-09 requires
the actual scoped WPF behavior, coherent states, meaningful regression checks, physical accessibility
evidence required by the validation plan, and user acceptance. Distinguish:

- Planning complete: direction and operator feedback captured; local implementation ready.
- Implemented: code exists and relevant automated checks pass.
- Native acceptance pending: physical desktop/user checks remain.
- Redesign complete: scoped product is accepted and committed; production deletion remains disabled.

The parked release-validation stream can still be incomplete after redesign completion. Do not
weaken its thresholds, infer provider or Recycle Bin authority, alter consumed evidence identities,
or make broad 'release ready' claims from this separate UI work.
