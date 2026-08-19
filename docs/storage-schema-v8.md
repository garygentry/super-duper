# Storage Schema V8

Schema version 8 adds bounded ordered-preferred-root rule application and reversal provenance. It
preserves schema-v7 reusable rule configuration and schema-v5/v6 manual review state. Rust remains
the only owner of product SQLite; Windows clients use the versioned worker protocol.

## Transactional migration

Opening a version-7 database runs one `BEGIN IMMEDIATE` migration. It adds the manual-decision
revision marker and creates the application, rule-decision, and reversal-command tables plus their
indexes before setting `user_version = 8`. Failure rolls back to an unchanged valid v7 database.
Supported older databases continue to migrate forward in order, and unknown newer schemas remain
fail closed.

Existing manual file rows receive `manual_revision = 0`, which makes them older than every future
rule application while preserving unconditional manual `Keep`/`Remove` precedence. Existing
manual folder rows and reusable rules are unchanged.

## Application provenance

`review_rule_application` belongs to one active review plan and snapshots:

- its apply operation ID, run, and plan;
- rule ID, revision, name, kind, and exact ordered roots JSON;
- canonical scope kind, complete normalized scope JSON/signature, and preview signature;
- source review revision and the one resulting applied revision;
- fixed scoped, applicable, blocked, rule-Keep, rule-Remove, physical-item, and byte totals;
- active/reversed state and created/reversed timestamps.

An application record remains after reversal so history and exact replay survive restart and later
rule edits. Rule configuration is never copied back from or mutated through an application.

`review_rule_decision` belongs to exactly one application and one immutable duplicate-file member.
It stores `keep` or `remove`, a stable explanation/rank, decision time, and the same immutable file
snapshot fields used by manual review. A partial unique index prevents two active applications from
owning the same plan/file. Manual rows remain in `review_decision`; folder review remains in
`review_folder_decision`.

## Effective review overlay

The effective file choice is derived in this order:

1. any manual `keep` or `remove`;
2. a manual `undecided` whose `manual_revision` is later than the rule application's applied
   revision;
3. the active rule-produced `keep` or `remove`;
4. explicit older manual `undecided`, or implicit `undecided` when no row exists.

New manual mutations store their resulting shared plan revision. They never update a rule row or
application record. Reversal deletes only rule-decision rows owned by that application, so manual
choices made at any time remain durable and manual-owned.

## Idempotency and reversal

The application row itself is the apply idempotency ledger. An exact retry returns its original
application ID, applied revision, and fixed outcome; operation-ID reuse with another payload fails.
The complete canonical scope and both source revisions participate in payload equality.

`review_rule_reversal_command` stores one exact reversal payload and applied revision. An exact
retry returns the original outcome. A new reversal for an already reversed application fails.
Application and reversal each advance the shared plan revision once, and each commits its complete
provenance/decision/command changes atomically.

Run deletion cascades plans, application decisions, applications, and reversal commands. Explicit
database truncation removes reversal commands and application decisions before applications,
manual review rows, plans, and immutable scan data. Reusable preference rules retain their existing
separate cleanup order.

## Safety boundary

Application reruns the same bounded immutable preview evaluation and stages only rows from
applicable sets. The transaction rejects stale rule/review revisions, signature drift, overlapping
active applications, and any file/folder overlap or survivor-invariant failure without partial
success. Reversal reruns effective-plan invariants before commit.

No schema-v8 operation reads, validates, hydrates, or mutates the live filesystem or excluded cloud
placeholders. It does not create a deletion schedule and does not use `scanned_file.marked_deleted`
or legacy `deletion_plan` as review truth.
