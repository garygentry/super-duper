# Windows Post-MVP Duplicate Review UX Plan

## Status

Active implementation roadmap for the Windows duplicate-review experience. Milestone 6
release-acceptance remediation and the fail-closed Milestone 7 slice are complete. The first twelve
read-only Milestone 8 slices and the first five bounded accessibility-remediation slices are implemented
and accepted; the broader milestone remains in progress and is gated by the remaining criteria
below.

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

Status: The first twelve read-only file-review slices are accepted. They add a worker-owned summary
for the current duplicate-file query, immutable member location context, bounded per-set
selected-root/drive span, a worker-owned across-drives entry point, and aggregate location coverage
plus bounded selected-root and drive facets with exact filters without introducing review
decisions, live filesystem state, or deletion behavior. The seventh slice adds a worker-owned
minimum-copy-count filter through the same bounded group and cross-facet query paths. Full
Milestone 8 remains in progress. The eighth slice adds a precise, accessible 1 GB-or-larger entry
point over the existing indexed worker-owned one-copy-size predicate. The ninth slice adds bounded
next/previous-set navigation and returns keyboard focus to the selected virtualized group row. The
tenth slice adds indexed exact canonical-member-path matching while retaining the existing literal
substring path search as a distinct default mode. The eleventh slice adds indexed any-member exact
filename-extension and explicit no-extension matching without inferring from the representative
label or introducing file-type classification.
The twelfth slice adds indexed all-member filename-extension and all-member no-extension matching
as an explicit opt-in while preserving the accepted any-member default.
The first bounded accessibility-remediation slice makes the primary duplicate-file filters reflow
inside the supported narrow workspace while preserving their explicit keyboard and UI Automation
order.
The second bounded accessibility-remediation slice raises explicit, coalesced UI Automation
notifications when the current duplicate-file group query completes or fails.
The third bounded accessibility-remediation slice does the same for the current selected-set
member query while leaving non-displayed prefetch silent.
The fourth bounded accessibility-remediation slice announces explicit selected-root and drive
facet paging and sorting outcomes while keeping filter-driven refreshes and non-displayed prefetch
silent.
The fifth bounded accessibility-remediation slice keeps Session Setup editors inside the supported
narrow workspace while retaining internal scrolling for long paths and patterns.

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

#### Across-drives filter slice (2026-08-17)

##### Audit and scope decision

The accepted group path already binds normalized search and minimum-size filters to the worker's
summary, total, opaque cursor signature, five-page Core cache, cancellation source, and group query
generation. A boolean across-drives predicate is therefore the smallest useful result-understanding
entry point: it reuses that complete bounded path and avoids a separate facet command, cache,
collection, or stale-response channel. A root/drive facet remains broader work because it needs its
own bounded paging and query-generation design.

##### Implemented contract and UX

- `duplicate_file_group.page.filter.acrossDrives` is an optional boolean with a `false` default.
  When true, SQLite returns only run-owned groups with more than one distinct, non-empty,
  case-insensitive drive label in their immutable scanned members.
- Rows, total, and the review summary share the same predicate. The boolean is part of the opaque
  cursor query signature, so a cursor from either filter state fails closed as `invalid_cursor` in
  the other state.
- Schema v4 and existing sorts are unchanged. The query uses indexed group/member ownership joins
  and performs no filesystem, Cloud Files, preview, validation, durable-decision, deletion, or
  `scanned_file.marked_deleted` work.
- Core sends the boolean through the existing filter value and uses the existing cache,
  cancellation, prefetch, and group generation. Clear filters resets it. WPF adds one keyboard and
  UI Automation accessible `Across drives` checkbox; applying it never creates an unbounded client
  collection.

##### Acceptance result

- Focused Debug storage and worker tests passed predicate/summary agreement, run ownership,
  case-insensitive non-empty drive counting, protocol serialization, and cursor rejection across
  filter states. Five focused Core tests, the real typed-client Infrastructure lifecycle test, and
  the targeted STA WPF surface test passed under SDK 10.0.400 from `C:\Windows\Temp` with absolute
  project paths; `global.json` was unchanged.
- The 100,000-group regression now includes a sparse immutable cross-drive population and exercises
  ordinary first/next keyset pages plus the across-drives query. The complete test passed in 1.64
  seconds in Debug and 1.01 seconds in optimized Release, within the five-second bounded gate.
  Representative-hardware profiling against the 100 ms warm-page target remains a full-milestone
  gate.
- `cargo test --workspace` and `cargo test --workspace --release` passed, including 15 storage and
  9 worker tests in each configuration. Debug and Release worker builds passed.
- Debug and Release solution builds passed with zero warnings and zero errors. Debug passed 37 Core,
  22 Infrastructure, and 3 WPF tests. The first parallel Release solution-test attempt passed Core
  and WPF but one unrelated Infrastructure lifecycle worker handshake timed out; that test passed
  immediately in isolation, and the complete serialized Release rerun passed 37 Core,
  22 Infrastructure, and 3 WPF tests. The one real-provider Infrastructure test remained
  intentionally environment-gated in both configurations.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed worker-owned
  across-drives filtering, accessible checkbox application/reset, the existing result
  summary/location/paging/sorting/Explorer workflows, Cloud setup/fail-closed behavior, and
  deterministic shutdown coverage. Targeted Rust formatting, PowerShell parsing, and
  `git diff --check` passed.
- Remaining Milestone 8 gates are paged selected-root/drive facets and richer server-side filters,
  next/previous-set keyboard focus restoration, the complete accessibility review, and
  representative-hardware warm-query/memory profiling. Durable decisions remain Milestone 10 and
  deletion remains Milestone 11.

#### Aggregate location-summary slice (2026-08-17)

##### Audit and scope decision

The accepted group-page path already keeps its normalized search, minimum-size, and across-drives
predicate aligned across rows, total, summary, opaque cursor signature, five-page Core cache,
cancellation source, prefetch, and group query generation. Extending that same summary is the
smallest bounded result-understanding entry point. A selected-root/drive facet still requires a
separate command, cursor kind, cache, cancellation source, query generation, and WPF paging surface,
so it remains an explicit later gate.

##### Implemented contract and UX

- `duplicate_file_group.page.summary` now includes the distinct non-empty, case-insensitive selected
  roots and drive labels represented anywhere in the matching sets, plus the number of matching
  sets spanning multiple drives. All values use the exact current run/search/minimum-size/
  across-drives predicate.
- SQLite computes the aggregates from indexed, run-owned `duplicate_group_member` and
  `scanned_file` rows. The worker returns counts only; it does not materialize or serialize a
  complete member or facet collection, and it performs no filesystem access.
- The fields travel with every bounded group page and therefore reuse the existing cursor, cache,
  cancellation, stale-response rejection, and query generation. Schema v4, group/member sorts,
  page limits, and cursor signatures are unchanged.
- WPF adds one wrapping result-summary strip with stable automation IDs and readable selected-root,
  drive, and cross-drive-set wording. Filtering updates it atomically with the visible page and
  existing review summary.
- The slice adds no preview, thumbnail, validation, Cloud Files access, durable decisions,
  deletion, unbounded WPF state, or use of `scanned_file.marked_deleted`.

##### Acceptance result

- Focused Debug storage and worker protocol regressions passed predicate agreement, run ownership,
  case-insensitive/non-empty location counting, serialization, and across-drives behavior. All five
  focused Core duplicate-file tests, the typed real-worker Infrastructure lifecycle test, and the
  targeted STA WPF surface test passed under SDK 10.0.400 from `C:\Windows\Temp` with absolute
  project paths; the unavailable pinned 10.0.303 SDK and `global.json` were unchanged.
- The 100,000-group ordinary/across-drives regression, including the new aggregate, completed in
  1.69 seconds Debug and 0.91 seconds optimized Release, within the five-second bounded gate.
  Representative-hardware profiling against the 100 ms warm-query target and explicit
  bounded-memory measurement remain full-milestone gates.
- `cargo test --workspace` and `cargo test --workspace --release` passed, including 15 storage and
  9 worker tests in each configuration. Debug and Release worker builds passed.
- Debug and Release solution builds passed with zero warnings and zero errors. Each configuration
  passed 37 Core, 22 Infrastructure, and 3 WPF tests; the one real-provider Infrastructure test
  remained intentionally environment-gated. Tests were serialized with `-m:1`.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed worker-owned filtered
  aggregate location coverage, accessible WPF location text, existing result summary/location/
  paging/sorting/Explorer workflows, Cloud setup/fail-closed behavior, and deterministic shutdown.
  Targeted Rust formatting, PowerShell parsing, and `git diff --check` passed.
- Remaining Milestone 8 gates are paged selected-root/drive facets and richer server-side filters,
  next/previous-set keyboard focus restoration, the complete accessibility review, and
  representative-hardware warm-query/bounded-memory profiling. Durable decisions remain Milestone
  10 and deletion remains Milestone 11.

#### Paged selected-root facet slice (2026-08-17)

##### Audit and scope decision

The accepted group channel owns the current search, minimum-size, across-drives predicate,
summary, keyset cursor, five-page Core cache, cancellation source, and query generation. Selected
roots are immutable user-chosen scan context and can number up to 64, so a selected-root facet is a
more useful first paged facet than the usually small drive set. The facet is a separate read-only
channel because deriving values or counts from group/member pages would be incomplete and
unbounded. Drive facets and other richer filters remain later slices.

##### Implemented contract and UX

