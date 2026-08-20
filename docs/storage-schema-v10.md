# Storage Schema v10

Schema version 10 adds Rust-owned durable records for a revision-bound Recycle Bin operation. The
implemented operation surface remains disabled in production: it can prepare, classify, confirm,
page, cancel, report outcomes, and reconstruct ambiguous state, while the WPF composition still
injects the disabled executor and exposes no submission action. A separately gated Windows
executor adapter and disposable acceptance tests now exercise Shell only when explicitly invoked.

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

The current five-minute preparation, 60-second confirmation, 30-second admission, and
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
admission expiry, Shell-attempt ID, and durable start/report timestamps. `batch.next` now reruns
the immutable target, complete-hash, exact-folder-tree, affected-file-survivor, and affected-folder-
survivor checks before changing a batch from `pending` to `admitted`; an expired lease returns the
batch to `pending` so admission must run again. Snapshot identity, size, and nanosecond modified
time are projected with an admitted batch for the Infrastructure `PreDeleteItem` check.
`recycle_operation_report`
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

Every worker response still reports `executorEnabled:false`; WPF remains reconstruction and
disclosure only, `CanSubmit` remains false, and application composition still injects
`DisabledRecycleOperationCapabilityExecutor`. The real `WindowsRecycleOperationExecutor` is not
production-wired. Explicit tests may invoke it on a dedicated STA thread after a successful local-
root `SHQueryRecycleBinW` query and non-opening ordinary-item classification. It requires a fresh
`admitted` batch, calls the durable-start acknowledgement after declarative `DeleteItem` queuing
and before `PerformOperations`, maps `PreDeleteItem`/`PostDeleteItem`/`FinishOperations` and abort
evidence, and never offers a permanent-delete fallback. `FOFX_ADDUNDORECORD` is intentionally not
set pending its unresolved evidence review.
