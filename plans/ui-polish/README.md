# Windows usability and visual polish

Status: implementation, background verification and physical appearance/focus checks complete;
UX23's transient native loading observation remains pending. Started 2026-09-15
(America/Los_Angeles). Branch: `codex/ui-redesign`; baseline `6206610`.

The 2026-09-17 continuation from `2f9679a` regained native launch, capture and input.
Bounded scan 4 opening and scan 6 member paging captured only settled frames without overlap.
P08 remains open for UX23's loading-frame observation. Accepted checks remain valid.

The operator requested a new end-to-end usability effort after accepting UIR-09: reduce clutter,
make common tasks intuitive, establish consistent visuals and icons, exercise the real app with
real files autonomously, and fix underlying engine defects when necessary. This is a new stream;
the completed redesign and its historical acceptance remain intact.

## Read in order

1. [Review and evidence](review.md): feature inventory, findings, observed limitations.
2. [Execution plan](execution-plan.md): accepted design, finite gates, acceptance and autonomy.
3. [Checkpoint](session-checkpoint.md): exact next work and verification state.

## Accepted decisions

| Decision | Recommended direction | Status |
|---|---|---|
| Primary workflow | Balance large duplicate-file review and backup/archive folder comparison | Accepted |
| Visual style | Restrained Windows-native; clear hierarchy, compact navigation, generous content area | Accepted |
| Meaning of ready | Polish existing capabilities, retaining disabled file removal | Accepted |

The operator also accepted flexible navigation, manual decisions plus prominent optional location
preferences, adaptive layouts down to 900×600, remembered section disclosure, context-aware navigation
to completed results, mixed real archive data and a required final native mouse/keyboard pass.
The operator explicitly authorized implementation with subagents and appropriate model/effort
selection. P01 tooling and P02 independent foundations may proceed concurrently; integration and
acceptance remain sequential. No intermediate operator design approval is required.

If actual Recycle Bin execution is requested, expand the plan explicitly with execution, recovery,
and the inherited release/provider safety gates before implementing that capability. Do not silently
interpret UI polish as enabling deletion or claim a removal-disabled app is ready for actual cleanup.

After these decisions are settled, ordinary layout, wording, icons, architecture-preserving
refactors, isolated data preparation, tests and necessary defect fixes proceed without repeated
operator design sign-off. Final quality assessment belongs to the agent under the acceptance
contract; user feedback remains welcome, not a mandatory gate at every slice.