- `duplicate_file_selected_root_facet.page` returns distinct non-empty, case-insensitive immutable
  root values plus matching-set counts. SQLite applies the current search, minimum-size, and
  across-drives predicate, but intentionally ignores the current root selection so users can
  switch roots. It supports `matchingGroupCount` and `value` sorts, stable ID tie breaking, and
  1-500 row keyset pages.
- The facet cursor has its own kind and an explicit query signature binding run, facet sort and
  direction, search, minimum size, and across-drives. `duplicate_file_group.page.filter.selectedRoot`
  adds one exact case-insensitive root predicate to rows, total, summary, and the group cursor
  signature. Schema v4 is unchanged and all queries remain run-owned SQLite reads.
- Core shows 25 facet values at a time, caches at most five cursor pages, and owns an independent
  cancellation source and query generation. A late facet response cannot replace a newer query;
  group and member channels retain their independent stale-response rules. WPF never binds the
  complete facet dataset and adds no filesystem, preview, thumbnail, validation, cloud, decision,
  deletion, or `scanned_file.marked_deleted` behavior.
- WPF exposes one accessible selected-root ComboBox with worker-owned counts, count/name sorts,
  previous/next facet paging, persistent active-filter text, and existing Apply/Clear semantics.
  Real smoke selects the facet by keyboard. The first automation pass exposed object type names
  from the ComboBox items; explicit item-container automation names fixed that accessibility defect.

##### Acceptance result

- Focused Debug storage and worker protocol tests passed run ownership, case-insensitive exact-root
  filtering, count/name sorting, forward/backward keyset paging, and cursor rejection across facet
  sort and group-filter signatures. The focused Core suite passed 38 tests, including the new
  five-page facet-cache bound and late-response rejection. The real typed-client Infrastructure
  lifecycle test and all three STA WPF surface tests passed.
- The expanded 100,000-group ordinary/across-drives regression now includes selected-root facet
  and exact-root filter coverage. Its latest serialized focused runs completed in 3.06 seconds
  Debug and 1.42 seconds optimized Release, within the five-second regression gates.
  Representative-hardware warm-query and explicit bounded-memory profiling remain full-milestone
  gates.
- `cargo test --workspace` and `cargo test --workspace --release` passed, including 15 storage and
  9 worker tests in each configuration. Debug and Release worker builds passed.
- Debug and Release solution builds passed with zero warnings and zero errors under installed SDK
  10.0.400 from `C:\Windows\Temp` with absolute solution paths; the unavailable pinned 10.0.303
  SDK and `global.json` were unchanged. Final serialized tests passed 38 Core, 22 Infrastructure,
  and 3 WPF tests in each configuration; the one real-provider Infrastructure test remained
  intentionally environment-gated. An initial cross-configuration parallel run produced three
  Debug worker-handshake timeouts under contention; the typed test passed in isolation and both
  complete serialized reruns passed.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed worker-owned facet values,
  counts, sorting and exact-root filtering; accessible keyboard facet selection; existing result
  summary/location/paging/sorting/Explorer workflows; Cloud setup/fail-closed behavior; and
  deterministic shutdown. Targeted Rust formatting, PowerShell parsing, and `git diff --check`
  passed.
- Remaining Milestone 8 gates are a paged drive facet and richer worker-owned filters,
  next/previous-set keyboard focus restoration, the complete accessibility review, and
  representative-hardware warm-query/bounded-memory profiling. Durable decisions remain Milestone
  10 and deletion remains Milestone 11.

#### Paged drive facet slice (2026-08-17)

##### Audit and scope decision

Drive labels are naturally lower-cardinality than selected roots and will usually fit on one page,
while a copy-count threshold would reuse the existing group channel with less implementation. The
drive facet is nevertheless the more valuable next result-understanding entry point: it completes
the accepted location workflow from aggregate drive coverage to a specific drive's sets, composes
with the selected-root facet, and closes the remaining location-facet gate. It remains keyset-paged
and bounded so mapped, removable, UNC, migrated, and future label sets do not become an implicit
client-side cardinality assumption.

##### Implemented contract and UX

- `duplicate_file_drive_facet.page` returns distinct non-empty, case-insensitive immutable drive
  labels with matching-set counts. It applies search, minimum size, across-drives, and the current
  exact selected-root predicate while intentionally ignoring the current drive selection so users
  can switch drives. Count/name sorts and forward/backward pages remain worker-owned.
- `duplicate_file_group.page.filter.selectedDrive` adds an exact case-insensitive drive predicate
  to rows, total, summary, and the group cursor signature. The selected-root facet applies that
  drive predicate; the drive facet applies the selected-root predicate. Each facet ignores only
  itself so their alternatives remain useful together.
- The drive cursor uses the separate `duplicate-file-drive-facets` kind and an explicit signature
  containing run, sort/direction, normalized search, minimum size, across-drives, and selected root.
  Core uses a separate five-page LRU cache, cancellation source, query generation, prefetch path,
  error state, and stale-response guard. WPF binds only the current 25-value page.
- WPF adds an accessible drive ComboBox with explicit item-container names, count/name sorts,
  previous/next drive paging, and persistent active-filter text. It shares the existing Apply/Clear
  behavior without materializing group/member pages or deriving counts on the client.
- Schema v4 is unchanged. All values and counts come from run-owned SQLite rows. The slice adds no
  filesystem or Cloud Files access, preview, thumbnails, validation, durable decisions, deletion,
  `scanned_file.marked_deleted` use, or unbounded WPF collection.

##### Acceptance result

- Focused Debug storage and worker protocol tests passed exact-drive filtering, cross-facet
  composition, case-insensitive/non-empty labels, count/name sorting, forward/backward keyset
  paging, and cursor rejection across root/drive signatures. Seven focused Core duplicate-file
  tests passed, including the independent drive-generation stale-response regression and five-page
  cache bound. The real typed-client lifecycle tests and targeted STA WPF surface test passed. The
  first typed-client attempt correctly exposed that `cargo check` had left the old worker executable
  in place; after the actual Debug worker build, the focused and full typed-client runs passed.
- The expanded 100,000-group ordinary/across-drives/root/drive facet and exact-filter regression
  completed in 2.15 seconds Debug and 1.59 seconds optimized Release, within the five-second
  regression gates. Representative-hardware warm-query and explicit bounded-memory profiling
  remain full-milestone gates.
- `cargo test --workspace` and `cargo test --workspace --release` passed, including 15 storage and
  9 worker tests in each configuration. Debug and Release worker builds passed.
- Debug and Release solution builds passed with zero warnings and zero errors under installed SDK
  10.0.400 from `C:\Windows\Temp` with absolute solution paths; the unavailable pinned 10.0.303
  SDK and `global.json` were unchanged. Serialized tests passed 39 Core, 22 Infrastructure, and 3
  WPF tests in each configuration; the one real-provider Infrastructure test remained
  intentionally environment-gated.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed worker-owned drive values,
  counts, sorting, selected-root composition and exact-drive filtering; accessible keyboard drive
  selection; existing root/across-drives/summary/location/paging/sorting/Explorer workflows; Cloud
  setup/fail-closed behavior; and deterministic shutdown.
- Remaining Milestone 8 gates are richer worker-owned filters, next/previous-set keyboard focus
  restoration, the complete accessibility review, and representative-hardware warm-query/
  bounded-memory profiling. Durable decisions remain Milestone 10 and deletion remains Milestone
  11.

#### Minimum-copy-count filter slice (2026-08-18)

##### Audit and scope decision

`duplicate_group.file_count` is immutable, stored for every run-owned duplicate set, already used
by the copy-count sort, and covered by `idx_group_run_count`. A normalized minimum-copy-count
predicate can therefore reuse the existing group rows, total, summary, keyset cursor, five-page
Core cache, cancellation source, and query generation while both location facets keep their
independent caches, cancellation sources, generations, and stale-response guards.

An extension/type filter is broader: the current displayed type is derived after SQL from only the
representative name, while exact-content group members may have different extensions. An accurate
any-member extension filter needs explicit semantics and likely a stored/indexed facet rather than
representative-label inference. More-specific path filtering also needs prefix/exact/descendant and
case-normalization semantics beyond the existing case-insensitive member-path substring search.
The indexed copy-count threshold is therefore the smallest useful next result-understanding entry
point.

##### Implemented contract and UX

- The duplicate-file group, selected-root facet, and drive facet filters add
  `minimumCopyCount`, defaulting to `2` and rejecting values below `2`. SQLite applies
  `duplicate_group.file_count >= ?` in the shared normalized group predicate.
- Group rows, total, review/location summary, and both cross-composed facet counts apply the same
  threshold. All three opaque cursor signatures include the normalized value, so cursors fail
  closed when the threshold changes.
- Core carries the threshold through the existing group path and both independent facet paths.
  Their five-page cache bounds, cancellation sources, query generations, prefetching, and
  stale-response rejection are unchanged. WPF adds one keyboard/UI-Automation-accessible `Three or
  more copies` checkbox and continues to bind only current bounded pages.
- Schema v4 is unchanged. Every query remains a run-owned SQLite read. The slice performs no
  filesystem or Cloud Files access and adds no preview, thumbnail, validation, durable decision,
  deletion, `scanned_file.marked_deleted`, or unbounded WPF state.

##### Acceptance result

- Focused Debug storage and worker protocol tests passed threshold validation, predicate/summary
  agreement, group and both facet cursor-signature rejection, and group/facet count behavior. All
  seven focused Core duplicate-file tests, the typed real-worker Infrastructure lifecycle test,
  and the targeted STA WPF surface test passed under installed SDK 10.0.400 from
  `C:\Windows\Temp` with absolute project paths; the unavailable pinned 10.0.303 SDK and
  `global.json` were unchanged.
