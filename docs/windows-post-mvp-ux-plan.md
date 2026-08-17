# Windows Post-MVP Duplicate Review UX Plan

## Status

Active implementation roadmap for the Windows duplicate-review experience. Milestone 6
release-acceptance remediation and the fail-closed Milestone 7 slice are complete. The first two
read-only Milestone 8 slices are implemented and accepted; the broader milestone remains in
progress and is gated by the remaining criteria below.

This is the durable planning source for post-MVP Windows UX work. Update this document when a
milestone is refined, split, accepted, or superseded so future coding sessions do not have to
reconstruct product and architecture decisions from conversation history.

## Prerequisite

The release blockers in
[`windows-release-acceptance-remediation-plan.md`](windows-release-acceptance-remediation-plan.md)
remain the entry gate. Post-MVP implementation must not enter the release-remediation scope or be
used to redefine its acceptance criteria.

## Product Goal

Turn collected duplicate results into a trustworthy decision workflow that helps a user:

1. Understand where duplicate content exists and how its locations relate.
2. Decide explicitly which copies to keep and which to remove.
3. Apply reusable location preferences without implying that the app can identify an "original."
4. Validate current filesystem state before any destructive action.
5. Send reviewed removals to the Windows Recycle Bin and reconcile partial outcomes.
6. Understand warnings, skipped content, external changes, and file-operation results.

The intended journey is:

```text
immutable scan snapshot
        -> duplicate review
        -> durable decisions
        -> live preflight
        -> Recycle Bin operation
        -> reconciled working results
        -> durable activity history
```

## Product Principles

### Historical truth and current truth are separate

A completed run remains an immutable statement of what the scanner observed. Missing, changed,
moved, externally deleted, or recycled files are represented by a mutable live-state overlay. The
app must never rewrite historical results to make the current filesystem appear consistent.

### Review is not deletion

Marking a copy as `Keep` or `Remove` is a reversible planning action. Execution is a separate,
validated operation with its own status, progress, cancellation, and result history.

### The user defines preference

All members of an exact duplicate-file set have identical scanned content. Metadata and location
can inform a user preference, but cannot prove which copy is the original or objectively best.
Automated recommendations must state the rule that selected the survivor.

### At least one verified survivor

No plan may remove every independently accessible physical copy from a duplicate set. Hard-link
aliases do not count as separate survivors.

### Cloud access must be explicit

Scanning, previewing, thumbnail extraction, validation, and deletion must not unexpectedly hydrate
cloud placeholders. The default behavior excludes registered cloud sync roots.

### Large-result behavior remains server-owned

Sorting, filters, facets, review summaries, event queries, and deletion-plan queries remain owned by
the Rust worker. WPF collections and caches remain bounded independently of result count.

## Current Foundations and Gaps

The implemented MVP already provides:

- immutable session runs and run-scoped results;
- cursor-paged duplicate-file and exact-folder queries;
- bounded WPF page caches and stale-request rejection;
- exact-folder nested-result suppression;
- copy-path and `SHOpenFolderAndSelectItems` Explorer reveal;
- scan progress, cancellation, recovery, and durable run history.

The post-MVP design must replace or extend these current limitations:

- results explain matches but do not retain user decisions;
- `scanned_file.marked_deleted` mixes mutable action state into an immutable snapshot;
- `deletion_plan` is file-centric and does not model review plans, folder actions, validation,
  partial execution, or live invalidation;
- warning counts mostly lead to local diagnostics instead of structured, queryable events;
- `warning.page` was planned but is not implemented by the worker;
- generic reparse-point avoidance is not a complete Cloud Files policy or no-hydration contract;
- external filesystem changes do not update a working view of historical results.

## Information Architecture

Keep the session-oriented shell, but evolve the result area into a `Review` workspace:

```text
Setup | Progress | Run history | Review | Activity

Review
  Summary
  Duplicate files
  Exact duplicate folders
  Review plan
  Resolved
```

The file and folder experiences remain distinct modes. They share review state, filters, location
language, plan summaries, live-state badges, and navigation.

## Wave 1 - Understand and Decide

### Milestone 7 - Cloud-Safe Scan Policies

Status: The fail-closed `exclude_registered_roots` vertical slice is accepted after Debug/Release
available-provider coverage and a Release provider-unavailable operator pass against a registered
consumer OneDrive root. The accepted surface does not include the two opt-in policies:
`include_sync_roots_skip_placeholders` and `allow_cloud_access` remain unavailable until their
separate placeholder-state, hydration-confirmation, and real-provider gates are complete.

#### Refined implementation plan (2026-08-17)

The first vertical slice implements the fail-closed default policy end to end. The two opt-in
policies remain designed protocol values but are not exposed until Windows placeholder-state tests
and explicit download confirmation exist; silently treating a partial implementation as opt-in
cloud access would weaken the no-hydration contract.

##### User story and non-goals

A user can create or edit a session, see registered Cloud Files roots that intersect the selected
scan roots, add exclusions for non-Cloud-Files providers, and start a run knowing those locations
will be pruned before traversal opens them. This slice does not add content preview, thumbnails,
validation, review decisions, deletion, or any use of `scanned_file.marked_deleted`.

##### UX states

