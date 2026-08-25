# Scan Optimization New-Session Kickoff Prompt

Copy the prompt below into a new coding session. It is intentionally state-independent: the agent
must read the committed checkpoint and continue from it, so reusing the prompt never restarts
accepted work.

```text
Continue the active large-drive scan optimization and observability roadmap in this repository.

First follow AGENTS.md and read docs/windows-roadmap-session-handoff.md plus
docs/scan-optimization-plan.md completely. Audit HEAD, the worktree, the latest commit, and the
plan's current execution checkpoint. Treat Git and cited verification as truth. Do not redo an
accepted work package or infer state from this prompt.

Resume at the first dependency-ready work package named by the scan plan. Progress through as many
dependency-ready packages as can be implemented and verified coherently in this session; do not
stop after a narrow finding or one commit merely because a later package exists. Keep each commit
bounded and update the package ledger, execution checkpoint, verification baseline, and session
handoff after every completed package or coherent package group.

Pause only for a real external/user-decision blocker, a safety/authority boundary, irreparable
required verification failure, or when continued work is likely to cause context degradation that
risks an incomplete audit, unsafe edit, or unreliable handoff. Before pausing, leave completed work
verified, documented, committed, and the worktree clean. Apply the plan's idempotence and anti-spin
rules: audit once per package, do not manufacture progressively narrower follow-ups after criteria
pass, and after two identical failed attempts record the blocker and continue another ready package
in the active stream when possible.

The Windows post-MVP release-validation stream remains parked. Do not select its gates or alter its
production Recycle Bin locks. Use small deterministic fixtures during development and reserve full
large-drive runs for explicit acceptance. Report completed commits, verification, current package
state, and the exact next package or blocker.
```