- The expanded 100,000-group ordinary/keyset, across-drives, root/drive facet, exact-filter, and
  minimum-copy-count group/facet regression completed in 2.21 seconds Debug and 1.39 seconds
  optimized Release, within the five-second regression gates. Representative-hardware warm-query
  and explicit bounded-memory profiling remain full-milestone gates.
- `cargo test --workspace` and `cargo test --workspace --release` passed, including 15 storage and
  9 worker tests in each configuration. Debug and Release worker builds passed.
- Debug and Release solution builds passed with zero warnings and zero errors under installed SDK
  10.0.400 from `C:\Windows\Temp` with absolute solution paths. Serialized tests passed 39 Core,
  22 Infrastructure, and 3 WPF tests in each configuration; the one real-provider Infrastructure
  test remained intentionally environment-gated.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed worker-owned
  minimum-copy-count group/summary/root-facet/drive-facet behavior, the accessible WPF entry point,
  existing location/paging/sorting/Explorer workflows, Cloud setup/fail-closed behavior, and
  deterministic shutdown. Targeted Rust formatting, PowerShell parsing, and `git diff --check`
  passed. Repository-wide `cargo fmt --all -- --check` remains blocked by pre-existing formatting
  drift in unchanged CLI/FFI files outside this slice; those unrelated files were preserved.
- Remaining Milestone 8 gates are additional richer worker-owned filters, next/previous-set
  keyboard focus restoration, the complete accessibility review, and representative-hardware
  warm-query/bounded-memory profiling. Durable decisions remain Milestone 10 and deletion remains
  Milestone 11.

#### One-gigabyte size entry-point slice (2026-08-18)

##### Audit and scope decision

An extension/type filter is not a safe small follow-up because exact-content members can have
different extensions and the displayed representative type is derived from only the representative
name. Worker-owned extension semantics need an explicit any-member/all-member model, normalized
extension storage, and an index before they can be exposed. A path-prefix filter likewise needs
explicit path-segment, descendant, case-normalization, selected-root-relative, and boundary
semantics; re-labeling the existing member-path substring search would be misleading.

The existing `minimumSize` predicate is already defined over immutable
`duplicate_group.file_size` (one-copy size), covered by `idx_group_run_size`, and normalized through
group rows, total, summary, the group cursor signature, and both cross-composed facet predicates and
signatures. A precise size preset is therefore the smallest coherent result-understanding entry
point and requires no new protocol field or query channel.

##### Implemented contract and UX

- WPF adds an accessible `1 GB or larger` checkbox whose automation name states the exact one-copy
  threshold: 1,073,741,824 bytes. Core normalizes the effective `minimumSize` to the greater of the
  manually entered non-negative byte threshold and 1,073,741,824 while the preset is active.
- SQLite continues to apply `duplicate_group.file_size >= ?` through the single normalized group
  predicate. Rows, total, review/location summary, selected-root facet counts, and drive facet
  counts therefore share the same threshold, and all three existing cursor signatures continue to
  fail closed when it changes.
- The group, member, selected-root-facet, and drive-facet caches remain independently bounded to
  five pages. Their existing cancellation sources, query generations, prefetch limits, and
  stale-response rejection are unchanged; no new WPF collection or facet channel is introduced.
- Schema v4 remains unchanged and Rust retains exclusive SQLite ownership. The slice reads only
  immutable run-owned rows and adds no filesystem or Cloud Files access, preview, thumbnail,
  validation, durable decision, deletion, `scanned_file.marked_deleted`, or Milestone 10/11 state.

##### Acceptance result

- Focused storage and worker tests passed minimum-size predicate/summary/facet agreement and group,
  selected-root-facet, and drive-facet cursor-signature rejection. Eight focused Core duplicate-file
  tests and the targeted STA WPF surface test passed under installed SDK 10.0.400 from
  `C:\Windows\Temp` with absolute project paths; the unavailable pinned 10.0.303 SDK and
  `global.json` were unchanged.
- The expanded 100,000-group ordinary/keyset, minimum-size, across-drives, minimum-copy-count, and
  root/drive facet regression completed in 2.24 seconds Debug and 0.88 seconds optimized Release,
  within its five-second gates. Representative-hardware warm-query and explicit bounded-memory
  profiling remain full-milestone gates.
- `cargo test --workspace` and `cargo test --workspace --release` passed, including 15 storage and
  9 worker tests in each configuration. Debug and Release worker builds passed.
- Debug and Release solution builds passed with zero warnings and zero errors. Serialized tests
  passed 40 Core, 22 Infrastructure, and 3 WPF tests in each configuration; the one real-provider
  Infrastructure test remained intentionally environment-gated.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed worker-owned minimum-size
  group/summary/root-facet/drive-facet behavior, the accessible fixed-threshold WPF toggle and
  restoration path, existing location/paging/sorting/Explorer workflows, Cloud setup/fail-closed
  behavior, and deterministic shutdown. Targeted Rust formatting, PowerShell parsing, and
  `git diff --check` passed. Repository-wide `cargo fmt --all -- --check` remains blocked by
  pre-existing formatting drift in unchanged CLI/FFI files, which remain preserved.
- Remaining Milestone 8 gates are further richer worker-owned filters once their semantics and
  indexing are explicit, next/previous-set keyboard focus restoration, the complete accessibility
  review, and representative-hardware warm-query/bounded-memory profiling. Durable decisions remain
  Milestone 10 and deletion remains Milestone 11.

#### Next/previous-set keyboard focus-restoration slice (2026-08-18)

##### Audit and scope decision

Extension/type filtering is still not a safe small slice. The displayed representative type is
derived from one representative label, while exact-content members may use different extensions.
An any-member extension predicate would mean that at least one immutable member has the requested
normalized extension; an all-member predicate would mean that every immutable member has that
extension, with a separately defined no-extension value. Before either behavior is chosen, the
contract must define terminal-dot, dotfile, multiple-suffix, empty-extension, Unicode, and
case-normalization rules; distinguish extension from a maintained file-type classification; store
or generate the normalized value under Rust ownership; and add a run/group/value index that can
serve group rows, total, summary, and both cross-facet count paths. The current client-derived
representative type and `idx_file_run_path` do not satisfy that contract.

A more-specific path filter is also broader than this slice. Exact canonical-path equality,
boundary-aware canonical-path prefix/descendant matching, and selected-root-relative matching are
different operations. Prefix/descendant semantics must specify separator and path-segment
boundaries, whether the root itself matches, case normalization, selected-root-relative behavior,
and an indexable worker representation. The existing group search remains a case-insensitive
literal substring across immutable member canonical paths and continues to be labeled `Path
search`; it is not relabeled as a path-prefix filter.

Next/previous-set focus restoration is therefore the smallest coherent remaining gate. It reuses
the current 200-row group page, opaque group cursors, five-page group cache, cancellation source,
query generation, and stale-response guard. Selecting a new set continues to cancel the previous
member request and advance the independent member generation.

##### Implemented interaction

- `Previous set` and `Next set` move within the current bounded group page. At a page boundary they
  load one existing previous/next keyset cursor; the next page selects its first row and the
  previous page selects its last row, preserving continuous sorted-set traversal.
- The controls expose stable automation IDs, descriptive automation names, and Alt+P/Alt+N access
  keys. After keyboard, mouse, or UI Automation invocation, WPF realizes the selected virtualized
  row with `ScrollIntoView`, updates layout, and returns keyboard focus to that row so arrow-key
  review can continue.
- The navigation path adds no protocol field, cursor kind, SQL predicate, index, facet channel,
  cache, cancellation source, query generation, filesystem access, or unbounded collection. The
  existing group/member and independent selected-root/drive facet bounds remain unchanged.
- Schema v4 and immutable historical results are unchanged. The slice adds no preview, thumbnail,
  validation, Cloud Files access, durable decision, deletion, `scanned_file.marked_deleted`, or
  Milestone 10/11 behavior.

##### Acceptance result

- Ten focused Core duplicate-file tests passed, including traversal within and across bounded group
  pages and rejection of a late member response from the previously selected set. All three
  focused STA WPF surface tests passed and verify accessible controls plus focus inside the selected
  realized group row.
- The unchanged 100,000-group regression passed in focused runs in 2.18 seconds Debug and 1.57
  seconds optimized Release. In the complete workspace runs it completed in 2.04 seconds Debug and
  0.88 seconds Release, within the five-second bounded gates. Representative-hardware warm-query
  and explicit bounded-memory profiling remain full-milestone gates.
- `cargo test --workspace` and `cargo test --workspace --release` passed, including 15 storage and
  9 worker tests in each configuration. Debug and Release worker builds passed.
- Debug and Release solution builds passed with zero warnings and zero errors under installed SDK
  10.0.400 from `C:\Windows\Temp` with absolute solution paths; the unavailable pinned 10.0.303
  SDK and `global.json` were unchanged. Serialized tests passed 42 Core, 22 Infrastructure, and 3
  WPF tests in each configuration; the one real-provider Infrastructure test was intentionally
  skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed next/previous-set
  selection and keyboard-focus restoration, existing worker-owned filters/facets/summary/location
  workflows, Explorer reveal, Cloud setup/fail-closed behavior, and deterministic shutdown.
  PowerShell parsing and `git diff --check` passed. Repository-wide `cargo fmt --all -- --check`
  remains blocked by pre-existing formatting drift in unchanged CLI/FFI files, which remain
  preserved.
- Remaining Milestone 8 gates are further richer worker-owned filters once their semantics and
  indexing are explicit, the complete accessibility review, and representative-hardware
  warm-query/bounded-memory profiling. Durable decisions remain Milestone 10 and deletion remains
  Milestone 11.

