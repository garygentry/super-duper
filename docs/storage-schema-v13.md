# Storage Schema v13

Schema version 13 adds the bounded dirty-root and watcher-overflow persistence required by
`WPM12-watcher-overflow`. It extends the accepted schema-v12 working overlay without rewriting
immutable scan rows, recorded manual/rule decisions, or any Recycle Bin operation evidence.

## Migration, backup, and downgrade

Opening a version-12 database runs one `BEGIN IMMEDIATE` migration. It creates
`review_live_root_state`, `review_live_root_overflow`, `review_live_root_reconciliation`, and
`review_live_root_reconciliation_item`. It rebuilds `review_live_file_state` so a latest working
observation is owned by exactly one schema-v12 explicit validation or one schema-v13 root
reconciliation, recreates the effective-decision view and indexes, sets `user_version` to 13, and
commits. Failure rolls the entire migration back to version 12. Unknown newer schemas remain
rejected without modification.

Close the app and worker before backing up an existing database, and copy the main database, `-wal`,
and `-shm` files together. There is no in-place downgrade. Restore the complete pre-migration backup
to return to a v12 build; never lower `user_version` or remove v13 objects manually.

## Durable dirty-root state

`review_live_root_state` stores at most one latest row for each immutable selected root in a
completed run. Runs accept at most 64 selected roots, so restart reconstruction is bounded without
paging or filesystem enumeration. `state=dirty`, `reason_code=watcher_overflow`, a monotonic
`dirty_revision`, the overflow time, the last committed reconciliation cursor, and the cumulative
bounded item count make loss of watcher coverage explicit. A later overflow increments the dirty
revision and resets reconciliation progress; it never leaves an earlier clean result trusted.

`review_live_root_overflow` is an idempotent operation ledger. A report must name a completed run
and one exact root from its immutable parameter snapshot. Replaying the same operation/payload does
not increment the dirty revision. Reusing the operation ID with another run/root, naming a session's
newer edited roots, or reporting against an incomplete run fails without writes.

## Explicit bounded reconciliation

`review_live_root_reconciliation` owns one explicit request for one dirty revision, exact review
revision, and page size from 1 through 200. The server resumes from the root state's durable file-ID
cursor, selects only duplicate-group members whose immutable `scanned_file.root_path` matches that
root, and validates at most the requested count. The item ledger retains the exact returned batch
for idempotent replay. The request has no whole-result response and WPF binds only its already
selected member page.

Filesystem observations use the accepted schema-v12 metadata/stable-identity validator and classify
immutable run exclusions before access. Missing or changed copies preserve the recorded decision
but invalidate its working projection; unavailable copies retain their decision. Each item updates
`review_live_file_state` with `reconciliation_id` and clears `validation_id`, while a later explicit
page/set validation performs the inverse. Exactly one source owns every latest observation.

At commit, storage repeats completed-run, selected-root, dirty revision, review revision, and cursor
checks. A concurrent overflow, review mutation, or reconciliation batch therefore rejects the stale
request without overlay writes. The root stays visibly dirty while another batch exists and becomes
clean only in the same transaction that commits the final bounded batch. Cancellation or a late WPF
response cannot replace another run/root/review context; committed storage progress remains
restart-safe.

No schema-v13 table registers a watcher, emits one notification per event, binds a full result set,
opens file content, mutates the filesystem, invokes Shell or Recycle Bin, or authorizes production
execution. `RecycleOperationViewModel.CanSubmit` remains false, production composition uses
`DisabledRecycleOperationCapabilityExecutor`, every worker operation response reports
`executorEnabled:false`, and no **Move to Recycle Bin now** action exists.
