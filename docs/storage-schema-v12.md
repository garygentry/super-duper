# Storage Schema v12

Schema version 12 adds a bounded external-validation overlay for `WPM12-external-invalidation`.
It never rewrites immutable scan rows or recorded manual/rule review history and grants no watcher,
provider-hydration, Shell, Recycle Bin, deletion, or filesystem-mutation authority.

## Migration, backup, and downgrade

Opening a version-11 database runs one `BEGIN IMMEDIATE` migration. It creates
`review_live_validation`, `review_live_validation_item`, and `review_live_file_state`, creates their
ownership/query indexes, separates the recorded-decision projection from the working effective
projection, sets `user_version` to 12, and commits. Any failure rolls the complete migration back
to version 11. Unknown newer schemas remain rejected without modification.

Before first opening an existing database with this build, close the app and worker and back up the
main database, `-wal`, and `-shm` files as one set. There is no in-place downgrade. To return to a
v11 build, restore the complete pre-migration backup; do not lower `user_version` or remove v12
objects manually.

## Bounded validation ledger and latest overlay

`review_live_validation` is an idempotent command ledger bound to a completed run, one duplicate
group, the exact active review revision, and scope `selection` or `visible_page`. A request contains
1–200 distinct positive file IDs. `review_live_validation_item` records exactly one observation for
each explicit ID in request order. Neither table owns a cursor, folder traversal, background queue,
or full-result query.

`review_live_file_state` stores only the latest observation per run/file. A `missing` or `changed`
observation makes an existing recorded `Keep` or `Remove` choice ineffective in the working
projection while retaining the recorded decision and its immutable snapshot. The overlay keeps the
prior decision kind for actionable display. A later `present` observation keeps that invalidation
sticky until the operator records a fresh decision or explicitly chooses `Undecided`.

`recorded_review_decision` remains the immutable-history-compatible union of manual and active-rule
choices. `effective_review_decision` derives working choices by excluding rows whose latest overlay
is invalidated. Existing plan, folder-overlap, paging, preferred-root, and preflight queries continue
to use the effective projection and retain their accepted survivor and revision rules.

## Exclusion and access contract

Validation first resolves each explicit ID through persisted run/group/member ownership. It then
classifies the canonical path against the immutable run exclusion snapshot. An excluded location is
reported `unavailable/excluded_location` without calling the filesystem validator, opening content,
following a placeholder, or attempting hydration. Non-excluded validation uses metadata and stable
identity only; it never opens file content and never mutates the path.

Exact operation replay returns the stored observations. Reusing an operation ID with another
payload, a stale review revision, an incomplete run, a cross-group ID, a duplicate ID, or more than
200 IDs fails without overlay writes. The revision and ownership checks are repeated inside the
commit transaction so a late response cannot attach to a newer review context. Overlay and command
rows survive restart; immutable `scanned_file`, duplicate-group, and recorded-decision rows remain
unchanged.

Production execution remains disabled: `RecycleOperationViewModel.CanSubmit` is false, composition
uses `DisabledRecycleOperationCapabilityExecutor`, every worker operation response reports
`executorEnabled:false`, and no **Move to Recycle Bin now** action exists.

Schema v13 builds on this accepted overlay with a separate dirty-root/overflow and bounded
reconciliation contract; see [`storage-schema-v13.md`](storage-schema-v13.md). The v12 validation
ledger, sticky invalidation rules, effective-decision projection, and immutable-history boundary
remain unchanged.
