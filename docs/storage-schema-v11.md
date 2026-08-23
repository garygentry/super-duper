# Storage Schema v11

Schema version 11 adds one append-only `recovery_review_observation` table for the accepted WPM11
Option A operator-review model. The table is separate from immutable schema-v10 operation evidence.
It grants no filesystem, provider, Shell, Recycle Bin, restore, retry, replay, deletion, or outcome-
resolution authority.

## Migration, backup, and downgrade

Opening a version-10 database runs one `BEGIN IMMEDIATE` migration. It creates the table and its
operation/item and supersession indexes, sets `user_version` to 11, and commits. Any failure rolls
the complete migration back to version 10. Unknown newer schemas remain rejected without
modification. Run deletion cascades through the operation/item ownership chain and removes review
rows; normal review APIs never update or delete an observation.

Before first opening an existing database with this build, close the app and worker and back up the
main database, `-wal`, and `-shm` files as one set. There is no in-place downgrade. To return to a
v10 build, restore the complete pre-migration backup; do not lower `user_version` or remove v11
objects manually. Operator observations created after migration are absent from that backup.

## Append-only observation contract

Each row stores a unique bounded request ID and payload signature, operation and immutable unknown-
item foreign keys, one of the five approved observation kinds, the operator-supplied RFC 3339
timestamp, an optional bounded note, evidence version 1, and a server creation time. A correction
adds a new row whose unique `supersedes_observation_id` names the prior current row and whose bounded
`correction_reason` explains the correction. The prior row remains unchanged.

Current projection is the row with no successor. Review state is derived rather than persisted:

- no current observations: `not_started`;
- some but not all unknown items observed: `in_progress`;
- every unknown item observed: `review_complete_with_unresolved_evidence`.

The final state deliberately remains unresolved. Recording or superseding an observation never
changes `recycle_operation.status = recovery_required`,
`recycle_operation_batch.status = ambiguous`, `recycle_operation_item.result_status = unknown`, or
the original `recycle_operation_recovery` row.

## Bounded and non-inspecting behavior

Mutation validates all fields and ownership before a single transactional insert. Exact request
replay is inert; conflicting payload reuse and stale/cross-item supersession fail without writes.
Observation pages are limited to 1–200 rows and separately expose current projection or complete
history. All review methods use only persisted SQLite fields. They cannot inspect or infer live
source, provider, content, or Recycle Bin state and cannot invoke any operation transition.

The persistence gate itself added no WPF controls. The separately accepted
`WPM11-recovery-review-ui` workflow now uses this contract only for bounded manual observations and
append-only corrections. Production execution remains disabled:
`RecycleOperationViewModel.CanSubmit` remains false, composition still uses
`DisabledRecycleOperationCapabilityExecutor`, and every worker response reports
`executorEnabled:false`.