- Session setup defaults to `Exclude registered cloud sync roots` and shows readable provider/path
  chips for detected intersections.
- A selected root inside a registered sync root is called out as fully excluded; a broad ancestor
  root shows the registered subtree that will be skipped.
- Detection has explicit `checking`, `ready`, `unsupported`, and `unavailable` states. Saving is
  allowed when detection is unavailable, but starting the default-policy run is blocked until a
  fresh successful detection completes. Manual exclusions remain editable in that state.
- Refresh and save/start detection run asynchronously, are cancellable with the owning view-model
  operation, and never run on the WPF dispatcher.
- Keyboard and automation names cover the policy description, refresh action, detected locations,
  manual exclusions, and fail-closed error.

##### Policy semantics

- `exclude_registered_roots` is the default and the only enabled policy in this slice. Its
  effective exclusions are the union of registered sync roots and normalized manual exclusions.
- `include_sync_roots_skip_placeholders` will allow ordinary local files below a sync root while
  pruning every Cloud Files placeholder from directory-enumeration attribute/tag data. It remains
  unavailable until `CfGetPlaceholderStateFromAttributeTag` integration is covered without
  opening placeholder content.
- `allow_cloud_access` will permit operations that may hydrate only after a separate explicit
  confirmation scoped to the operation. It remains unavailable in this slice.
- Manual exclusions are absolute location prefixes, normalized and de-duplicated independently of
  glob ignores. A parent exclusion subsumes its children.
- If registered-root discovery is unsupported or unavailable, the default policy fails closed at
  run start. The scanner's existing reparse avoidance remains defense in depth, not a substitute
  for registered-root detection.

##### Storage and immutable runs

- Schema v4 adds `cloud_policy`, `manual_location_exclusions_json`,
  `registered_cloud_locations_json`, and `cloud_detection_status` to `scan_session` with safe
  defaults for migrated sessions.
- `scan_run.parameters_json` snapshots all four values. Editing or refreshing a session never
  changes an existing run's effective exclusions.
- A run-owned structured exclusion table records one aggregate row per pruned subtree, including a
  stable reason code and optional provider identity/display name. The run summary stores the count;
  no row is emitted per descendant file.
- The v3-to-v4 migration is transactional, rolls back on failure, retains all historical rows, and
  continues to reject a newer unknown schema.

##### Protocol additions

- `session.list`, `session.get`, `session.create`, and `session.update` carry `cloudPolicy`,
  `manualLocationExclusions`, `registeredCloudLocations`, and `cloudDetectionStatus`.
- Run DTO `parameters` carries the immutable snapshot and the run DTO carries
  `excludedSubtreeCount`.
- `run_exclusion.page` is a bounded, run-scoped query with `offset`/`limit` (maximum 500), stable
  path ordering, and no filesystem access. It is the initial Activity-data hook; a later Activity
  milestone may generalize it without rewriting these records.
- Unknown fields remain rejected by the worker's allow-listed request DTOs. Invalid policies,
  relative exclusions, oversized provider metadata, and incomplete default-policy detection return
  `invalid_session`; unavailable detection at start returns structured `invalid_session` details.

##### Windows detection and no-hydration boundary

- `SuperDuper.Windows.Infrastructure` enumerates registrations with
  `StorageProviderSyncRootManager.GetCurrentSyncRoots`; Core owns only an interface and DTOs.
- Detection uses registered paths and provider registration identity, never provider-name substring
  matching.
- The Rust scanner receives platform-neutral effective location exclusions. It compares a candidate
  path before `is_dir`, `file_type`, metadata, canonicalization, stable-identity lookup, hashing, or
  persistence validation. A root selected inside an excluded location is rejected at the same
  pre-I/O boundary.
- Excluded entries never reach the size buckets, hash cache, duplicate analysis, folder analysis,
  preview/thumbnail contracts, or later validation contracts. Future consumers must use the same
  policy snapshot rather than independently reopening paths.
- Positive prefix classifications are cached by pruning the complete subtree once; cancellation is
  checked before every root and directory batch.

##### Acceptance and recovery tests

- Rust traversal tests cover a broad ancestor with a registered subtree, a root explicitly inside
  that subtree, an ordinary non-excluded sibling, manual exclusions, aggregation, and cancellation.
- Storage tests cover v3 migration, safe defaults, immutable run snapshots, rollback/newer-schema
  behavior, and bounded run-exclusion queries.
- Worker protocol tests cover defaults, round trips, invalid policy/location metadata, fail-closed
  start, run snapshots, and exclusion paging.
- Core tests cover setup states, dirty/save behavior, selected-root messaging, and detection failure;
  Infrastructure tests cover registration mapping and path intersection without content access.
- Windows integration/operator acceptance uses an isolated OneDrive fixture containing an online
  file, an offline placeholder, a broad ancestor root, an explicit cloud root, and a temporarily
  unavailable provider. File hydration/pin state and network transfer are checked before and after.
- WPF smoke verifies setup remains responsive, detected chips are accessible, and an unavailable
  detector cannot start a default-policy scan. Real provider/placeholder assertions remain an
  interactive Windows acceptance step where the test environment cannot safely provision a sync
  provider.

#### Acceptance hardening result (2026-08-17)

