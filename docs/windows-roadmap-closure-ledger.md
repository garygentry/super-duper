# Windows Post-MVP Roadmap Closure Ledger

## Status

Scaffold only. The next roadmap-control goal must populate and verify this ledger before another
product-code slice is selected. This file does not authorize implementation, physical acceptance,
provider campaigns, production Recycle Bin execution, or a claim that any unlisted gate is closed.

The authoritative product criteria remain in `windows-post-mvp-ux-plan.md`. The active checkpoint
and one-session authorization remain in `windows-roadmap-session-handoff.md`.

## Completion contract

The bootstrap goal must define the exact Windows post-MVP completion boundary for Milestones 7-14,
including whether completion means code complete, operator accepted, or production enabled. It must
also resolve:

- the current Milestone 9 status and remaining work;
- whether unavailable Milestone 7 opt-in cloud policies are required or reviewed follow-ons;
- which Milestone 14 items are required closure work versus optional follow-ons; and
- how independent Milestone 8 gates constrain Milestone 11 production enablement.

The roadmap is not complete until every required gate below is `accepted`, every excluded gate has
a cited reviewed `deferred` decision, and all authoritative milestone statuses agree.

## Field definitions

| Field | Required content |
|---|---|
| Gate ID | Stable `WPM<Milestone>-<short-name>` identifier; never renumber a published gate. |
| Milestone | Owning Milestone 7-14. |
| Criterion or decision | Exact acceptance bullet, open decision, or required closure outcome. |
| Disposition | `local_audit`, `local_code`, `operator_evidence`, `design_decision`, `blocked`, `deferred`, or `accepted`. |
| State | `open`, `ready`, `in_progress`, `locally_exhausted`, `blocked`, `deferred`, or `accepted`. |
| Dependencies | Gate IDs or named external prerequisites that must be satisfied first. |
| Evidence | Commit, test, report, artifact, operator record, or reviewed decision; never inference alone. |
| Owner/prerequisite | Agent-local, operator, provider fixture, representative hardware, or explicit reviewer. |
| Next action | One concrete bounded action; never "find another gap". |
| Completion check | Command, artifact, observation, or reviewed decision that closes the gate. |

## Gate ledger

Populate one row for every acceptance criterion and still-open decision in Milestones 7-14. Split a
criterion only when its parts have different dependencies, owners, or verifiers. Group work only
when the rows share one implementation boundary and completion check.

| Gate ID | Milestone | Criterion or decision | Disposition | State | Dependencies | Evidence | Owner/prerequisite | Next action | Completion check |
|---|---:|---|---|---|---|---|---|---|---|
| _Bootstrap required_ | - | Inventory and classify all remaining criteria and decisions. | `local_audit` | `ready` | None | Authoritative plans and current repository | Agent-local audit | Populate this ledger without product-code changes. | Every Milestone 7-14 criterion and decision is represented and cross-checked. |

## Milestone reconciliation decisions

Record the reviewed scope/status decision for each milestone, with links to its gates and evidence.

| Milestone | Current authoritative status | Required completion meaning | Remaining gate IDs | Reviewed disposition |
|---:|---|---|---|---|
| 7 | _Populate_ | _Populate_ | _Populate_ | _Populate_ |
| 8 | _Populate_ | _Populate_ | _Populate_ | _Populate_ |
| 9 | _Populate_ | _Populate_ | _Populate_ | _Populate_ |
| 10 | _Populate_ | _Populate_ | _Populate_ | _Populate_ |
| 11 | _Populate_ | _Populate_ | _Populate_ | _Populate_ |
| 12 | _Populate_ | _Populate_ | _Populate_ | _Populate_ |
| 13 | _Populate_ | _Populate_ | _Populate_ | _Populate_ |
| 14 | _Populate_ | _Populate_ | _Populate_ | _Populate_ |

## Dependency-ordered queue

After populating the ledger, list only ready or externally blocked work in dependency order. The
first ready row becomes the only gate authorized by the next session handoff.

| Order | Gate ID or coherent gate group | Why it is next | Required verifier or prerequisite |
|---:|---|---|---|
| 1 | _Populate_ | _Populate_ | _Populate_ |

## Audit and anti-spin record

For each bounded audit that finds no local work, record the gate, inspected evidence, date, and
`locally_exhausted` conclusion. Do not repeat the audit until named new evidence, code, or a reviewed
decision changes its boundary.

| Date | Gate ID | Evidence inspected | Conclusion | Reopen condition |
|---|---|---|---|---|

## Decision log

| Date | Gate ID(s) | Decision or evidence accepted | Resulting next gate |
|---|---|---|---|