#### Exact canonical-member-path filter slice (2026-08-18)

##### Audit and scope decision

Extension/type filtering remains broader than one safe slice. Extension is a member attribute, not
a representative-label attribute. `any-member` means at least one immutable member has the
requested normalized extension; `all-member` means every immutable member has it. Both modes must
support an explicit no-extension value. The proposed extraction uses only the final filename
segment: a terminal dot, a name with no dot, and a dotfile whose only dot is its leading dot map to
no-extension; `.env.local` maps to `local`; and `archive.tar.gz` maps to `gz`. The stored key must
exclude the dot and apply locale-independent Unicode normalization plus case folding. A file-type
classification is a separate, versioned maintained mapping and must never be presented as the
extension itself. Correct group rows, totals, summaries, and cross-facet counts require a
Rust-owned run/group/member/extension representation and index; the representative-derived Type
column and current path index remain insufficient, so neither extension mode is exposed here.

Path filtering has three distinct contracts:

- Exact canonical-path equality compares one complete immutable `canonical_path`. The accepted
  slice rewrites no separators, device prefixes, dot segments, leading/trailing characters, or
  Unicode normalization forms; it applies locale-independent Unicode lowercase comparison only.
- A future canonical prefix mode is boundary aware. `prefix-or-self` matches equality or a path
  whose next character is a normalized separator; `descendant` requires that separator and at
  least one following segment. A directory root itself has no file snapshot, while a file equal to
  the anchor matches only the explicitly named `prefix-or-self` mode. It must normalize `/` and
  `\`, DOS/UNC device prefixes, root trailing separators, Unicode form, and case into a stored
  `canonical_path_key`; the existing substring `LIKE` predicate is not this mode.
- A future selected-root-relative mode requires an exact selected-root value and matches only
  members owned by that root. Its normalized relative key has no leading separator and uses the
  same whole-segment equality/prefix/descendant boundaries. An empty relative anchor denotes all
  file descendants of the selected root; the root directory itself is not a `scanned_file` row.
  It requires stored/indexed root and relative keys, such as
  `(run_id, root_path_key, relative_path_key, id)`.

The exact equality mode is the smallest complete result-understanding entry point because the
immutable canonical path already exists. Rust registers the `UNICODE_NOCASE` collation before
schema reconciliation and adds additive schema-v4 indexes on
`(run_id, canonical_path COLLATE UNICODE_NOCASE)` and `(file_id, group_id)`. Prefix/descendant and
selected-root-relative modes stay deferred until their separate normalized keys and range-query
indexes exist.

##### Implemented contract and UX

- `filter.pathMatch` is `substring` by default or `exact` on duplicate-file group,
  selected-root-facet, and drive-facet requests. A blank search normalizes back to `substring`;
  exact input is bounded to 32,767 Unicode scalar values and is not trimmed.
- Exact mode uses one indexed immutable member-path equality predicate. Group rows, total, review
  and location summary, selected-root facet counts, and drive facet counts share it. The group,
  selected-root-facet, and drive-facet cursor signatures include the normalized mode and reject a
  cursor from the other mode.
- Core carries the mode through the existing group and both facet requests. The group, member,
  selected-root-facet, and drive-facet caches remain independently bounded to five pages, with
  their existing cancellation sources, query generations, two-page directional prefetch limits,
  and stale-response rejection. No new facet channel or WPF collection is introduced.
- WPF keeps `Path search` as the default literal substring behavior and adds an `Exact path`
  checkbox with a stable automation ID, name, help text, and keyboard position. The complete
  immutable path remains available from member detail for copying into the filter.
- Schema version 4, immutable historical runs, and Rust-only SQLite ownership are preserved. The
  slice adds no filesystem or Cloud Files access, preview, thumbnail, validation, durable
  decision, deletion, `scanned_file.marked_deleted`, or Milestone 10/11 behavior.

##### Acceptance result

- Focused exact-path storage and worker protocol tests passed, the three typed worker-lifecycle
  Infrastructure tests passed, the focused Core duplicate-file suite passed 12 tests, and the
  focused WPF surface suite passed all 3 STA tests. The optimized large-result test covers exact
  group, selected-root-facet, and drive-facet queries without walking result pages.
- `cargo test --workspace` and `cargo test --workspace --release` passed with 16 storage and 9
  worker tests in each configuration. The expanded 100,000-group regression completed in 2.41
  seconds Debug and 1.03 seconds optimized Release. Explicit Debug and Release worker builds also
  passed.
- Debug and Release .NET builds, run from `C:\Windows\Temp` against the absolute solution path,
  passed with zero warnings and errors. Each configuration passed 44 Core, 22 Infrastructure, and
  3 WPF tests; the real-provider Infrastructure test remained intentionally skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` passed with exact member-path group,
  summary, selected-root-facet, and drive-facet protocol coverage plus WPF exact-path interaction.
  Both retained the accepted next/previous-set focus restoration and covered the existing facet,
  Explorer, cloud-fail-closed, and shutdown paths. The smoke's facet automation was hardened to
  use the keyboard contract because WPF combo-box list items do not consistently expose the UI
  Automation `SelectionItemPattern`.
- Targeted Rust formatting, PowerShell parsing, and `git diff --check` passed. Repository-wide
  `cargo fmt` remains intentionally blocked by the accepted pre-existing CLI/FFI formatting drift;
  those unrelated files were not reformatted.
- The complete accessibility review remains open. Accessibility refinements present in the
  worktree were preserved and covered by the current Debug/Release .NET and real WPF smoke runs,
  but this exact-path slice does not claim that broader gate.

#### Any-member filename-extension filter slice (2026-08-18)

##### Audit and scope decision

Boundary-aware canonical prefix/descendant filtering remains broader than one safe slice. It still
requires separator/device-prefix normalization, explicit prefix-or-self versus descendant-only
semantics, Unicode-form and case rules, and stored range keys. Selected-root-relative matching is a
separate operation requiring an exact selected root plus indexed root/relative keys. The existing
member-path substring search remains a literal case-insensitive substring operation and is not
renamed or reused as either path mode.

Filename extension can be a complete bounded slice when it remains an immutable member attribute.
The exposed `any-member` behavior means at least one set member has the requested key. A future
`all-member` behavior would require every immutable member to have that key; it is not exposed, so
mixed-extension exact-content sets remain discoverable through any-member matching. Filename
extension is not MIME or maintained file-type classification, and the representative-derived Type
label remains display-only.

The final persisted filename segment supplies the suffix after its last dot. A name with no dot, a
terminal dot, and a dotfile whose only dot is its leading dot have no extension; `.env.local` maps
to `local`, and `archive.tar.gz` maps to `gz`. Keys exclude the dot and use locale-independent
Unicode lowercase while preserving normalization form. Input is not trimmed, path-canonicalized,
or Unicode-form-normalized. An absent value means no extension predicate; an explicit empty value
means no extension.

##### Implemented contract and UX

- Schema v4 gains an additive internal nullable `scanned_file.extension_key` without advancing
  `user_version`. Rust writes it with each immutable snapshot. Opening an older v4 database adds
  the column if needed, backfills null keys transactionally in bounded 500-row SQLite batches, and
  creates `idx_file_run_extension_key(run_id, extension_key, id)`. Reconciled connections return
  after read-only checks; no filesystem or Cloud Files access occurs.
- `filter.extension` is optional on group, selected-root-facet, and drive-facet requests. It accepts
  at most 255 Unicode scalar values without dots or path separators. Empty explicitly selects
  no-extension members. The normalized any-member predicate serves group rows, total, summary, and
  both cross-composed facet counts. All three cursor signatures distinguish absent, empty, and
  normalized non-empty extension values.
- Core and Infrastructure carry the field through the existing group, selected-root-facet, and
  drive-facet requests. The group, member, selected-root-facet, and drive-facet caches remain
  independently bounded to five pages with their existing cancellation sources, query
  generations, two-page directional prefetch limits, and stale-response rejection. No new channel
  or unbounded WPF collection is introduced.
- WPF adds a labeled extension box and a separate no-extension checkbox on a wrapping filter row.
  Automation names/help text state any-member behavior, the distinction from file type, and
  no-extension terminal-dot/dotfile behavior. Explicit tab order, system brushes, live regions,
  row virtualization, full-path help text, and bounded DataGridCell focus restoration remain
  intact.
- The slice reads only immutable run-owned SQLite state. It adds no preview, thumbnail, validation,
  filesystem read, Cloud Files access, durable decision, deletion, `scanned_file.marked_deleted`,
  or Milestone 10/11 behavior.

##### Acceptance result

- Focused extension/backfill storage tests and the comprehensive worker paging/cursor test passed.
  Three typed worker-lifecycle tests, 13 focused Core duplicate-file tests, and all three STA WPF
  surface tests passed. Coverage includes mixed member extensions, explicit no-extension,
  terminal-dot/dotfile/multiple-suffix rules, Unicode case with normalization-form preservation,
  group/summary/facet predicate agreement, invalid input, and all three cursor signatures.
- The expanded 100,000-group group/summary/selected-root-facet/drive-facet regression completed in
  2.39 seconds Debug and 1.05 seconds optimized Release, within its five-second gates.
  Representative-hardware warm-query and explicit bounded-memory profiling remain open.
- `cargo test --workspace` and `cargo test --workspace --release` passed with 18 storage and 9
  worker tests in each configuration. Explicit Debug and Release worker builds passed. The first
  full Debug attempt exposed that the minimal v3 migration fixture omitted `scanned_file`; bounded
  extension reconciliation now no-ops when result storage is absent, and the focused migration
  regression plus both complete final Rust runs passed.