- Fixed a stale command-notification finding where registration detection could become ready or
  unavailable without invalidating the shell's `Start scan` command. Core regression coverage now
  requires `CanStart` notification on both detection and checking-state transitions.
- Registration discovery now has a testable Infrastructure source boundary. Deterministic tests
  cover unsupported discovery, enumeration failure returning `unavailable`, cancellation, blank
  paths, case-insensitive de-duplication, stable ordering, and provider-name fallback without
  opening registered content.
- `Invoke-WindowsCloudPolicyAcceptance.ps1` is the repeatable real-provider gate. It verifies WinRT
  registration discovery, a locally available file, an offline/recall-on-access placeholder, a
  broad ancestor root whose unrelated children are manually pruned, an explicit cloud root,
  immutable exclusion records, and before/after file state. It also requires the selected provider
  processes' read/write/other transfer counters to remain unchanged. `-ExpectProviderUnavailable`
  is the second operator mode; the script never stops or pauses a provider itself.
- The 2026-08-17 Release run against a registered consumer OneDrive root passed. The locally
  available fixture remained `0x00000420` with 1,147 logical/allocated bytes. The offline fixture
  remained `0x00401620` with 1,516 logical bytes and zero allocated bytes. The broad-ancestor run
  discovered zero files and recorded 109 bounded exclusions (one registered cloud subtree plus
  manually pruned siblings); the explicit-root run discovered zero files and recorded one cloud
  exclusion. Placeholder flags, allocation, timestamps, and OneDrive process transfer counters
  were unchanged.
- Debug and Release real WPF smoke passed Cloud locations refresh/responsiveness and a deterministic
  registration-unavailable launch where `Start scan` remained disabled before and after refresh.
- The operator then quit OneDrive through its supported UI and completed the Release
  `-ExpectProviderUnavailable` pass against the still-registered consumer root. The same local and
  offline fixtures retained `0x00000420`/1,147 allocated bytes and `0x00401620`/zero allocated bytes,
  respectively; provider transfer counters were unchanged; the broad-ancestor run recorded 109
  exclusions and zero files; and the explicit-root run recorded one exclusion and zero files. This
  closes the final acceptance gate for the fail-closed policy without automating provider shutdown.
- A separate commercial-root attempt ended during metadata-only fixture selection because that root
  contained no offline/recall-on-access placeholder within the bounded search. It did not start a
  worker scan and is an environment fixture limitation, not a product failure; the consumer-root
  pass supplies the required real unavailable-provider evidence.

#### User outcome

Scanning a broad local root does not unexpectedly download OneDrive or another registered Cloud
Files library. The user can see what was excluded and can deliberately opt into cloud content.

#### UX

- Add a `Cloud locations` section to session setup.
- Default to `Exclude registered cloud sync roots`.
- Detect when an explicitly selected root is inside a cloud sync root and show the provider and
  effective behavior before saving or starting a run.
- Show detected excluded locations as readable chips rather than requiring raw glob patterns.
- Provide advanced policies:
  - exclude registered cloud sync roots;
  - include a sync root but skip Cloud Files placeholders;
  - allow cloud access and possible downloads after explicit confirmation.
- Retain manual excluded paths for providers that do not use the Windows Cloud Files platform.
- Summarize one skipped subtree as one structured event instead of producing an event per file.

#### Engine and protocol

- Add `cloudPolicy` and manual location exclusions to session definitions and immutable run
  parameters.
- On Windows, classify sync roots and placeholder state before canonicalizing or opening content.
- Use registered sync-root detection and Cloud Files placeholder/attribute information rather than
  matching provider names in path strings.
- Cache positive sync-root classifications during traversal.
- Ensure progress and accounting distinguish excluded subtrees from access failures.

#### Acceptance criteria

- Excluding a OneDrive subtree causes no content reads, hashes, thumbnail extraction, or hydration.
- An explicitly selected cloud root is detected before scan start.
- Completed-run summary and Activity data explain every excluded cloud subtree.
- Windows integration tests cover placeholders, ordinary local files inside a sync root, explicit
  cloud roots, broad ancestor roots, and an unavailable provider.
- Non-Windows core behavior remains UI-agnostic and platform-neutral.

### Milestone 8 - Duplicate Review Workspace

Status: The first two read-only file-review slices are accepted. They add a worker-owned summary
for the current duplicate-file query, immutable member location context, and bounded per-set
selected-root/drive span without introducing review decisions, live filesystem state, facets, or
deletion behavior. Full Milestone 8 remains in progress.

#### Refined first vertical slice (2026-08-17)

##### User story and smallest coherent scope

A user selecting a completed run can understand the scale of the current duplicate-file result,
identify the largest recovery opportunity represented by that filter, and see which selected root
and drive own each visible copy. The surface explicitly says that exact content was verified at
scan time and that the representative name is a label, not an original.

The slice extends only the duplicate-file workspace. Exact-folder relationship cards remain
Milestone 9 work. It reuses the existing group filter, stable sort, keyset cursor, member-on-select
loading, five-page LRU caches, cancellation sources, and group/member query generations.

##### Non-goals and safety boundary

- No `Keep`, `Remove`, or `Undecided` state, rules, review plans, or schema for durable decisions.
- No live-state validation, changed-since-scan state, preview, thumbnail, content read, or Cloud
  Files access.
