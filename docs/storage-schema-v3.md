# Storage Schema V3

Schema version 3 is the persistence foundation for the Windows MVP. SQLite remains owned by Rust;
the Windows application will access it only through the worker protocol.

## Sessions and runs

`scan_session` is a reusable named definition. Names are unique with SQLite `NOCASE` collation.
Editing a session changes its current roots and ignore patterns only.

`scan_run` is an immutable execution belonging to one session. The engine inserts a `pending` run
before filesystem traversal, stores a JSON snapshot containing roots, ignore patterns, and the
directory-analysis threshold, then transitions it to `running`. Result tables reference the run
directly or through another run-owned row.

Supported durable states are `pending`, `running`, `cancelling`, `completed`, `cancelled`, `failed`,
and `interrupted`. Opening the product database reconciles abandoned `running` or `cancelling` rows
to `interrupted`. Terminal rows have `completed_at`; failed/interrupted rows retain an error
message. A failed or cancelled pipeline cannot transition to `completed` through the storage API.

Counters have independent meanings:

- `files_discovered` and `bytes_discovered` come from traversal, including non-duplicate files.
- `files_hashed` counts files successfully processed by the hashing phase.
- `duplicate_file_groups`, `duplicate_folder_groups`, and `wasted_bytes` describe results.
- `warning_count` aggregates recoverable traversal, hashing, and persistence warnings.

## Result ownership

`scanned_file` is a per-run snapshot with a unique `(run_id, canonical_path)` key. It includes the
selected root, root-relative path, metadata, optional content hash, optional stable identity field,
and an optional warning. `duplicate_group` owns one run and membership can only resolve file rows
from that same run.

`directory_node` and `directory_similarity` also carry `run_id`; fingerprints are owned transitively
through their directory. This prepares folder results for the exact duplicate-folder algorithm,
which remains deferred.

The RocksDB content-hash cache remains global because it is an optimization rather than historical
result state.

## Duplicate-file result queries

Milestone 4 exposes `duplicate_group` and its run-owned `scanned_file` members only through the Rust
worker. Group and member pages use keyset boundaries composed from an allow-listed sort value and a
stable row ID, never a caller-provided SQL fragment or a large numeric offset. Filtered counts are
computed by the worker-owned query, and path searches are literal case-insensitive substring
matches. Completed runs are immutable, so continuation cursors cannot observe rows moving between
pages.

The default group order is recoverable bytes descending and group ID ascending. Supporting indexes
cover run plus recoverable bytes, file size, and copy count; member lookup uses the group membership
index. Representative names are derived deterministically from the first member path rather than
stored as mutable UI metadata.

## Forward migration

Opening a version 2 database performs one `BEGIN IMMEDIATE` migration to version 3. The migration
renames the legacy tables, creates the v3 schema, copies recoverable records, validates foreign
keys, and drops legacy tables only within the transaction. A failure rolls back instead of
recreating or truncating the database.

Each legacy `scan_session` becomes an `Imported scan <id>` definition and a run with the same ID.
Completed/cancelled/failed states are retained; legacy active states become `interrupted`. Legacy
roots become the run snapshot and counters are carried forward with their legacy accuracy.
Duplicate groups and members are reconstructed as run-owned snapshots.

The v2 schema had two unavoidable information losses before migration:

- File metadata was stored in one mutable row per canonical path, so overwritten metadata from
  earlier scans cannot be recovered. The migration copies the last available row into every
  recoverable group/run association and marks it with a migration warning.
- Directory analysis had no session owner and represented the latest global analysis. It is
  assigned to the newest migrated run. Existing deletion-plan entries are mapped to the newest
  recoverable snapshot of their path.

Databases with user tables at schema version 0 or 1 are rejected without modification because their
semantics cannot be identified safely. Versions newer than 3 are also rejected. Migration and
schema-version behavior are covered by focused storage tests.