- Debug and Release .NET builds from `C:\Windows\Temp` against the absolute solution path passed
  with zero warnings and errors under installed SDK 10.0.400; `global.json` remains unchanged.
  Each configuration passed 45 Core, 22 Infrastructure, and 3 WPF tests, with the real-provider
  Infrastructure test intentionally skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` passed worker group/summary/root-facet/
  drive-facet extension filtering, explicit no-extension filtering, accessible WPF interaction,
  and the accepted exact-path, next/previous-set focus, facet, Explorer, cloud-fail-closed, and
  shutdown workflows.
- Targeted Rust formatting, PowerShell parsing, and `git diff --check` passed. Repository-wide
  `cargo fmt --all -- --check` remains blocked only by the accepted pre-existing formatting drift
  in unchanged CLI/FFI files; those unrelated files were preserved.
- Remaining Milestone 8 gates are further richer worker-owned filters such as all-member extension,
  separately versioned file-type classification, or boundary-aware path modes once their contracts
  and indexes are complete; the complete accessibility review; and representative-hardware
  warm-query/bounded-memory profiling. Durable decisions remain Milestone 10 and deletion remains
  Milestone 11.

#### All-member filename-extension filter slice (2026-08-18)

##### Audit and scope decision

Explicit file-type classification remains broader than one safe slice because it requires a
versioned Rust-owned mapping, migration/reclassification rules, and product language that cannot be
confused with filename extension. Boundary-aware canonical prefix/descendant matching still needs
separator and device-prefix normalization, prefix-or-self versus descendant-only behavior,
Unicode-form and case rules, root-self handling, and stored range keys. Selected-root-relative
matching remains a distinct operation requiring an exact selected root and indexed root/relative
keys. The accepted case-insensitive member-path substring search remains unchanged and is not
relabeled as prefix matching.

All-member extension matching is the smallest coherent result-understanding entry point because it
reuses the accepted immutable `extension_key`, `idx_file_run_extension_key`, and indexed
group/member ownership. `any` remains the default and means at least one member has the requested
key. `all` means the count of immutable members with that key equals the group's persisted copy
count. The count equality both defines every-member behavior and prevents incomplete membership
state from matching vacuously. An empty key retains the accepted no-extension extraction rules, so
`all` with an empty extension means every immutable member has no extension.

##### Implemented contract and UX

- Group, selected-root-facet, and drive-facet filters add `extensionMatch`, accepting `any` or
  `all` and defaulting to `any`. When extension is absent or null, the mode contributes no predicate
  and normalizes to `any`; explicit empty extension remains the no-extension predicate.
- The shared normalized predicate serves group rows, total, review/location summary, selected-root
  facet counts, and drive facet counts. All three cursor signatures bind the normalized mode and
  reject cross-mode cursors. Any-member lookup retains the run/extension index; all-member lookup
  uses the persisted key and indexed group membership without filesystem access or representative
  inference.
- Core and Infrastructure carry the mode through the existing group and both facet requests. The
  group, member, selected-root-facet, and drive-facet caches remain independently bounded to five
  pages with their existing cancellation sources, query generations, two-page directional
  prefetch bounds, stale-response rejection, and bounded WPF collections.
- WPF adds an explicitly ordered `All copies must match` checkbox. Its automation name/help text
  explains every-immutable-member behavior, all-member no-extension behavior, and that filename
  extension remains distinct from file type. Clear filters restores the accepted any-member
  default. Existing live regions, system high-contrast brushes, full-path help text, virtualization,
  and bounded DataGridCell focus restoration remain unchanged.
- Schema version 4, immutable historical runs, Rust-only SQLite ownership, the pre-I/O exclusion
  boundary, and `scanned_file.marked_deleted` are unchanged. The slice adds no file-type mapping,
  path mode, filesystem or Cloud Files read, preview, thumbnail, validation, durable decision,
  deletion, or Milestone 10/11 behavior.

##### Acceptance result

- Focused extension-mode storage and worker cursor tests passed, including mixed-extension
  exclusion, positive all-extension and all-no-extension behavior, predicate/summary/facet
  agreement, invalid mode rejection, and group/selected-root-facet/drive-facet cursor separation.
  The three typed worker-lifecycle tests, the focused Core extension-mode test, and all three STA
  WPF surface tests passed after rebuilding the actual Debug worker executable.
- The expanded 100,000-group group/summary/selected-root-facet/drive-facet regression includes both
  any-member and all-member extension queries. Its latest complete workspace runs finished in 2.80
  seconds Debug and 1.20 seconds optimized Release, within the five-second gates.
  Representative-hardware warm-query and explicit bounded-memory profiling remain open.
- `cargo test --workspace` and `cargo test --workspace --release` passed with 18 storage and 10
  worker tests in each configuration. Explicit Debug and Release worker builds passed.
- Debug and Release .NET builds from `C:\Windows\Temp` against the absolute solution path passed
  with zero warnings and errors under installed SDK 10.0.400; the unavailable pinned 10.0.303 SDK
  and `global.json` were unchanged. Each configuration passed 45 Core, 22 Infrastructure, and 3 WPF
  tests, with the real-provider Infrastructure test intentionally skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` passed worker group/summary/root-facet/
  drive-facet any/all extension and no-extension filtering, accessible WPF interaction, and the
  accepted exact-path, next/previous-set focus, facet, Explorer, cloud-fail-closed, and shutdown
  workflows.
- Targeted Rust formatting, PowerShell parsing, and `git diff --check` passed. Repository-wide
  `cargo fmt --all -- --check` remains blocked only by the accepted pre-existing formatting drift
  in unchanged CLI/FFI files; those unrelated files were preserved.
- Remaining Milestone 8 gates are further richer worker-owned filters only after their mapping or
  boundary-aware range-key contracts and indexes are explicit; the complete accessibility review;
  and representative-hardware warm-query/bounded-memory profiling. Durable decisions remain
  Milestone 10 and deletion remains Milestone 11.

#### Minimum-width filter-reflow accessibility slice (2026-08-18)

##### Audit and scope decision

Explicit file-type classification is not a safe small follow-up because it still requires a
versioned Rust-owned mapping, migration/reclassification behavior, and product language that stays
distinct from filename extension. Boundary-aware canonical prefix-or-self/descendant-only
matching still requires finalized separator, segment, root-self, case, Unicode, and stored
range-key semantics. Selected-root-relative matching remains a separate mode requiring an exact
selected root and an indexed worker-owned root/relative key. The accepted case-insensitive member
path substring search remains unchanged and is not relabeled as prefix filtering.

The implementation audit found the accepted worker contract, shared normalized SQL predicate,
extension/path indexes, group/summary/facet cursor signatures, four independent five-page caches,
four cancellation/generation channels, two-page directional prefetch bounds, stale-response
rejection, and bounded virtualized WPF collections aligned. The smallest concrete remaining
accessibility defect was instead in layout: the primary filter row used fixed-width grid columns
whose combined width exceeded the duplicate-file workspace at the application's supported narrow
window size. Later extension, selected-root, and drive controls already reflowed.

##### Implemented interaction

- The duplicate-file heading and primary filters now use a wrapping layout. At a 620-DIP workspace
  width, the path and size editors, three fixed presets, and Apply action wrap to later rows and
  remain inside the surface instead of clipping horizontally.
- Document order now matches the accepted explicit keyboard order: path search/exact path,
  minimum size, 1 GB or larger, three or more copies, across drives, and Apply. Existing tab
  indexes, label access keys, automation IDs/names/help text, high-contrast system brushes,
  virtualization, live states, and next/previous-set focus restoration are unchanged.
- The slice changes no Core state, protocol field, SQL, schema/index, cursor, cache, cancellation,
  query generation, prefetch behavior, or collection bound. It reads no filesystem or Cloud Files
  content and adds no preview, thumbnail, validation, durable decision, deletion,
  `scanned_file.marked_deleted`, or Milestone 10/11 behavior.

##### Acceptance result

- Focused extension-mode storage and worker cursor tests passed, as did the pre-change 45-test Core
  suite and 3-test STA WPF suite. The new targeted STA regression then passed and proves that the
  primary filters wrap to a later row and remain within a 620-DIP workspace while the existing
  tab-order, automation, system-brush, virtualization, and focus assertions remain active.
- `cargo test --workspace` and `cargo test --workspace --release` passed with 18 storage and 10
  worker tests in each configuration. Explicit Debug and Release worker builds passed. The
  unchanged 100,000-group regression completed in 2.79 seconds Debug and 1.26 seconds optimized
  Release.