- No facets, saved filters, bulk selection, folder/file overlap analysis, or complete-result export.
- No deletion, Recycle Bin integration, or extension of `scanned_file.marked_deleted` or
  `deletion_plan`.
- Historical result rows remain immutable. The summary and location context are read only and use
  only persisted run-owned SQLite data.

##### Protocol and storage impact

- Schema v4 is unchanged. Existing `scanned_file.root_path`, `relative_path`, and `drive_letter`
  columns supply location context; Rust remains the only SQLite owner.
- `duplicate_file_group.page` adds a `summary` object for the normalized current filter with
  matching set count, matching copy count, potential recoverable bytes, and largest recoverable
  set bytes. The worker computes it with the same run/filter predicate as `total`; WPF never
  derives it by walking result pages.
- Duplicate-file member DTOs add selected root, relative path, and drive label from the immutable
  scanned snapshot. Members remain unloaded until a group is selected and remain cursor-paged at
  1-500 rows.
- Existing sorts, filters, cursor signatures, stable ID tie breakers, frame limits, performance
  diagnostics, and structured errors are unchanged. No new mutation or idempotency contract is
  required.
- Summary results travel with the bounded group page, so the existing group query generation rejects
  a late summary and rows together. Cached pages remain capped at five; the UI retains at most one
  visible page of groups and one visible page of members.

##### Performance and cancellation budgets

- A warm first group page, including its filtered summary, targets 100 ms on representative
  Windows 11 hardware and must keep the existing 100,000-group regression comfortably bounded.
- Group pages remain 200 rows in WPF and at most 500 in the protocol. Member rows are queried only
  after selection, also 200 in WPF and at most 500 in the protocol.
- Changing run, filter, or sort cancels the prior group request and advances its generation;
  changing the selected group cancels the prior member request and advances its generation.
- Summary computation is SQLite-only and has no filesystem cancellation boundary. Client request
  cancellation still prevents a superseded response from reaching the visible state.
- No result or summary collection grows with the total group/member count, and no per-result WPF
  dispatcher update is introduced.

##### Accessibility and interaction behavior

- Summary values have concise visible labels and automation names that include their meaning; they
  are read in stable set/copy/bytes/opportunity order and remain usable in high contrast.
- The selected-set header is persistent while its paged members load and exposes the complete
  representative name through normal text behavior.
- Member-grid columns identify selected root, relative path, drive, size, and modified time. The
  complete canonical path remains available as the relative-path cell's automation name and
  tooltip, and through the copy-path and Explorer reveal commands.
- Existing keyboard grid navigation, sort invocation, filter controls, paging commands, focus
  behavior, automation IDs, row virtualization, and actionable error surfaces remain intact.

##### Acceptance and regression tests

- Storage tests prove unfiltered and filtered summaries share the exact group predicate, use
  run-owned rows only, and expose persisted selected-root/relative/drive context on bounded member
  pages.
- Worker protocol tests prove the summary and member fields serialize as decimal-safe V1 additions,
  remain run scoped, and retain cursor/filter validation.
- Core tests prove formatted summary/location values, late filtered-page rejection, and the
  five-page cache bound. A late old page must not replace either the newer rows or newer summary.
- WPF smoke verifies the summary labels and explanatory copy are accessible, filtering updates the
  visible summary, selection loads location context, and the existing result paging/sorting/reveal
  workflow remains responsive.
- Focused optimized Rust and Core tests run first. Proportional completion requires Debug/Release
  Rust and .NET tests plus real Debug/Release WPF smoke on an interactive Windows 11 x64 desktop.
- The environment-gated real-provider acceptance is not repeated because this slice performs no
  filesystem or Cloud Files access; the accepted Milestone 7 pre-I/O boundary remains unchanged.

##### First-slice acceptance result (2026-08-17)

- Focused Debug storage and worker protocol tests passed. The five Core duplicate-file view-model
  tests and three STA WPF surface tests passed under .NET SDK 10.0.400 from a temporary working
  directory because the pinned 10.0.303 SDK is unavailable; `global.json` was not changed.
- The optimized 100,000-group storage regression passed in 0.85 seconds for its first and next
  keyset pages together, below its existing five-second bounded regression gate. Dedicated
  representative-hardware profiling against the 100 ms warm-page target remains a full-milestone
  performance gate.
- `cargo test --workspace` and `cargo test --workspace --release` passed, including 15 storage and
  9 worker tests in each configuration. Debug and Release worker builds also passed.
- Debug and Release solution builds passed with zero warnings and zero errors. In each
  configuration, 37 Core, 22 Infrastructure, and 3 WPF smoke tests passed. The one
  environment-gated real-provider Infrastructure test was intentionally skipped by the normal
  suite, consistent with the accepted Milestone 7 workflow.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed the new filtered-summary,
  selected-set explanation, and member-location protocol checks, plus existing result sorting,
  paging, filtering, Explorer reveal, Cloud locations refresh/fail-closed behavior, and shutdown
  coverage. PowerShell parsing, targeted Rust formatting checks, and `git diff --check` passed.
- The accepted slice performs only SQLite reads of immutable rows. It adds no schema migration,
  filesystem access, cloud access, durable decision state, preview, validation, deletion, or use of
  `scanned_file.marked_deleted`.
