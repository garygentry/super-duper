# Windows Release-Validation New-Session Kickoff Prompt

Copy the prompt below into a new coding session. It is intentionally state-independent: the agent
must audit the committed checkpoint and continue from the ledger rather than trusting the prompt's
historical expectations.

```text
Continue the active Windows post-MVP release-validation roadmap toward its reviewed completion
contract.

First follow AGENTS.md. Audit HEAD, the worktree, recent history, and the complete latest commit.
Read docs/windows-roadmap-closure-ledger.md completely. In
docs/windows-roadmap-session-handoff.md, read only Session objective, Current checkpoint, Immediate
next step, Required startup audit, Non-negotiable boundaries, and Completion loop; skip its
historical accepted-slice record and decision log unless the selected gate cites them. Then read only
the first dependency-ready gate's sections in docs/windows-post-mvp-ux-plan.md and its directly
linked acceptance procedure. Treat Git, the ledger, retained evidence, and cited verification as
truth. Do not replay accepted historical slices or mine full iteration logs.

Select the ledger's first dependency-ready non-accepted gate. State its exact prerequisites,
authority, bounded action, completion check, non-goals, and production-lock impact before acting.
Active scheduling is not physical/provider/performance-campaign or WPM11 production-wiring
authority. Obtain every separate explicit approval required by the selected row. Preserve the parked
scan stream at SOP9c blocked_invalid_campaign; do not rerun V1/V2, design a successor, start SOP9d,
or substitute scan work.

Work efficiently through as many dependency-ready local packages as can be completed and verified
coherently. Keep each commit bounded to one gate or inseparable gate group, update the ledger,
product plan, ROADMAP, and handoff after each completed gate/group, and continue immediately when
the next local gate is ready and no gate-specific stop or authority boundary intervenes. Do not stop
after one narrow passing check merely because later packages exist.

Apply the audit-once and anti-spin rules. Never reopen accepted or locally_exhausted work without
its documented reopen condition. Do not manufacture progressively narrower audits, tests, state
combinations, or follow-ups after the named completion criterion passes. Retain the first qualifying
physical/provider/performance outcome, including failure; never rerun merely until green. After two
identical failures with no new evidence, preserve the blocker and smallest next experiment, then
advance only to an explicitly independent ready lane if the ledger authorizes it.

Keep production Recycle Bin execution disabled: CanSubmit remains false, production continues to
inject DisabledRecycleOperationCapabilityExecutor, every worker response remains
executorEnabled:false, and no Move to Recycle Bin now action exists. WPM11-production-wiring requires
all dependencies plus a later distinct product/safety approval.

Before handing off, run proportional verification and git diff --check, review every safety and
authority boundary, commit all completed in-scope work, and leave the worktree clean. Report commits,
gate states, retained evidence, verification, skipped gates, the exact next dependency-ready gate or
blocker, and every operator decision still required.
```
