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

## Read-only review indexes

Milestone 8 adds no table or column and does not advance `user_version`. Rust registers the
`UNICODE_NOCASE` SQLite collation before schema reconciliation. It compares locale-independent
Unicode lowercase strings, without filesystem access, separator rewriting, device-prefix
rewriting, dot-segment resolution, or Unicode normalization-form conversion.

`idx_file_run_path_unicode_nocase` covers `(run_id, canonical_path COLLATE UNICODE_NOCASE)`, and
`idx_group_member_file` covers the immutable file-to-group ownership join. Together they serve the
exact canonical-member-path predicate used by duplicate-file group rows, total, summary,
selected-root facets, and drive facets. Opening an existing schema-v4 database creates these
additive indexes idempotently under Rust ownership.

The existing member-path substring predicate is unchanged and is not a prefix index. A future
boundary-aware prefix/descendant or selected-root-relative filter requires separately specified
normalized path keys and indexes before it can be exposed.

Milestone 8 also adds a nullable internal `scanned_file.extension_key` column without advancing
`user_version`. Rust populates it from the persisted final `file_name` segment when a file snapshot
is inserted. The key is the suffix after the last dot, excluding the dot. A name without a dot, a
terminal dot, or a dotfile whose only dot is its leading dot stores the empty no-extension key;
`.env.local` stores `local`, and `archive.tar.gz` stores `gz`.

Extension keys use locale-independent Unicode lowercase while preserving the original Unicode
normalization form. They do not trim characters, infer MIME or file type, inspect the
representative label, canonicalize a path, or read the filesystem. The optional group filter uses
`None` for no extension predicate and the empty stored key for an explicit no-extension predicate.
Its match mode is `any` by default: one matching immutable member is sufficient even when other
exact-content members use different extensions. The optional `all` mode requires the number of
matching immutable members to equal the group's persisted copy count. With the empty key, `all`
therefore means every member has no extension. When the extension is absent, the mode is ignored
and normalized to `any`. Maintained file-type classification remains a separate, unexposed
contract.

`idx_file_run_extension_key` covers `(run_id, extension_key, id)` and serves the any-member lookup
through `idx_group_member_file`. All-member matching uses the same stored key plus the indexed
group membership lookup and compares its matching-member count with `duplicate_group.file_count`;
it does not infer from the representative. The shared normalized predicate serves duplicate-file
group rows, total, summary, selected-root facet counts, and drive facet counts. Opening an older
schema-v4 database adds the column if needed,
backfills null keys in bounded 500-row SQLite batches, and creates the index transactionally. Once
the column, index, and keys are present, ordinary worker connections take only the read-only
reconciliation checks. The backfill uses persisted filenames only and performs no filesystem I/O.

## Migration

Opening schema v3 performs one `BEGIN IMMEDIATE` migration that adds the session policy columns,
the run counter, and `run_exclusion`, then sets `user_version` to 4. Existing sessions receive the
safe default policy with empty exclusions and `unavailable` detection; they must be refreshed by a
Windows client before another run can start. Existing runs and result rows are unchanged.

Migration failure rolls back the transaction. Version 2 still uses the existing history-preserving
migration path and lands on the current v4 schema. Versions 0 and 1 with user tables remain
unsupported, and versions newer than 4 are rejected without modification.