- Remaining Milestone 8 gates include paged facets and richer server-side filters, aggregate
  location summaries and an across-drives/root/drive entry point, next/previous-set keyboard focus
  restoration, the complete accessibility review, and representative-hardware warm-query/memory
  profiling. Durable decisions remain Milestone 10 and deletion remains Milestone 11.

#### Bounded per-set location-span slice (2026-08-17)

##### Audit and scope decision

The existing duplicate-file group path already owns stable sorting, normalized filters, opaque
keyset cursors, five-page LRU caching, cancellation, and group-query generations. Member detail has
a separate bounded cache and generation and loads only after selection. The smallest coherent next
slice therefore rides immutable location counts on each existing group row instead of adding a
facet command, independent cache, or new stale-response path.

##### Implemented contract and UX

- Each bounded `duplicate_file_group.page` row now includes case-insensitive counts of distinct
  non-empty selected roots and drive labels across that set's immutable members. A drive count
  greater than one identifies a cross-drive set; zero is retained for legacy or non-drive paths.
- Schema v4 is unchanged. The query joins only run-owned `duplicate_group_member` and
  `scanned_file` rows and performs no filesystem, Cloud Files, preview, validation, decision, or
  deletion work.
- The fields use the existing group page, cursor, cache, cancellation token, and query generation.
  They do not introduce sorting or filtering in this slice and do not change cursor signatures.
- WPF shows one non-sortable `Location span` value per visible group and repeats the selected set's
  span in its persistent header while member pages load. The text explicitly distinguishes
  selected roots from drives and calls out sets spanning multiple drives.

##### Acceptance result

- Focused Debug storage and worker protocol regressions passed. All five Core duplicate-file tests
  and the targeted STA WPF surface test passed under .NET SDK 10.0.400 from `C:\Windows\Temp` with
  absolute project paths; the unavailable pinned 10.0.303 SDK and `global.json` were unchanged.
- The optimized 100,000-group first/next keyset-page regression passed in 0.46 seconds, below its
  existing five-second bounded gate. Representative-hardware profiling against the 100 ms warm
  target remains a full-milestone gate.
- `cargo test --workspace` and `cargo test --workspace --release` passed, including 15 storage and
  9 worker tests in each configuration. Debug and Release worker builds passed.
- Debug and Release solution builds passed with zero warnings and zero errors. In each
  configuration, 37 Core, 22 Infrastructure, and 3 WPF tests passed; the one real-provider
  Infrastructure test remained intentionally environment-gated.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed worker location counts,
  accessible group/header location text, existing result paging/sorting/filtering and Explorer
  reveal, Cloud setup/fail-closed behavior, and deterministic shutdown coverage.
- Targeted Rust formatting, PowerShell parsing, and `git diff --check` passed.
- Remaining gates are aggregate location summaries, a bounded across-drives/root/drive filter or
  paged facet entry point, next/previous-set keyboard focus restoration, complete accessibility
  review, and representative-hardware warm-query/memory profiling. Durable decisions remain
  Milestone 10 and deletion remains Milestone 11.

#### User outcome

The results surface answers three questions without requiring repeated Explorer investigation:

1. What is duplicated?
2. Where are the copies?
3. Which sets are worth reviewing first?

#### Summary

- Potential recoverable space.
- Sets awaiting review, partially reviewed, ready, changed, and resolved.
- Largest duplicate locations and cross-drive duplication.
- Entry points such as `Over 1 GB`, `Three or more copies`, `Across drives`, and `Changed since
  scan`.

#### Duplicate-set list

- Representative name or folder, with wording that representative does not mean original.
- One-copy size, copy count, recoverable size, distinct locations, drives, and selected roots.
- Review-state and live-state badges.
- Server-side filters for name, extension, path, root, drive, size, copy count, review state, and
  live state.
- Stable server-side sorts and paged facet counts.

#### Selected-set detail

- Persistent set header explaining that exact content was verified at scan time.
- `Keep`, `Remove`, or `Undecided` decision per member.
- Multiple `Keep` decisions are valid.
- Location/root and shortened breadcrumb presentation, with the complete path always accessible.
- Size, modified time, root, drive, cloud/live state, and decision columns.
- Commands for next/previous set, set decision, clear decision, copy path, and reveal.
- Focus restoration and keyboard shortcuts for continuous review.

#### Engine and protocol

- Extend group/member DTOs with selected-root, drive/location, review summary, and live summary.
- Add summary/facet queries without materializing member rows.
- Preserve the existing bounded cursor-cache and query-generation rules.
- Do not load members until a set is selected.

#### Acceptance criteria

- A user can review thousands of sets without losing selection, focus, decisions, or filter state.
- A 100,000-group fixture stays responsive and memory remains bounded by page/cache settings.
- No UI operation binds the complete result or facet dataset.
- Late result, facet, or review responses cannot replace a newer query generation.
- Accessibility names and keyboard actions cover all review decisions and navigation.

### Milestone 9 - Folder Intelligence and Windows Exploration

#### User outcome

Exact duplicate folders are presented as redundant locations or trees, not merely as rows with
long paths.

#### Folder UX

