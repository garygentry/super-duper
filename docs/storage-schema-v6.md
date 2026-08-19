# Storage Schema V6

Schema version 6 adds durable manual exact-folder-copy decisions while preserving schema-v5 file
decisions and the single active review plan per immutable completed run. Rust remains the only
owner of product SQLite. Windows clients use the versioned worker protocol.

## Transactional migration

Opening a version-5 database runs one `BEGIN IMMEDIATE` migration. It creates the folder-decision
table, its separate idempotency ledger, and all supporting indexes before setting
`user_version = 6`, then commits. Failure rolls the migration back to an unchanged valid v5
database. Version-2, version-3, and version-4 databases still migrate forward in order; unknown
older and newer versions remain fail-closed.

Deleting a run cascades both decision kinds and command ledgers. Database truncation clears folder
and file command/decision rows before plans and immutable scan rows.

## Folder-copy decisions

- `review_folder_decision` belongs to a plan, visible exact-folder group, immutable
  `duplicate_folder_group_member`, and its `directory_node` root. It stores `keep`, `remove`, or
  `undecided`, manual provenance/time, path and descendant totals, and the structural/verified
  fingerprints used for the choice.
- `review_folder_command` stores the exact folder-specific mutation payload and applied shared plan
  revision. It does not overload the file-specific `review_command` foreign keys or payload.
- `idx_folder_group_member_directory` supports decision-proportional ancestor/descendant overlap
  and intact-copy aggregation without scanning every folder copy.

The stable member/directory IDs establish immutable ownership. Snapshot fields explain the scanned
copy and are never refreshed from the live filesystem.

## Shared revision and safety

File and folder mutations advance the same `review_plan.revision`. Exact command replays are
resolved before the current-revision check; reuse with another payload is rejected.

The transaction computes one effective removal union from explicit file removals and immutable
file paths below removed folder roots. It rejects:

- Keep/Remove containment conflicts and redundant nested removals;
- any duplicate-file set without an accessible physical survivor, deduplicating non-empty file
  identities as hard-link aliases;
- any visible or suppressed exact-folder set without an intact independently accessible copy.

Suppressed groups participate in safety but are not directly mutable in this slice. Combined
logical and physical totals use distinct immutable file IDs and physical keys, so folder/file
overlaps and hard links cannot be counted twice. These rows are review state only: they are not a
deletion schedule and do not use `scanned_file.marked_deleted` or legacy `deletion_plan` truth.

## Bounded reads

`review_plan.get` returns one fixed-size combined file/folder summary for the active plan revision.
`review_folder_group.page` uses a maximum page size of 500 and a forward keyset cursor bound to run,
plan, revision, page size, and visible-group mode. Exact-folder member pages remain bounded and now
carry folder decisions, the selected-set summary, and revision-bound next/previous cursors.

All calculations use immutable run-owned SQLite rows. They do not enumerate, validate, preview,
hydrate, or mutate the live filesystem or excluded cloud placeholders, and no deletion command is
exposed.