- Debug and Release .NET builds from `C:\Windows\Temp` against the absolute solution path passed
  with zero warnings and errors under installed SDK 10.0.400; the unavailable pinned 10.0.303 SDK
  and `global.json` were unchanged. Each serialized configuration passed 45 Core, 22
  Infrastructure, and 3 WPF tests, with the real-provider Infrastructure test intentionally
  skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` passed the accepted exact-path,
  any/all extension/no-extension, facets, summary/location, paging, next/previous-set focus,
  Explorer, cloud-fail-closed, and deterministic shutdown workflows. `git diff --check` passed.
- This slice does not claim the complete accessibility gate. Remaining Milestone 8 gates are
  further richer worker-owned filters only after their mapping or boundary-aware range-key
  contracts and indexes are explicit; the rest of the complete accessibility review, including
  broader screen-reader announcement and supported-size/DPI review; and representative-hardware
  warm-query/bounded-memory profiling.

#### Duplicate-file query screen-reader announcement slice (2026-08-18)

##### Audit and scope decision

Explicit file-type classification remains broader than a bounded follow-up because it requires a
versioned Rust-owned mapping, migration/reclassification behavior, and language that remains
distinct from filename extension. Canonical prefix-or-self and descendant-only matching still
require explicit separator/segment boundaries, root-self behavior, case and Unicode normalization,
and stored range keys. Selected-root-relative matching remains separate and requires an exact
selected root plus indexed worker-owned root/relative keys. The accepted exact canonical-path
equality and case-insensitive member-path substring modes remain unchanged and distinct.

The contract audit found the shared normalized Rust predicate, exact-path and extension indexes,
group/summary/cross-facet agreement, all three cursor signatures, four independent five-page
caches and cancellation/generation channels, two-page directional prefetch bounds, stale-response
rejection, and virtualized one-page WPF collections aligned. The concrete accessibility gap was
smaller: the view assigned live-region metadata but did not explicitly raise a UI Automation event
when an asynchronous duplicate-file group query settled, so screen readers had no reliable
completion or error announcement.

##### Implemented interaction

- Core publishes one concise completion message after the current group generation settles. It
  includes matching sets, copies, potential recoverable space, and aggregate location coverage; an
  empty query has an explicit no-matches message.
- Filter-validation and worker-query failures publish a separate error message. WPF raises
  `ActionCompleted` with `MostRecent` processing for success and `ActionAborted` with
  `ImportantMostRecent` processing for failure under one stable duplicate-file-query activity ID.
- A monotonic announcement version raises a new event even when two page or filter operations have
  identical text. The attached WPF behavior waits for data binding and a loaded automation peer,
  coalesces superseded versions, and retains the accepted polite/assertive live-region metadata.
- Only the current group generation can complete `IsLoading` and publish success or worker failure;
  cancelled or stale generations retain the accepted rejection behavior. This slice does not yet
  claim selected-set detail, facet-paging, or complete-workspace screen-reader announcements.
- The slice changes no protocol field, SQL predicate, schema/index, cursor, page size, cache,
  cancellation source, query generation, prefetch rule, virtualization setting, or collection
  bound. It performs no filesystem or Cloud Files read and adds no preview, thumbnail, validation,
  durable decision, deletion, `scanned_file.marked_deleted`, or Milestone 10/11 behavior.

##### Acceptance result

- Focused extension-mode storage and worker cursor-signature tests passed. The 13 focused Core
  duplicate-file tests and all 3 STA WPF surface tests passed; coverage proves concise success and
  validation-error messages, repeatable announcement versions, notification kind/processing/
  activity metadata, a loaded automation-peer event, the accepted 620-DIP reflow, tab/automation
  order, system brushes, virtualization, and focus restoration.
- `cargo test --workspace` and `cargo test --workspace --release` passed with 18 storage and 10
  worker tests in each configuration. Explicit Debug and Release worker builds passed. The
  unchanged 100,000-group regression completed in 3.02 seconds Debug and 1.44 seconds optimized
  Release, within its five-second regression gates.
- Debug and Release .NET builds from `C:\Windows\Temp` against the absolute solution path passed
  with zero warnings and errors under installed SDK 10.0.400; the unavailable pinned 10.0.303 SDK
  and `global.json` were unchanged. Each serialized configuration passed 45 Core, 22
  Infrastructure, and 3 WPF tests, with the real-provider Infrastructure test intentionally
  skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` passed the accepted exact-path,
  any/all extension/no-extension, facet, summary/location, paging, next/previous-set focus,
  Explorer, cloud-fail-closed, and deterministic shutdown workflows. `git diff --check` passed.
- This slice does not claim the complete accessibility gate. Remaining Milestone 8 gates are
  further richer worker-owned filters only after their mapping or boundary-aware range-key
  contracts and indexes are explicit; selected-set/facet and broader screen-reader review plus
  supported-size/DPI behavior; and representative-hardware warm-query/bounded-memory profiling.

#### Selected-set query screen-reader announcement slice (2026-08-19)

##### Audit and scope decision

No richer worker-owned filter has both a sufficiently small complete contract and its required
mapping or range indexes. Explicit file-type classification still requires a versioned Rust-owned
mapping plus migration/reclassification behavior and must remain distinct from filename
extension. Canonical prefix-or-self and descendant-only matching still require separator and
segment boundaries, root-self behavior, case and Unicode normalization, and stored range keys.
Selected-root-relative matching remains a separate operation requiring an exact selected root and
indexed root/relative keys. Accepted exact canonical-path equality and case-insensitive member-path
substring search remain unchanged and distinct.

The implementation audit reconfirmed the shared normalized Rust group/summary/cross-facet
predicate, exact-path and extension indexes, all three cursor signatures, four independent
five-page caches and cancellation/generation channels, two-page directional prefetch bounds,
stale-response rejection, and virtualized one-page WPF collections. The smallest concrete
accessibility gap was the selected-set detail channel: its member count, representative label,
location span, loading, empty, and error elements had live-region metadata, but an asynchronous
member-page completion or worker failure did not explicitly raise a UI Automation notification.

##### Implemented interaction

- Core publishes one concise completion message only after the displayed current member generation
  settles. It names the selected representative label, copy count, selected-root/drive span, and
  scan-time exact-content/representative-not-original explanation; an empty member result is
  explicit.
- WPF raises `ActionCompleted` with `MostRecent` processing for success and `ActionAborted` with
  `ImportantMostRecent` processing for worker-query failure under one stable
  `DuplicateFileMemberQuery` activity ID. Existing polite/assertive live-region metadata remains.
- Monotonic announcement versions make identical member pages repeatable. Displayed prefetched-cache
  pages announce, non-displayed prefetch remains silent, and the existing loaded-peer/data-binding
  behavior coalesces superseded versions.
- Only the current member generation can display rows, complete loading, or announce. Changing the
  selected group still cancels the previous request, clears the member cache, advances the member
  generation, and rejects late rows and announcements. Group, root-facet, and drive-facet channels
  retain their independent behavior.
- The slice changes no Rust, protocol field, SQL predicate, schema/index, cursor, page size, cache,
  cancellation source, query generation, prefetch rule, virtualization setting, or collection
  bound. It performs no filesystem or Cloud Files read and adds no preview, thumbnail, validation,
  durable decision, deletion, `scanned_file.marked_deleted`, or Milestone 10/11 behavior.

##### Acceptance result

- The 2 focused storage extension/backfill tests and focused worker cursor-signature test passed.
  All 14 focused DuplicateFiles Core tests and all 3 STA WPF surface tests passed. Coverage proves
  selected-set success wording, explicit empty detail, worker failure, monotonic repeat for a
  displayed prefetched-cache page, stale-generation silence, completion/error notification
  kind/processing/activity metadata, loaded automation-peer events, the accepted 620-DIP reflow,
  tab/automation order, system brushes, virtualization, and focus restoration.
- `cargo test --workspace` and `cargo test --workspace --release` passed with 18 storage and 10
  worker tests in each configuration. Explicit Debug and Release worker builds passed. The
  unchanged 100,000-group regression completed in 3.43 seconds Debug and 1.29 seconds optimized
  Release, within its five-second regression gates.
