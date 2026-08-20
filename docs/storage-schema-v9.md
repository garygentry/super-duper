# Storage Schema V9

Schema version 9 adds immutable reviewed-plan preflight snapshots and durable validation
observations. It preserves schema-v8 rule-application provenance, schema-v5/v6 manual review
state, and immutable scan history. Rust remains the only owner of product SQLite; Windows clients
use the worker protocol.

## Transactional migration

Opening a version-8 database runs one `BEGIN IMMEDIATE` migration. It creates `preflight`,
`preflight_item`, and `preflight_item_source` plus their bounded-query indexes, then sets
`user_version = 9`. Failure rolls back to an unchanged valid v8 database. Supported older schemas
continue to migrate in order, and unknown newer schemas remain fail closed.

No existing scan, review, rule, application, or legacy deletion row is rewritten. Session/run
deletion cascades preflight rows through their run and plan foreign keys. Explicit truncation
removes item sources, items, and preflight headers before review plans and immutable scan rows.

## Immutable preflight header

`preflight` records one idempotent validation generation:

- unique `operation_id`, run, active review plan, and exact `review_revision`;
- deterministic `snapshot_signature` over the frozen physical-file and exact-folder targets;
- lifecycle state: `pending`, `running`, `cancelling`, `completed`, `cancelled`, `interrupted`, or
  `failed`;
- fixed logical removal, physical removal, exact-folder removal, affected-group, planned-byte, and
  total-validation-item counts;
- durable processed, ready, changed, missing, unavailable, and conflict counters;
- created/started/completed timestamps and bounded structured failure code/detail.

The header does not authorize or describe an execution. A later review mutation advances
`review_plan.revision`; it does not modify an older preflight. Queries compare the stored revision
with the current active plan and return `isCurrent` separately.

Startup changes only abandoned `running` or `cancelling` headers to `interrupted`. Already
committed observations remain queryable. A retry uses a new operation ID and generation instead of
mixing observations into an interrupted snapshot.

## Physical items and observations

`preflight_item` materializes a file or exact-folder target with a `remove` or `survivor` role. File
rows store the immutable scan file ID/path, stable identity, byte size, nanosecond modified time,
and complete content hash. Folder rows store the immutable directory/member IDs plus structural
and verified fingerprints. A stable file identity is the physical key when available; otherwise a
normalized path is the conservative snapshot key.

Each item begins `pending` and receives exactly one observation in its generation:

- `ready`: exact validation succeeded;
- `changed`: an accessible ordinary file differs in identity, size, modified time, or hash;
- `missing`: the exact path does not exist;
- `unavailable`: metadata, identity, enumeration, or content I/O failed;
- `conflict`: type/link/reparse/placeholder/exclusion, folder-tree, alias, snapshot, or survivor
  safety failed.

Stable `reason_code`, observed metadata, optional OS error number, and observation time explain the
outcome. Preflight never writes these results into scan rows, effective review decisions, rule
provenance, future operation state, `scanned_file.marked_deleted`, or `deletion_plan`.

## Logical sources and hard links

`preflight_item_source` relates a physical validation item to each contributing manual/rule file
decision, exact-folder decision, or required survivor. It retains duplicate-file/folder IDs,
immutable file/directory IDs, and the exact snapshot path. Multiple selected hard-link aliases may
share one physical file item and complete hash read while remaining separate logical paths.

Survivor re-evaluation joins sources back to their affected duplicate groups. At least one ready,
independently accessible physical survivor must remain for every file group, and one complete ready
folder copy must remain for every affected exact-folder group. An unselected hard-link alias can
keep its one physical file accessible, but aliases never inflate the physical survivor count.

## Paging and indexes

`idx_preflight_run_created` supports latest-generation lookup by run.
`idx_preflight_status` supports bounded lifecycle reconciliation. Item indexes support pending work
in immutable ordinal order and detail pages in outcome/role/kind/path/ID order.
`idx_preflight_item_source_item` bounds source lookup for one physical item. Worker pages are capped
at 200 rows and require query-bound opaque cursors; WPF retains at most five 100-row pages.

## No-hydration and no-deletion boundary

Before any target access, preflight checks the immutable run's effective registered/manual
exclusions. Excluded paths are conflicts and are never opened, enumerated, canonicalized, hashed,
or passed to native identity APIs. On Windows, non-opening attributes classify Cloud Files
offline/recall entries and reparse points before metadata or content access. Placeholders are
reported as conflicts and never hydrated.

Schema v9 contains no operation, batch, schedule, Shell result, Recycle Bin state, deletion result,
or Milestone 12 live-state overlay. No code in this slice invokes `IFileOperation`, Shell deletion,
or the `trash` execution path.
