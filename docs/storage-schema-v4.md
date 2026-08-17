# Storage Schema V4

Schema version 4 extends the immutable-run Windows persistence model with cloud-safe scan policy
state. Rust remains the only owner of SQLite; Windows clients use the worker protocol exclusively.
The v3 run/result ownership and keyset-paging rules documented in
[`storage-schema-v3.md`](storage-schema-v3.md) remain unchanged.

## Session policy and immutable run snapshots

`scan_session` adds:

- `cloud_policy`, defaulting to `exclude_registered_roots`;
- `manual_location_exclusions_json` for absolute paths belonging to providers that do not register
  with Windows Cloud Files;
- `registered_cloud_locations_json`, containing the registration path plus bounded provider
  identity/display metadata supplied by Windows Infrastructure; and
- `cloud_detection_status`, which is `complete`, `unsupported`, or `unavailable`.

The worker accepts only the fail-closed `exclude_registered_roots` policy for execution in the
first Milestone 7 slice. Detection must be `complete` before `run.start`. A run's
`parameters_json` snapshots all policy fields, so refreshing or editing a session never changes a
historical run.

## Structured exclusions

`run_exclusion` owns one aggregate record per subtree pruned during one run. It stores the run,
path, stable reason code, optional provider identity/display name, and occurrence count. The unique
`(run_id, path, reason_code)` key prevents per-descendant event growth. `scan_run` stores
`excluded_subtree_count` for summaries.

Exclusion rows are separate from `scanned_file`: excluded paths never become file snapshots and do
not participate in hashing, duplicate analysis, directory analysis, preview, validation, or
deletion-plan state. `scanned_file.marked_deleted` and `deletion_plan` are unchanged and remain
outside the post-MVP review workflow.

`run_exclusion.page` uses a bounded offset page (maximum 500) ordered by path and ID. This dataset
is small by construction and remains server-owned; later Activity work may generalize the event
surface without rewriting the immutable exclusion records.

## Migration

Opening schema v3 performs one `BEGIN IMMEDIATE` migration that adds the session policy columns,
the run counter, and `run_exclusion`, then sets `user_version` to 4. Existing sessions receive the
safe default policy with empty exclusions and `unavailable` detection; they must be refreshed by a
Windows client before another run can start. Existing runs and result rows are unchanged.

Migration failure rolls back the transaction. Version 2 still uses the existing history-preserving
migration path and lands on the current v4 schema. Versions 0 and 1 with user tables remain
unsupported, and versions newer than 4 are rejected without modification.
