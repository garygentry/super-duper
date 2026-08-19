# Storage Schema V5

Schema version 5 adds durable review state for immutable completed runs. Rust remains the only
owner of product SQLite; Windows clients use the versioned worker protocol. Schema-v4 cloud-safe
scan policy, exclusion, immutable-result, and read-only query behavior documented in
[`storage-schema-v4.md`](storage-schema-v4.md) remains unchanged.

## Transactional migration

Opening a version-4 database runs one `BEGIN IMMEDIATE` migration. It creates all review tables,
constraints, and indexes before setting `user_version = 5`, then commits. Failure rolls the
migration back. Version-3 databases migrate through v4 and v5 in order; unknown older and newer
versions remain fail-closed.

Deleting a run cascades its review state. Database truncation removes the command ledger,
decisions, and plans before immutable run rows.

## Durable review tables

- `review_plan` belongs to one immutable run and carries a monotonic revision. A partial unique
  index permits one `active` plan per run; `archived` is reserved for a later plan-lifecycle slice.
- `review_decision` belongs to one plan, duplicate-file group, and scanned-file member. It records
  `keep`, `remove`, or `undecided`, `manual` provenance, the decision time, and the immutable path,
  file identity, size, modified time, and content-hash snapshot used for the decision.
- `review_command` records a caller operation ID and its exact bounded mutation payload and
  applied revision. Replaying an identical operation is a no-op; reusing the ID for another
  payload is rejected.

These tables do not use `scanned_file.marked_deleted` or legacy `deletion_plan` state. They do not
represent validation, execution, or current filesystem state.

## Survivor invariant

A `remove` decision is rejected when it would leave its duplicate set without an independently
accessible physical copy. A non-empty immutable `file_identity` groups hard-link aliases as one
physical item. When identity is unavailable, canonical path is the conservative distinct fallback.
The check and decision update run in the same transaction.

## Bounded protocol reads

`review_plan.get` returns an active plan or a virtual revision-zero plan plus aggregate decision,
planned-byte, and physical-survivor counts. `review_group.page` uses revision-bound forward keyset
cursors and a maximum page size of 500. Existing duplicate-member pages include active-plan
decision/provenance, plan revision, and a selected-group summary while retaining their existing
page bounds.

All review operations read or mutate SQLite snapshots only. They do not read the filesystem,
access excluded cloud placeholders, validate live state, preview content, or expose deletion.
