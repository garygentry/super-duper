# Storage schema v15

Schema v15 adds `scan_run.warning_revision`, a non-negative durable revision for one run's bounded
warning snapshot. Existing v14 runs migrate with revision zero. The migration checks for the column
before adding it, so narrowly constructed historical fixtures and an interrupted/repeated open do
not create a duplicate column. Versions newer than 15 remain rejected.

The revision advances when structured aggregates are replaced, the active unclassified fallback
changes, or the warning lifecycle moves through running, cancelling, terminal completion, or
startup interruption. Ordinary progress that leaves warning rows/count/state unchanged does not
invalidate a warning cursor. Warning rows, total aggregate count, exact accounted occurrence count,
persisted run warning count, revision, and run status are read from one SQLite snapshot.

Worker `warning.page` cursors bind the exact run, sort field/direction, warning revision, and run
status. An active mutation or terminal handoff therefore returns `invalid_cursor` rather than
combining adjacent pages from different snapshots. Terminal rows remain immutable, and restart
reconstructs the latest durable interrupted or terminal snapshot without filesystem access.

The additive worker response also reports separately configured bounded diagnostic-log location
metadata. That local worker-stderr log is supplemental developer/recovery detail; it is not stored
in SQLite, is not paged as a warning occurrence, and cannot replace exact durable warning
accounting.

## Case-insensitive parent-directory and directory-path indexes

This change adds no table or column and does not advance `user_version`.
`idx_file_run_parent_unicode_nocase` covers `scanned_file(run_id, parent_dir COLLATE
UNICODE_NOCASE)` and serves every `directory.path COLLATE UNICODE_NOCASE = file.parent_dir` join in
`storage/review.rs` where a directory drives and a file is looked up (including
`review_folder_group_summary_tx`, read on every `duplicate_folder_group.members` page).
`idx_dir_run_path_unicode_nocase` covers the mirror-image shape, `directory_node(run_id, path
COLLATE UNICODE_NOCASE)`, and serves the two join sites where a file drives and a directory is
looked up instead (`review_plan_summary`'s `removed_file_ancestors` CTE, read on every review-plan
load, and `validate_review_state`'s `file_folder_overlap` query, run on every review decision
mutation). Without an index in the matching collation on whichever side is being probed, these
joins fell back to a per-row scan of every candidate row in the run, so opening the Windows app's
folder-copy comparison — and, for the mirror-image sites, loading a review plan or saving a review
decision — scaled with total run size rather than with the size of the folder copies or decision
involved. Opening an existing schema-v15 database creates both additive indexes idempotently under
Rust ownership, matching the existing `idx_file_run_path_unicode_nocase` pattern.

`review_folder_group_summary_tx` also now short-circuits when no review plan is active: with no
plan, `plan_id = ?` never matches (SQL NULL equality), so every decision is undecided and nothing is
ever "removed" — the full recursive descendant-tree walk always resolves to the same trivial shape,
computed directly instead.