- Present duplicate roots as side-by-side location cards.
- Highlight differing path segments.
- Show size per copy, files per copy, copy count, and recoverable size.
- Add a location matrix grouped by selected scan root, drive, and useful top-level path segment.
- Offer `Show suppressed nested matches` as an advanced toggle.
- Explain that removing a duplicate root affects the complete verified tree.
- Detect and explain overlaps between folder-level and file-level review decisions.

#### Explorer integration

- Double-click to reveal.
- Open an item normally, open its parent, and open all duplicate locations.
- Select multiple duplicates together when they share an Explorer parent.
- Copy one path or all paths.
- Open Windows Properties.
- Perform Shell calls away from the WPF dispatcher.
- Do not add an Explorer shell extension in this milestone.

#### Preview policy

- Do not require thumbnails to complete this milestone.
- If cache-only thumbnails are added, use bounded asynchronous caching and never extract content
  from an excluded cloud placeholder.
- Opening or previewing cloud content is a separate explicit user action, not a side effect of row
  selection.

#### Acceptance criteria

- The relationship between folder copies is understandable without repeatedly reading full path
  strings.
- Explorer commands provide actionable failures and never freeze the UI.
- Explorer selection groups items by parent directory.
- Folder and file review decisions cannot silently schedule the same physical item twice.

### Milestone 10 - Durable Review Plans and Preference Rules

#### User outcome

Review decisions survive restart, can be applied consistently through understandable rules, and
remain reversible until execution.

#### Decision model

- A review plan belongs to one immutable completed run.
- A member decision is `keep`, `remove`, or `undecided`.
- Store whether the decision was manual or produced by a named rule.
- Store the file snapshot used when the decision was made.
- Track plan state separately from execution state.
- Permit multiple named or historical plans later, but support one active plan per run initially.

#### Preference rules

- Ordered preferred scan roots.
- Preferred or protected path prefixes.
- Preferred drives.
- Keep newest or oldest.
- Keep shortest path.
- Keep a configured number of copies.
- Never remove from a protected location.

Every automatic decision displays its reason, for example `Kept because D:\Photos ranks above
E:\Backup`. The UI must not label a metadata-selected member as the original.

#### Bulk-review safety

- Preview a rule before applying it.
- State the scope explicitly: selected sets, current filtered query, or complete run.
- Show affected sets, files, and bytes.
- Reject a plan that removes every independently accessible physical copy.
- Detect folder/file overlaps and hard-link aliases.
- Make clearing or reversing decisions immediate and non-destructive.

#### Storage direction

Replace `scanned_file.marked_deleted` as the source of truth with concepts equivalent to:

```text
review_plan
review_decision
review_rule
live_item_state
file_operation
file_operation_item
```

Exact names and normalization belong in the schema design, but historical snapshot, review state,
live state, and execution state must remain distinct.

#### Acceptance criteria

- Decisions survive application and worker restart.
- Applying a rule always produces a preview first.
- The same plan summary is obtained regardless of result-page navigation order.
- Conflicting and unsafe decisions are blocked with actionable explanations.
- Review-plan queries remain bounded for large plans.

## Wave 2 - Delete Safely and Stay Synchronized

### Milestone 11 - Preflight and Recycle Bin Execution

#### User outcome

The user can execute a reviewed plan with strong assurance that the files still match the scan and
that Windows will place eligible items in the Recycle Bin.

#### Preflight

- Confirm that every removal target still exists and has the expected type.
- Recheck stable identity, size, high-resolution modified time, and content hash.
- Confirm an independently accessible survivor that is not also scheduled for removal.
- Treat hard-link aliases as one physical file.
- Reject unexpected reparse points, links, placeholders, and inaccessible paths.
- For folder removal, re-enumerate the tree and require the verified fingerprint to match.
- Block the entire folder action if any file was added, removed, renamed, changed, or became
  unavailable.
- Surface invalidated plan entries instead of silently dropping them.

#### Execution architecture

- Recycle Bin only in the initial release; permanent deletion remains unavailable.
- Use a Windows `IFileOperation` implementation in `SuperDuper.Windows.Infrastructure` on an STA
  thread.
- Keep the worker authoritative for plans, preflight records, operation IDs, and durable results.
- Freeze the submitted plan revision during execution.
- Execute bounded batches and receive per-item Shell progress/outcomes.
- Make worker commands idempotent so result reporting can be retried after a connection failure.
- Serialize scan and deletion filesystem mutations until concurrency semantics are designed.

#### Review screen

- Show files, folders, total logical bytes, expected recoverable bytes, and affected locations.
- Separate ready, invalidated, unavailable, and non-recyclable items.
- Require one final confirmation that names the Recycle Bin behavior.
- Show cancellable preflight and operation progress.
- Offer `Open Recycle Bin` after completion; do not promise application-level restore semantics.

#### Acceptance criteria

- A stale or changed target cannot be recycled from historical data alone.
- Folder deletion is blocked when its current tree differs from the verified scan tree.
- Per-item success, failure, cancellation, and unknown outcomes are durable.
- Crash recovery can reconcile prepared, executing, completed, partially completed, and unknown
  operations.
- No permanent-delete path is reachable from the Windows app.

### Milestone 12 - Live Reconciliation and External Filesystem Changes

#### User outcome

