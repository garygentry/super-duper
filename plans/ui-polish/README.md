# Windows usability and visual polish

Status: initial review and proposed execution plan; product decisions pending. Started 2026-09-15
(America/Los_Angeles). Branch: `codex/ui-redesign`; baseline `6206610`.

The operator requested a new end-to-end usability effort after accepting UIR-09: reduce clutter,
make common tasks intuitive, establish consistent visuals and icons, exercise the real app with
real files autonomously, and fix underlying engine defects when necessary. This is a new stream;
the completed redesign and its historical acceptance remain intact.

## Read in order

1. [Review and evidence](review.md): feature inventory, findings, observed limitations.
2. [Execution plan](execution-plan.md): proposed design, finite gates, acceptance and autonomy.
3. [Checkpoint](session-checkpoint.md): exact next work and pending operator decisions.

## Decisions requested after the initial review

| Decision | Recommended direction | Status |
|---|---|---|
| Primary workflow | Balance large duplicate-file review and backup/archive folder comparison | Pending |
| Visual style | Restrained Windows-native; clear hierarchy, compact navigation, generous content area | Pending |
| Meaning of ready | Polish existing capabilities, retaining disabled file removal | Pending |

If actual Recycle Bin execution is requested, expand the plan explicitly with execution, recovery,
and the inherited release/provider safety gates before implementing that capability. Do not silently
interpret UI polish as enabling deletion or claim a removal-disabled app is ready for actual cleanup.

After these decisions are settled, ordinary layout, wording, icons, architecture-preserving
refactors, isolated data preparation, tests and necessary defect fixes proceed without repeated
operator design sign-off. Final quality assessment belongs to the agent under the acceptance
contract; user feedback remains welcome, not a mandatory gate at every slice.
