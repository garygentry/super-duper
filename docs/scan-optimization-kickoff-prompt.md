# Scan Optimization New-Session Kickoff Prompt

This stream is active at the user-approved `SOP10-large-folder-analysis-remediation`. The prompt
authorizes local implementation, deterministic/synthetic verification, builds, documentation, and
bounded commits. It does not authorize rerunning consumed SOP9c identities or launching a new
physical full-drive scan.

Copy the prompt below into a new coding session. It is intentionally state-independent: the agent
must read the committed checkpoint and continue from it, so reusing the prompt never restarts
accepted work.

```text
Execute the active SOP10 large-folder-analysis remediation plan in this repository.

First follow AGENTS.md and read docs/windows-roadmap-session-handoff.md plus
docs/scan-optimization-plan.md completely. Audit HEAD, the worktree, the latest commit, and the
plan's current execution checkpoint. Treat Git and cited verification as truth. Do not redo an
accepted work package or infer state from this prompt. Preserve the operator's unrelated README.md
change. Confirm no Super Duper app/worker process is active before any build or runtime action; the
operator deliberately cancelled and closed the prior app.

Resume at SOP10b-streaming-exact-folder-analysis, the first dependency-ready package named by the
scan plan. Progress through as many
dependency-ready packages as can be implemented and verified coherently in this session; do not
stop after a narrow finding or one commit merely because a later package exists. Keep each commit
bounded and update the package ledger, execution checkpoint, verification baseline, and session
handoff after every completed package or coherent package group.

Honor the approved design choices: exact duplicate folders only (no automatic Jaccard similarity),
bounded streaming/bottom-up analysis with no per-directory scanned-file query or per-ancestor file
clone, and performance-first scan-resistant cache generations. Preserve reuse_verified as the
default and revalidate_content as the user override. The first fixed run must conservatively hash
legacy-cache misses once because the old cache lacks the current stable identity/change-token proof;
subsequent unchanged runs must reuse qualified partial/full hashes. Do not weaken signature checks,
hard-link semantics, cancellation, cloud exclusion, warning truth, or production deletion locks.

Implement the published SOP10 packages and their completion checks, including bounded folder-
analysis substage progress and the deterministic Release-scale profile near 3.55 million files and
633,000 directories. Use small fixtures while iterating. Retain the first scale result; do not tune
and rerun merely for a favorable number. Stop at a verified fixed Release build and operator rerun
checklist. Do not launch a real full-drive scan, delete or overwrite the cancelled run database or
legacy cache, or reuse SOP9c V1/V2 campaign identities without separate explicit operator approval.

Pause only for a real external/user-decision blocker, a safety/authority boundary, irreparable
required verification failure, or when continued work is likely to cause context degradation that
risks an incomplete audit, unsafe edit, or unreliable handoff. Before pausing, leave completed work
verified, documented, committed, and the worktree clean. Apply the plan's idempotence and anti-spin
rules: audit once per package, do not manufacture progressively narrower follow-ups after criteria
pass, and after two identical failed attempts record the blocker and continue another ready package
in the active stream when possible.

The Windows post-MVP release-validation stream remains parked. Do not select its gates or alter its
production Recycle Bin locks. Report completed commits, verification, current package state, the
exact next package or blocker, expected first-run versus repeat-run behavior, and whether the fixed
build is ready for separately authorized physical acceptance.
```