- Debug and Release .NET builds from `C:\Windows\Temp` against the absolute solution path passed
  with zero warnings and errors under installed SDK 10.0.400; the unavailable pinned 10.0.303 SDK
  and `global.json` were unchanged. Each final serialized configuration passed 46 Core, 22
  Infrastructure, and 3 WPF tests, with the real-provider Infrastructure test intentionally
  skipped. The first Debug Infrastructure pass had one unrelated recovery race because its scan
  completed before the worker kill; that test passed immediately in isolation and the complete
  serialized Debug rerun passed.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` passed the accepted exact-path,
  any/all extension/no-extension, facet, summary/location, paging, next/previous-set focus,
  Explorer, cloud-fail-closed, and deterministic shutdown workflows. The selected-set UI
  Automation notification itself is covered by the loaded-peer STA regression. `git diff --check`
  passed. Repository-wide `cargo fmt --all -- --check` remains blocked only by the accepted
  pre-existing formatting drift in unchanged CLI/FFI files; those files were preserved.
- This slice does not claim the complete accessibility or performance gates. Remaining Milestone 8
  work is further richer worker-owned filters only after their explicit mappings or boundary-aware
  range keys and indexes exist; facet paging/sort and broader screen-reader review plus supported
  size/DPI behavior; and representative-hardware warm-query/bounded-memory profiling.

#### Facet paging/sort screen-reader announcement slice (2026-08-19)

##### Audit and scope decision

No richer worker-owned filter has a sufficiently small complete contract plus the mapping or range
indexes needed to serve every normalized result path. Explicit file-type classification still
requires a versioned Rust-owned mapping, migration/reclassification behavior, and language distinct
from filename extension. Canonical prefix-or-self and descendant-only matching still require
separator/segment boundaries, root-self behavior, case and Unicode normalization, and stored
worker-owned range keys. Selected-root-relative matching remains a separate operation requiring an
exact selected root and indexed root/relative range keys. Accepted exact canonical-path equality,
canonical prefix/descendant concepts, selected-root-relative concepts, and case-insensitive literal
member-path substring search remain distinct; substring search is not relabeled as a prefix mode.

The implementation audit reconfirmed the shared normalized Rust group/summary/cross-facet
predicate, exact-path and extension indexes, all three cursor signatures, four independent
five-page caches and cancellation/generation channels, two-page directional prefetch bounds,
stale-response rejection, and one-page virtualized WPF collections. The supported-size audit also
reconfirmed the 900-by-600-DIP window minimum and accepted 620-DIP primary-filter reflow, but a
complete size/DPI gate requires multi-surface checks at supported scale factors and is broader than
one bounded change. The smallest concrete gap was therefore explicit screen-reader feedback after
a user pages or re-sorts either worker-owned facet.

##### Implemented interaction

- Explicit selected-root and drive facet paging/sort operations publish one concise completion
  message after the displayed current generation settles. The message names the facet, displayed
  count, total count, and effective sort; empty results are explicit.
- WPF raises `ActionCompleted` with `MostRecent` processing for success and `ActionAborted` with
  `ImportantMostRecent` processing for worker-query failure. Selected-root and drive channels use
  stable `DuplicateFileSelectedRootFacetQuery` and `DuplicateFileDriveFacetQuery` activity IDs.
- Monotonic per-channel announcement versions make identical cached pages repeatable. Displayed
  prefetched-cache pages announce, a successful cached page clears its prior channel error, and
  non-displayed prefetch plus initial/filter-driven facet refreshes remain silent to avoid stacking
  two facet notifications on every duplicate-file filter result.
- Only the current independent facet generation can display or announce. Superseded sort requests,
  cancellation, stale worker responses, and non-displayed prefetch cannot increment announcement
  versions. The accepted loaded-peer/data-binding behavior coalesces superseded versions.
- The slice changes no Rust, protocol field, SQL predicate, schema/index, cursor, page size, cache,
  cancellation source, query generation, prefetch bound, virtualization setting, tab order, focus
  behavior, or collection bound. It performs no filesystem or Cloud Files read and adds no preview,
  thumbnail, validation, durable decision, deletion, `scanned_file.marked_deleted`, or Milestone
  10/11 behavior.

##### Acceptance result

- The two focused extension/backfill storage tests and focused worker cursor-signature test passed.
  All 15 focused DuplicateFiles Core tests and all 3 STA WPF surface tests passed. Coverage proves
  worker-loaded empty sort results, repeatable displayed prefetched-cache pages, worker failures,
  explicit-sort stale-generation silence, non-displayed-prefetch silence, success/error notification
  kind/processing/activity metadata, loaded automation-peer events, accepted 620-DIP reflow,
  tab/automation order, system brushes, virtualization, and focus restoration.
- The unchanged 100,000-group regression passed focused runs in 4.64 seconds Debug and 1.31 seconds
  optimized Release, within its five-second gates. One initial complete Release workspace attempt
  ran under visible host contention: the unchanged exact-folder suite took 16.67 seconds and the
  scale test's extension queries tripped their five-second assertion at 6.75 seconds. The scale test
  passed immediately in isolation at 1.31 seconds and the complete Release workspace rerun passed,
  with its full storage suite completing in 3.24 seconds.
- `cargo test --workspace` and the final `cargo test --workspace --release` passed with 18 storage
  and 10 worker tests in each configuration. Explicit Debug and Release worker builds passed.
- Debug and Release .NET builds from `C:\Windows\Temp` against the absolute solution path passed
  with zero warnings and errors under installed SDK 10.0.400; the unavailable pinned 10.0.303 SDK
  and `global.json` were unchanged. Each final serialized configuration passed 47 Core, 22
  Infrastructure, and 3 WPF tests, with the real-provider Infrastructure test intentionally skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed the accepted exact-path,
  any/all extension/no-extension, facet, summary/location, paging, next/previous-set focus,
  Explorer, cloud-fail-closed, and deterministic shutdown workflows. The new facet UI Automation
  notifications are covered by the loaded-peer STA regression. `git diff --check` passed after the
  final documentation update. Repository-wide `cargo fmt --all -- --check` remains blocked only by
  accepted pre-existing formatting drift in unchanged CLI/FFI files; those files were preserved.
- This slice does not claim the complete accessibility or performance gates. Remaining Milestone 8
  work is further richer worker-owned filters only after their explicit mappings or boundary-aware
  range keys and indexes exist; broader screen-reader review and supported minimum-size/DPI
  behavior; and representative-hardware warm-query/bounded-memory profiling.

#### Session Setup minimum-width accessibility slice (2026-08-19)

##### Audit and scope decision

No richer worker-owned filter has both a sufficiently small complete contract and the mapping or
range indexes required across group rows, total, summary, both cross-facets, and all three cursor
signatures. Explicit file-type classification still requires a versioned Rust-owned mapping and
migration/reclassification behavior and remains distinct from filename extension. Canonical
prefix-or-self and descendant-only matching still require separator/segment boundaries, root-self
behavior, case and Unicode normalization, and stored range keys. Selected-root-relative matching
remains separate and requires an exact selected root plus indexed root/relative keys. Accepted
exact equality, boundary-aware path concepts, selected-root-relative concepts, and literal
case-insensitive member-path substring search remain distinct.

The implementation audit reconfirmed the accepted SQLite collation and exact-path/extension
indexes, shared normalized predicates, cursor signatures, four five-page caches, independent
cancellation/generations, two-page directional prefetch, stale-response rejection, bounded
virtualized result pages, and UI Automation notification behavior. The smallest concrete
accessibility defect was outside those accepted query channels: Session Setup placed 620-DIP
minimum-width multiline editors inside the supported 620-DIP workspace while also applying 28-DIP
side margins. The editors therefore extended beyond the right edge at the application's minimum
window width. A focused STA regression reproduced the overflow before implementation.

##### Implemented interaction

- The existing vertically scrollable Session Setup panel now stretches to its available viewport
  and explicitly disables a surface-level horizontal scrollbar. The name, root, cloud, exclusion,
  ignore-pattern, warning, validation, and action sections retain their order and behavior.
- Scan-root editors and the two multiline exclusion/pattern editors no longer impose widths larger
  than the narrow viewport. Long unwrapped exclusion paths and glob patterns retain explicit
  internal horizontal scrollbars, so fitting the surface does not truncate the editable value.
- The STA layout regression hosts Session Setup at a 620-DIP workspace, verifies both multiline
  editors remain within the right edge, and verifies their internal horizontal scrolling. The
  accepted duplicate-file 620-DIP reflow, tab/automation order, system brushes, virtualization,
  notification metadata, loaded peers, and focus restoration remain covered in the same suite.
- The slice changes no Core state, Rust, protocol field, SQL predicate, schema/index, cursor, page
  size, cache, cancellation source, query generation, prefetch rule, stale-response behavior,
  virtualization setting, or collection bound. It performs no filesystem or Cloud Files read and
  adds no preview, thumbnail, validation, durable decision, deletion,
  `scanned_file.marked_deleted`, or Milestone 10/11 behavior.

##### Acceptance result

- The new focused STA regression failed before the XAML change because the manual-exclusion editor
  extended past the supported narrow workspace and passed afterward. The two focused
  extension/backfill storage tests, focused worker cursor-signature test, all 15 DuplicateFiles
  Core tests, and all 3 STA WPF surface tests passed.
- The unchanged 100,000-group regression passed focused runs in 5.24 seconds Debug and 2.59 seconds
  optimized Release; every internal five-second bounded query assertion passed. In the complete
  workspace runs, the storage suite including that regression completed in 4.48 seconds Debug and
  1.77 seconds Release. These host runs do not replace the representative-hardware profiling gate.
- `cargo test --workspace` and `cargo test --workspace --release` passed with 18 storage and 10
  worker tests in each configuration. Explicit Debug and Release worker builds passed.
- Debug and Release .NET builds from `C:\Windows\Temp` against the absolute solution path passed
  with zero warnings and errors under installed SDK 10.0.400; the unavailable pinned 10.0.303 SDK
  and `global.json` were unchanged. Debug passed 47 Core, 22 Infrastructure, and 3 WPF tests. The
  first Release pass had the known active-run disposal race because the small fixture completed
  before disposal; the unchanged test passed immediately in isolation and the complete serialized
  Release rerun passed 47 Core, 22 Infrastructure, and 3 WPF tests. The one real-provider
  Infrastructure test remained intentionally skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed the accepted exact-path,
  any/all extension/no-extension, facet, summary/location, paging, next/previous-set focus,
  Explorer, cloud-fail-closed, and deterministic shutdown workflows.
- `git diff --check` passed. Repository-wide `cargo fmt --all -- --check` remains blocked only by
  the accepted pre-existing formatting drift in unchanged CLI/FFI files; those files were
  preserved.
- This slice closes one supported-minimum-width defect but does not claim the complete
  accessibility or performance gates. Remaining Milestone 8 work is further richer worker-owned
  filters only after their explicit mappings or boundary-aware range keys and indexes exist; the
  rest of the broader screen-reader and supported minimum-size/multi-DPI review; and
  representative-hardware warm-query/bounded-memory profiling.

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
| 2026-08-17 | Refined and accepted the worker-owned Milestone 8 across-drives filter after focused/optimized coverage, the Debug/Release Rust and .NET matrix, and real Debug/Release WPF smoke passed. | Add the smallest bounded location-based entry point through the existing group predicate, cursor, summary, cache, cancellation, and query generation while leaving facets, aggregate location summaries, decisions, validation, and deletion as later gates. |
| 2026-08-17 | Refined and accepted the worker-owned Milestone 8 aggregate location summary after focused/optimized coverage, the Debug/Release Rust and .NET matrix, and real Debug/Release WPF smoke passed. | Show selected-root, drive, and cross-drive-set coverage through the existing bounded group summary and stale-response path while leaving paged facets, keyboard focus restoration, complete accessibility review, representative profiling, decisions, validation, and deletion as later gates. |
| 2026-08-17 | Refined and accepted the worker-owned paged selected-root facet and exact-root filter after focused/optimized coverage, the Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed. | Add the first bounded facet entry point with an explicit cursor signature, five-page Core cache, independent cancellation/generation, stale-response rejection, and keyboard-accessible WPF interaction while leaving the drive facet, richer filters, focus restoration, complete accessibility review, representative profiling, decisions, validation, and deletion as later gates. |
| 2026-08-17 | Refined and accepted the worker-owned paged drive facet and exact-drive filter after focused/optimized coverage, the Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed. | Complete the bounded location-facet workflow with cross-facet composition, an explicit drive cursor signature, separate five-page Core cache, independent cancellation/generation, stale-response rejection, and keyboard-accessible WPF interaction while leaving richer filters, focus restoration, complete accessibility review, representative profiling, decisions, validation, and deletion as later gates. |
| 2026-08-18 | Refined and accepted the worker-owned minimum-copy-count filter after focused/optimized coverage, the Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed; the latest 100,000-group regression completed in 2.21 seconds Debug and 1.39 seconds optimized Release, and each .NET configuration passed 39 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally gated. | Add the smallest indexed `Three or more copies` entry point through the normalized group rows/total/summary and both cross-composed facet predicates/signatures while preserving existing bounded caches, independent facet cancellation/generations, stale-response rejection, and the Milestone 8/10/11 scope boundaries; remaining Milestone 8 gates are additional richer worker-owned filters, next/previous-set keyboard focus restoration, the complete accessibility review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-18 | Refined and accepted the precise worker-owned one-copy-size entry point after focused/optimized coverage, the Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed; the expanded 100,000-group regression completed in 2.24 seconds Debug and 0.88 seconds optimized Release, and each .NET configuration passed 40 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally gated. | Add the smallest indexed `1 GB or larger` result-understanding entry point by normalizing the existing `minimumSize` predicate to at least 1,073,741,824 bytes across group rows/total/summary and both facet predicates/signatures, while deferring extension/type and path-prefix filters until member-accurate semantics and indexes are explicit and preserving bounded caches, independent cancellation/generations, stale-response rejection, and the Milestone 8/10/11 boundaries. |
| 2026-08-18 | Refined and accepted bounded next/previous-set navigation with virtualized-row keyboard focus restoration after focused Core/WPF tests, Debug/Release 100,000-group coverage, the full Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed; the complete workspace regression completed in 2.04 seconds Debug and 0.88 seconds Release, and each .NET configuration passed 42 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | Close the focus-restoration gate through the existing group cursor/cache/cancellation/generation path and independent member stale-response guard without changing protocol, SQL, indexes, facets, or memory bounds; extension/type remains deferred until any-member/all-member normalization and indexed worker ownership are explicit, and true path filtering remains deferred until exact/prefix/descendant, segment-boundary, case, and selected-root-relative semantics are explicit. |
| 2026-08-18 | Refined and accepted the exact canonical-member-path entry point after focused exact-path/lifecycle/Core/WPF coverage, the full Debug/Release Rust and serialized .NET matrix, and corrected real Debug/Release WPF smoke passed; the expanded 100,000-group regression completed in 2.41 seconds Debug and 1.03 seconds optimized Release, and each .NET configuration passed 44 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | Add indexed, Unicode-case-normalized any-member exact equality through the shared group rows/total/summary predicate, all three cursor signatures, and both cross-composed facet counts without changing the five-page cache, cancellation, generation, stale-response, or WPF collection bounds. Extension/type remains deferred until explicit any-member/all-member/no-extension semantics and indexed normalized member keys exist; canonical prefix/descendant and selected-root-relative modes remain deferred until separator-boundary semantics and stored range-query keys exist. The remaining Milestone 8 gates are further richer worker-owned filters, the complete accessibility review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-18 | Refined and accepted indexed any-member filename-extension and explicit no-extension filtering after focused storage/worker/lifecycle/Core/WPF coverage, the full Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed; the expanded 100,000-group regression completed in 2.39 seconds Debug and 1.05 seconds optimized Release, and each .NET configuration passed 45 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | Define extension from every immutable member's final persisted filename segment, including terminal-dot, dotfile, multiple-suffix, Unicode-lowercase, normalization-form-preserving, and explicit no-extension rules; distinguish it from file-type classification; and serve group rows/total/summary plus both cross-facets and all three signatures from a Rust-owned backfilled key/index while preserving the four independent five-page caches, cancellation/generation/prefetch/stale-response bounds, and Milestone 8/10/11 boundaries. Canonical prefix/descendant and selected-root-relative modes remain deferred pending stored boundary-aware range keys; remaining gates are further explicitly indexed filters, the complete accessibility review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-18 | Refined and accepted opt-in all-member filename-extension and all-member no-extension filtering after focused storage/worker/lifecycle/Core/WPF coverage, the full Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed; the expanded 100,000-group regression completed in 2.80 seconds Debug and 1.20 seconds optimized Release, Rust passed 18 storage and 10 worker tests, and each .NET configuration passed 45 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | Keep accepted any-member behavior as the default while defining `all` as matching-member count equal to persisted copy count across group rows/total/summary, both cross-facets, and all three cursor signatures; reuse the Rust-owned extension key and indexed membership path without changing the four five-page caches, independent cancellation/generations, two-page directional prefetch, stale-response rejection, or Milestone 8/10/11 boundaries. Versioned file type and boundary-aware canonical/selected-root-relative path modes remain deferred pending their separate mappings or stored range keys; remaining gates are further explicitly indexed filters, the complete accessibility review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-18 | Refined and accepted the first bounded Milestone 8 accessibility-remediation slice after a focused 620-DIP STA layout regression, the full Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed; the unchanged 100,000-group regression completed in 2.79 seconds Debug and 1.26 seconds optimized Release, and each .NET configuration passed 45 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | No richer filter had both a sufficiently small contract and its required worker-owned mapping/range indexes. Make the fixed primary duplicate-file filter row reflow within the supported narrow workspace while preserving explicit tab/automation order and every accepted protocol, SQL, cursor, cache, cancellation, generation, prefetch, stale-response, virtualization, and Milestone 8/10/11 boundary; the remaining gates are further explicitly indexed filters, the rest of the complete accessibility review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-18 | Refined and accepted the bounded duplicate-file query screen-reader announcement slice after focused storage/worker/Core/STA coverage, the full Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed; the unchanged 100,000-group regression completed in 3.02 seconds Debug and 1.44 seconds optimized Release, and each .NET configuration passed 45 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | No remaining richer filter has both a small complete contract and its required versioned mapping or boundary-aware range indexes. Raise one coalesced repeatable `ActionCompleted` notification after the current duplicate-file group generation settles and an `ActionAborted` notification for validation/worker failures without changing accepted protocol, SQL/indexes, cursors, caches, cancellation/generations, stale-response rejection, prefetch, virtualization, or Milestone 8/10/11 boundaries; remaining gates are further explicitly indexed filters, selected-set/facet and broader screen-reader plus supported-size/DPI review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-19 | Refined and accepted the bounded selected-set member-query screen-reader announcement slice after focused storage/worker/Core/STA coverage, the full Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed; the unchanged 100,000-group regression completed in 3.43 seconds Debug and 1.29 seconds optimized Release, and each final .NET configuration passed 46 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | No richer filter has both a small complete contract and its required versioned mapping or boundary-aware range indexes. Raise repeatable coalesced `ActionCompleted` notifications for displayed current-generation member pages, including bounded prefetched-cache pages, and `ActionAborted` for current worker failures while non-displayed prefetch and stale generations remain silent; accepted protocol, SQL/indexes, cursors, four five-page caches, cancellation/generations, prefetch, virtualization, focus, and Milestone 8/10/11 boundaries remain unchanged. Remaining gates are further explicitly indexed filters, facet and broader screen-reader plus supported-size/DPI review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-19 | Refined and accepted the bounded selected-root/drive facet paging-and-sort screen-reader announcement slice after focused storage/worker/Core/STA coverage, the final Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed; the unchanged 100,000-group regression passed focused runs in 4.64 seconds Debug and 1.31 seconds optimized Release, and each final .NET configuration passed 47 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | No richer filter has both a small complete contract and its required versioned mapping or boundary-aware range indexes. Raise repeatable coalesced `ActionCompleted` notifications only for explicitly displayed facet paging/sort pages and `ActionAborted` for their current worker failures while filter-driven refresh, non-displayed prefetch, cancellation, and stale generations remain silent; accepted protocol, SQL/indexes, cursors, four five-page caches, independent cancellation/generations, prefetch, virtualization, focus, and Milestone 8/10/11 boundaries remain unchanged. Remaining gates are further explicitly indexed filters, broader screen-reader and supported minimum-size/DPI review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-19 | Refined and accepted the bounded Session Setup minimum-width accessibility slice after reproducing the overflow in a focused 620-DIP STA regression, then passing focused storage/worker/Core/STA coverage, the full Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke; the complete storage suites finished in 4.48 seconds Debug and 1.77 seconds Release, and each final .NET configuration passed 47 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | No richer filter has both a small complete contract and its required versioned mapping or boundary-aware range indexes. Stretch the existing setup scroller to its narrow viewport, remove oversized root/exclusion/pattern editor minimums, and retain internal scrolling for long unwrapped values without changing accepted protocol, SQL/indexes, cursors, caches, cancellation/generations, prefetch, stale-response handling, virtualization, announcements, or Milestone 8/10/11 boundaries. Remaining gates are further explicitly indexed filters, the rest of the broader screen-reader and supported minimum-size/multi-DPI review, and representative-hardware warm-query/bounded-memory profiling. |