The working results stay understandable after in-app deletion, Explorer deletion, rename, file
change, drive removal, network loss, or application restart.

#### Live-state overlay

Support states equivalent to:

```text
present
missing
changed
moved
recycled_by_app
externally_removed
unavailable
cloud_offline
validation_pending
```

The original run remains available through a `Show original scan` view. Working review queries join
the immutable snapshot with the latest validated live state.

#### Reconciliation behavior

- Validate visible rows lazily in bounded background batches.
- Validate a complete duplicate set when selected.
- Validate the full submitted plan before execution.
- Apply known in-app operation outcomes immediately.
- Recompute working copy counts and recoverable bytes after state changes.
- Move sets with fewer than two live physical copies to `Resolved` rather than deleting history.
- Where stable identity permits, associate a rename or move within selected roots with the original
  snapshot while preserving the original path.

#### Filesystem notifications

- Treat directory notifications as hints, not authoritative state.
- Coalesce events and send bounded `result.state_changed` notifications to the UI.
- If a notification buffer overflows, mark the affected root dirty and reconcile it.
- Fall back to selection-time, visible-page, plan-time, and manual validation for UNC paths,
  unsupported filesystems, watcher failure, and app downtime.
- Evaluate the NTFS USN journal later as an optional incremental-rescan optimization; do not make it
  a correctness requirement or an administrative-permission requirement for this milestone.

#### Acceptance criteria

- In-app removals update working lists and summaries without a complete rescan.
- External deletion and modification invalidate affected review decisions.
- Watcher overflow results in a visible dirty/reconciliation state, never silent trust.
- Mass changes do not generate one WPF dispatcher update per filesystem event.
- Historical-run queries remain reproducible regardless of live state.

## Wave 3 - Explain What Happened

### Milestone 13 - Activity and Issues Workspace

#### User outcome

Every meaningful warning or file-operation outcome can be understood and, when possible, acted on
without reading the developer diagnostic log.

#### Event model

Persist structured events with fields equivalent to:

```text
event id
session id
run id
file-operation id
timestamp
severity
phase/category
stable event code
path or related item id
human-readable message
structured details
occurrence count
resolution state
```

Introduce event persistence and protocol hooks in earlier milestones as their features require it.
Milestone 13 delivers the complete user-facing Activity experience.

#### Activity UX

- Default to the selected run, with session-wide and application-wide scopes.
- Filter by severity, phase, category, path, operation, and resolution state.
- Show timestamp, severity, reason, affected item, and available action.
- Provide reveal, copy details, retry validation, open related set/plan, and exclude parent next scan
  when relevant.
- Make the run warning count and status-bar warning entry points open a filtered Activity view.
- Keep the diagnostic log as a separate developer/recovery artifact.

#### Required categories

- cloud subtree excluded;
- access denied or unavailable root;
- link or reparse point skipped;
- file changed or vanished during scan;
- hash or cache failure;
- review decision invalidated;
- recycle success, failure, cancellation, or unknown result;
- external deletion, move, or modification;
- notification overflow and reconciliation requirement;
- worker interruption and recovery.

#### Scale rules

- Page and sort events in the worker.
- Batch inserts and avoid one transaction per event.
- Aggregate repeated events by stable code and useful subtree when individual rows add no value.
- Retain exact affected counts and representative samples.
- Bound message/detail sizes and keep sensitive paths local.

#### Acceptance criteria

- Every run warning count is drillable or explicitly represented by an aggregate with examples.
- An actionable event navigates to the relevant item, duplicate set, plan, or session setting.
- Activity memory use is independent of total event count.
- Deletion and reconciliation outcomes can be audited after restart.

### Milestone 14 - UX and Scale Hardening

#### Scope

- Complete keyboard-only review and deletion workflows.
- Validate screen-reader names, high contrast, DPI behavior, focus restoration, and reduced-motion
  expectations.
- Refine empty, stale, dirty, unavailable, invalidated, resolved, and partial-success states.
- Add saved filters and reusable preferred-location profiles.
- Export results, decisions, deletion outcomes, and activity in a documented format.
- Add run-to-run deltas for new, changed, and resolved duplicate sets.
- Add bounded cache-only Shell thumbnails only if they do not compromise cloud safety or UI
  responsiveness.
- Instrument review, facet, rule preview, plan, preflight, operation, reconciliation, and event
  queries.
- Exercise large-result and large-operation fixtures in Release configuration.

#### Acceptance criteria

- The full workflow is usable without a mouse.
- UI state remains coherent through cancellation, partial failure, worker restart, and drive loss.
- Performance remains bounded for 100,000 duplicate groups, large member sets, large review plans,
  and large Activity histories.
- Cloud-safe behavior remains true for scan, review, preview, validation, and deletion.

## Deferred Follow-On Work

The following should not be mixed into the exact-duplicate deletion path without a separate plan:

- similar or near-duplicate folders;
- automatic background or scheduled scanning;
- an Explorer context-menu shell extension;
- permanent deletion;
- application-managed Recycle Bin restore;
- content preview that hydrates cloud files implicitly;
- USN-journal-dependent correctness;
- cross-platform UI behavior.

Similar folders must remain visibly distinct from verified exact duplicates and must not be
presented as deletion-ready.

## Cross-Cutting Architecture Rules

