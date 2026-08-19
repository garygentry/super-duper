# Storage Schema V7

Schema version 7 adds reusable named ordered-preferred-scan-root rule configuration for read-only
preview. It preserves schema-v6 manual file/folder review decisions unchanged. Rust remains the
only owner of product SQLite; Windows clients use the versioned worker protocol.

## Transactional migration

Opening a version-6 database runs one `BEGIN IMMEDIATE` migration. It creates the rule, ordered-root,
and rule-command tables plus their indexes before setting `user_version = 7`, then commits. Failure
rolls back to an unchanged valid v6 database. Supported older databases continue to migrate forward
in order, and unknown newer schemas remain fail-closed.

Rules are reusable configuration rather than run-owned state. Deleting a scan session/run does not
delete them. Explicit database truncation removes the rule command/root/rule rows before scan data.

## Named ordered rules

- `preference_rule` stores a case-insensitively unique name, fixed
  `ordered_preferred_scan_roots` kind, active/reserved archived state, monotonic positive revision,
  and timestamps.
- `preference_rule_root` stores 1--64 exact root strings at dense zero-based ordinals. A rule cannot
  contain the same root twice under the registered locale-independent Unicode case-insensitive
  collation.
- `preference_rule_command` stores the caller operation ID, complete save payload, and applied
  rule/revision. Exact retry returns the original result; operation-ID reuse with another payload
  fails.

Saving a rule replaces its complete ordered-root list and advances only the rule revision. It does
not create or update a `review_plan`, `review_decision`, or `review_folder_decision`, and it does not
use `scanned_file.marked_deleted` or legacy `deletion_plan`.

## Preview reads

Preview matches rule roots to immutable `scanned_file.root_path` with exact locale-independent
case-insensitive equality. It reads the active manual plan at one revision, uses immutable file
identity/canonical-path physical keys, and uses `directory_node.parent_id` for folder containment.
The preview is reconstructed rather than persisted: no preview row is review truth or execution
state.

The three supported scopes are a bounded explicit set list, the complete normalized duplicate-file
filter, and the immutable completed run. Result pages are keyset-bounded; their cursor signature
includes rule and review revisions. Summary fields are fixed-size and globally de-duplicate logical
file IDs and physical keys within the scope. V1 rejects scopes above 100,000 sets or 500,000 logical
paths with `preview_too_complex`; no partial result is persisted or returned.

No schema-v7 operation reads, validates, hydrates, or mutates the live filesystem or excluded cloud
placeholders. Rule application, validation, deletion scheduling, and execution remain absent.
