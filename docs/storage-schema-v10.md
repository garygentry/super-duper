# Storage Schema v10

Schema version 10 adds Rust-owned durable records for a revision-bound Recycle Bin operation. The
implemented foundation is deliberately non-mutating: it can prepare, classify, confirm, page,
cancel, report injected outcomes, and reconstruct ambiguous state, but the production Windows
executor is disabled and no Shell deletion API is called.

## Migration, backup, and downgrade

Opening a version-9 database runs one `BEGIN IMMEDIATE` migration. It creates
`recycle_operation`, `recycle_operation_batch`, `recycle_operation_item`,
`recycle_operation_report`, and `recycle_operation_recovery` plus bounded-query indexes, then sets
`user_version` to 10 and commits. Any failure rolls the entire migration back to version 9. Tests
exercise successful migration, rollback after a deliberate name collision, historical migrations,
and fail-closed rejection of unknown newer versions.

Before first opening an existing database with this build, close the app and worker and back up the
main database, `-wal`, and `-shm` files as one set. There is no in-place downgrade. To return to a
v9 build, close every process and restore the complete pre-migration backup; do not lower
`user_version`, drop v10 tables manually, or open a v10 database with an older worker. Operation
evidence created after migration is necessarily absent from the restored backup.

## Domain separation

The five v10 tables contain operation intent and evidence only. They do not rewrite reusable rule
configuration, rule-application provenance, manual review decisions, immutable preflight
observations, immutable scan history, legacy `deletion_plan`, `scanned_file.marked_deleted`, or any
future Milestone 12 live-state overlay. Foreign keys bind an operation to its immutable run, active
review plan/revision, completed preflight generation, preflight sources, and snapshot entities.

## Header and state machine

`recycle_operation` stores the canonical idempotency key, run/plan/revision/preflight binding,
preflight and intent signatures, fixed counts/bytes/location/exclusion summary, policy version,
freshness timestamps, cancellation flag, and structured terminal error. States are `prepared`,
`awaiting_confirmation`, `submitted`, `executing`, `cancelling`, `expired`, `cancelled`,
`completed`, `partially_completed`, `failed`, and `recovery_required`.

Preparation accepts only the latest completed preflight for the current active review revision,
within a provisional five-minute lease, with every removal observation `ready`. One non-ready or
later generation fails the whole plan. Exact operation-ID replay returns the original row;
payload reuse conflicts. A review/provenance/new-preflight mutation expires unsubmitted intent;
submitted, executing, cancelling, and recovery-required operations lock those mutations. Run/session
deletion remains blocked while operation evidence is active or ambiguous.

The current five-minute preparation, 60-second confirmation, 30-second submission/admission, and
32-entry file-batch values are provisional implementation constants pending the separately listed
operator and performance gates. Exact folders are isolated into one-item batches. These values are
not yet accepted product constants.

## Items, batches, reports, and recovery

`recycle_operation_item` materializes one top-level selected directory entry. File entries retain
their logical source and immutable physical key; selected hard-link aliases remain separate Shell
entries while physical counts/bytes stay de-duplicated. Exact-folder roots are top-level entries;
descendants remain preflight evidence rather than duplicate Shell items. Eligibility and result
states are distinct and include explicit `non_recyclable` and `unknown` outcomes.

`recycle_operation_batch` bounds transport and records an item signature, ordinal, provisional
admission expiry, Shell-attempt ID, and durable start/report timestamps. `recycle_operation_report`
is the canonical replay ledger for eligibility, confirmation, batch-begin, result, and future
recovery reports. Report payload signatures sort item observations by durable item ID, so equivalent
orderings replay identically and changed payloads fail.

`recycle_operation_recovery` records per-item ambiguity. On startup, an operation abandoned before
submission expires. A submitted/executing/cancelling operation becomes `recovery_required`; each
pending item in a `shell_started` batch becomes `unknown`, the batch becomes `ambiguous`, and a
recovery row explains that mutation may have occurred without a durable callback. Retrying such an
operation is intentionally unavailable, preventing an already completed Shell mutation from being
repeated.

## Current safety boundary

The worker contract exposes bounded, allow-listed state transitions for deterministic tests, but
every response reports `executorEnabled:false`. The WPF surface is reconstruction and disclosure
only. Infrastructure injects `DisabledRecycleOperationCapabilityExecutor`, which returns
`non_recyclable/executor_disabled` without opening, canonicalizing, enumerating, hydrating, or
creating a Shell item for any path. No `IFileOperation`, `SHFileOperation`, Recycle Bin, move,
delete, scheduling, or permanent-delete code exists in this slice.