- Rust remains the only owner of the product SQLite database.
- WPF views remain in `SuperDuper.Windows`; application contracts and view models remain in
  `SuperDuper.Windows.Core`; Windows Shell and process/native integration remain in
  `SuperDuper.Windows.Infrastructure`.
- Protocol additions use allow-listed fields, structured errors, bounded messages, idempotent
  mutation commands, and stable paging.
- Review, live-state, operation, and event changes use forward schema migration with rollback on
  failure; opening a newer unknown schema remains a hard error.
- Filesystem, Shell, SQLite, and process operations never run on the WPF dispatcher.
- State-changing operations are serialized unless an explicit concurrency design proves safety.
- Automated tests must distinguish immutable historical queries from mutable working-view queries.

## Performance Guardrails

Initial targets should be measured and refined on representative Windows 11 hardware:

- result, facet, plan, and event first-page warm query target: 100 ms;
- progress and live-state UI updates: at most 10 per second;
- no per-file WPF dispatcher update during scan, reconciliation, or deletion;
- bounded page, thumbnail, validation, and event caches;
- cancellation checks within every filesystem batch;
- bounded deletion batches rather than one unbounded protocol frame or Shell item array;
- no full-result materialization for rule preview or plan summary;
- no content read, thumbnail extraction, or validation of excluded cloud placeholders.

## Recommended First Implementation Slice

Implement Milestone 7, Milestone 8, and only the durable manual-decision portion of Milestone 10:

1. Add the cloud policy to session/run contracts.
2. Guarantee no hydration for excluded sync roots.
3. Add the Review landing summary and enhanced duplicate-set detail.
4. Persist manual `Keep`, `Remove`, and `Undecided` decisions.
5. Enforce at least one independently accessible survivor.
6. Do not expose deletion yet.

This slice delivers meaningful UX, establishes the state separation required by later deletion,
and avoids extending the current `marked_deleted` flag into the product workflow.

## Milestone Definition Template

Before implementation of each milestone, add or refine:

- a short user story and non-goals;
- UI states and keyboard/accessibility behavior;
- schema migration and rollback behavior;
- protocol commands, events, idempotency, paging, and structured error codes;
- performance budgets and cancellation points;
- unit, protocol, infrastructure, WPF smoke, and operator-acceptance tests;
- recovery behavior for worker exit, app exit, unavailable roots, and partial operations;
- documentation updates and known limitations.

## Iteration Log

Record durable planning changes here. Implementation detail belongs in milestone-specific plans or
code reviews rather than a conversational transcript.

| Date | Change | Rationale |
| --- | --- | --- |
| 2026-08-17 | Established Milestones 7-14 and recommended first slice. | Make duplicate review, safe deletion, live synchronization, Activity, and cloud safety iterable across sessions. |
| 2026-08-17 | Refined Milestone 7 and started the fail-closed `exclude_registered_roots` vertical slice. | Make registered-root exclusion and immutable policy snapshots enforceable before exposing opt-in placeholder or hydration behavior. |
| 2026-08-17 | Implemented schema v4, cloud-aware session/run protocol fields, Infrastructure registration discovery, pre-I/O subtree pruning, bounded exclusion records, and WPF setup/summary states. | Deliver the safe default without exposing placeholder hydration, preview, validation, deletion, or review decisions; migrated sessions intentionally require a fresh detection before rerun. |
| 2026-08-17 | Hardened Milestone 7 acceptance with discovery fallbacks, command-state invalidation, Cloud setup UI automation, and a repeatable real-provider operator gate; Debug/Release available-provider OneDrive runs passed with unchanged placeholder/allocation/provider-transfer state. | Preserve the pre-I/O boundary with evidence from a real online file, offline placeholder, broad ancestor, and explicit cloud root while leaving the provider-unavailable operator pass explicit and never stopping sync software automatically. |
| 2026-08-17 | Accepted the fail-closed Milestone 7 surface after the Release consumer OneDrive `-ExpectProviderUnavailable` run passed with unchanged placeholder/allocation/provider-transfer state and zero discovered files. | Close the real unavailable-provider gate using an operator-controlled provider exit while continuing to withhold both advanced policies; a commercial-root attempt without an offline placeholder was recorded as a fixture limitation. |
| 2026-08-17 | Refined and started the first read-only Milestone 8 vertical slice: filtered file-review summary plus immutable selected-root/drive context on bounded member pages. | Improve result understanding through the existing worker-owned group/member paging boundary without entering durable decisions, live validation, facets, folder intelligence, or deletion scope. |
| 2026-08-17 | Accepted the first Milestone 8 slice after focused/optimized tests, the full Debug/Release Rust and .NET matrix, and real Debug/Release WPF smoke passed. | Establish a worker-owned filtered summary and bounded location-aware file detail; keep the broader workspace, facets, decisions, validation, and deletion as explicit later gates. |
| 2026-08-17 | Refined and accepted the bounded per-set Milestone 8 location-span slice after focused/optimized tests, the full Debug/Release Rust and .NET matrix, and real Debug/Release WPF smoke passed. | Expose immutable selected-root and cross-drive breadth through existing group pages, caches, cancellation, and query generations without adding facets, schema, filesystem access, decisions, validation, or deletion. |
