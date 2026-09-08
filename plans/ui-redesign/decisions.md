# Decisions and open questions

Date: 2026-09-08. 'Recommended' means the design selected by this evaluation; it is not silently
promoted to operator acceptance. User instructions outrank these planning defaults.

| ID | Decision | State / rationale |
|---|---|---|
| D01 | Preserve `wpf-poc`; keep all redesign work on `codex/ui-redesign` until completion | Explicit user instruction; implemented at `deefa40` plus new branch |
| D02 | Continue evaluation into a durable design/specification package | Explicit user instruction; fulfilled by this package |
| D03 | Keep WPF/.NET 10 Fluent and Rust worker boundary | Recommended; no evidence justifies migration |
| D04 | Scan / Results / Review / History with contextual diagnostics | Recommended; resolves capability-oriented navigation |
| D05 | Keep saved configurations but call them Saved scans | Recommended; distinguish each dated scan in persistent context |
| D06 | Results use list/detail at standard width and staged list/detail at narrow width | Recommended; comparison and decisions take priority over permanently expanded filters |
| D07 | Keep exact-only results and current backend safety/ownership/page contracts | Required inherited invariant |
| D08 | Scope initial redesigned release to review and non-deleting validation | Required current production boundary; polish is not Recycle Bin authorization |
| D09 | Use existing separate file/folder review group pages | Recommended; avoids unbounded combined removal-list assumptions |
| D10 | Start saves valid setup; explicit Save remains; dirty navigation offers Save/Discard/Stay | Recommended interaction refinement; no automatic persistence of every edit |
| D11 | No new thumbnails, preview, export, saved filters, pause/resume or global select-all | Scope control; existing deferred features remain deferred |
| D12 | Prototype uses fictional local state and explicit scenario controls | Implemented artifact boundary; all engine/file actions are illustrative |
| D13 | Accept the high-level workspace direction, with long-scan/rescan refinements | User: “This looks good” on 2026-09-08. D03-D06/D09-D10 remain the chosen direction; detailed native usability acceptance is still pending |
| D14 | Long scans are a primary workflow: live phase/current activity/progress with expandable diagnostics | Explicit user requirement; `scan-and-rescan-experience.md`, A08/A16; honest measured values, no invented whole-scan ETA |
| D15 | Repeat scans discover current files and retain qualifying persisted hash work across runs | Explicit user requirement; A17. Existing engine reuse is retained; new/deleted/changed files affect new results, not historical snapshots |
| D16 | Use bounded local slices, committed handoffs and one checkout/branch across Codex sessions | Recommended execution procedure in `codex-session-guide.md`; no additional task or automation created |

## Direction feedback (UIR-02 complete)

The operator accepted the high-level direction and requested stronger support for hours/days-long
scans, real-time activity, optional technical detail, and repeat scans that preserve hash work while
accounting for added/deleted files. These are now explicit screen requirements and acceptance cases.
UIR-02 closes as direction feedback, without asserting that a full interactive walkthrough occurred.
UIR-03 is the next local implementation gate. Native/user acceptance remains UIR-08/09.

This update prepares the requested requirements and multi-session procedure; no WPF implementation
starts here. Cosmetic preferences can evolve during implementation without repeated approval stops.
Physical/provider/production authority remains distinct.

## Questions deferred to evidence, not user guesswork

- Actual cause of the observed completed-session loading delay: characterize with isolated delayed
  responses and fixture-backed native UI during UIR-03, without mutating the production session.
- Final split sizes and text wrapping: decide from WPF measurements at supported sizes during UIR-05.
- Any current-versus-historical counter discrepancy: preserve separate meanings and verify paired
  source/build behavior before proposing data-contract changes.
- High contrast and physical multi-monitor behavior: validate in UIR-08; source brushes and a browser
  concept cannot settle these questions.
