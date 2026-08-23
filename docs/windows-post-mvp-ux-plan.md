# Windows Post-MVP Duplicate Review UX Plan

## Status

Active implementation roadmap for the Windows duplicate-review experience. Milestone 6
release-acceptance remediation and the fail-closed Milestone 7 slice are complete. The first twelve
read-only Milestone 8 slices and the first eight bounded accessibility-remediation slices are
implemented and accepted; the broader milestone remains in progress and is gated by the remaining
criteria below. The first four Milestone 10 slices are accepted: durable manual file decisions,
durable manual exact-folder decisions, the read-only ordered preferred scan-root rule preview, and
bounded ordered-rule application/reversal provenance. The first bounded Milestone 11 preflight
slice is also accepted. The second Milestone 11 slice now has a refined revision-bound Recycle Bin
operation design, its strictly non-mutating foundation, and a separately gated real Windows
executor. Schema v10, bounded protocol/Core contracts, restart evidence, read-only WPF
reconstruction, and explicit disposable Shell acceptance exist. Production still injects the
disabled executor and exposes no action; provider/physical/performance gates, recovery resolution,
and Milestone 12 changed/resolved working-state mutation remain unimplemented or unaccepted. A
read-only query-plan stabilization slice reduced the duplicate-group warm baseline, but
representative-hardware acceptance remains open because this development host still shows
simultaneous cross-query tail-latency spikes.

This is the durable planning source for post-MVP Windows UX work. Update this document when a
milestone is refined, split, accepted, or superseded so future coding sessions do not have to
reconstruct product and architecture decisions from conversation history.

## Completed prerequisite

The release blockers in
[`windows-release-acceptance-remediation-plan.md`](windows-release-acceptance-remediation-plan.md)
were fixed and the full acceptance gate was closed by commit `6f1c405`. The historical remediation
scope and acceptance criteria remain authoritative for the MVP but no longer gate post-MVP work.

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
The sixth bounded accessibility-remediation slice raises explicit, repeatable UI Automation
notifications when the exact-duplicate-folder group query completes or fails and fixes the shared
notification behavior so non-control status elements receive an automation peer.
The seventh bounded accessibility-remediation slice raises explicit, repeatable UI Automation
notifications when the displayed exact-duplicate-folder member query completes or fails while
keeping non-displayed prefetch and Explorer-action errors silent.
The eighth bounded accessibility-remediation slice moves the exact-folder heading and filters into
a wrapping narrow-workspace layout while preserving their existing automation and query behavior.

This read-only milestone can close on its own acceptance criteria without adding mutable review or
live filesystem state. Durable `Keep`, `Remove`, and `Undecided` decisions belong to Milestone 10;
validation and live-state behavior belong to Milestone 12. Additional rich filters are optional
future Milestone 8 work only after their complete semantics, migration/backfill behavior, indexes,
cursor signatures, summary/facet integration, and performance bounds are designed; they are not a
reason to blur those milestone boundaries.

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

#### Exact-folder group-query screen-reader announcement slice (2026-08-19)

##### Audit and scope decision

No richer worker-owned filter has both a sufficiently small complete contract and the mapping or
range indexes required across group rows, total, summary, both cross-facets, and all three cursor
signatures. File-type classification still requires a versioned Rust-owned mapping plus explicit
migration/reclassification behavior and remains distinct from filename extension. Canonical
prefix-or-self and descendant-only matching still require separator/segment boundaries, root-self
behavior, case and Unicode normalization, and stored range keys. Selected-root-relative matching
remains separate and requires an exact selected root plus indexed root/relative keys. Accepted
exact equality and case-insensitive literal member-path substring search remain unchanged and are
not relabeled as boundary-aware path filtering.

The storage, protocol, cursor, paging, cache, cancellation, generation, prefetch, stale-response,
virtualization, supported-width, and performance audit found no smaller complete data-contract
slice. The broader screen-reader review did find a smaller concrete defect than the next
multi-surface DPI change: exact-duplicate-folder group filtering, paging, and sorting settled
asynchronously without an explicit completion or failure notification. Focused loaded-peer testing
also found that WPF `Border` status elements do not create a default automation peer, so the shared
notification behavior silently skipped the already accepted duplicate-file group-error element.
That is a regression in the accepted error-announcement gate and is corrected by the same bounded
accessibility slice.

##### Implemented interaction

- Core publishes one concise completion message after a displayed current-generation
  exact-duplicate-folder group page settles, including an explicit no-matches state. Identical
  results increment a monotonic version so repeated filter/page operations remain announceable.
- WPF raises `ActionCompleted` with `MostRecent` processing for success and `ActionAborted` with
  `ImportantMostRecent` processing for validation or worker failure under the stable
  `DuplicateFolderGroupQuery` activity ID. Displayed prefetched-cache pages announce; non-displayed
  prefetch, cancellation, and stale generations remain silent.
- The shared notification behavior now creates a generic `FrameworkElementAutomationPeer` only
  when a status element has no existing or control-specific peer. This makes both the accepted
  duplicate-file group-error `Border` and the new exact-folder group-error `Border` raise their
  assertive notification without changing control semantics.
- The slice changes no Rust, protocol field, SQL predicate, schema/index, cursor, page size, cache,
  cancellation source, query generation, prefetch bound, stale-response rule, virtualization
  setting, focus behavior, or collection bound. It performs no filesystem or Cloud Files read and
  adds no preview, thumbnail, validation, durable decision, deletion,
  `scanned_file.marked_deleted`, or Milestone 10/11 behavior.

##### Acceptance result

- Both focused extension/backfill storage tests and the focused worker cursor-signature test
  passed. All 5 exact-folder Core tests and all 3 STA WPF surface tests passed. Coverage proves
  completion wording, explicit empty results, repeatable versions, validation and worker failures,
  stale-generation silence, notification kind/processing/activity metadata, and loaded-peer events
  for both duplicate-file and exact-folder group-error `Border` elements. The accepted 620-DIP
  filter and Session Setup regressions, tab/automation order, system brushes, virtualization, and
  focus restoration remain covered by the same STA suite.
- The unchanged 100,000-group regression passed focused runs in 5.20 seconds Debug and 4.97 seconds
  optimized Release, with every internal five-second query assertion passing. During the first
  complete Release workspace run, visible host contention made the combined unchanged any/all
  extension queries take 10.51 seconds and trip their five-second assertion. The regression passed
  immediately in isolation in 3.19 seconds, and the complete serialized Release workspace rerun
  passed. The complete storage suites finished in 20.01 seconds Debug and 18.87 seconds on the final
  Release pass on this contended host. These runs do not replace representative-hardware profiling.
- `cargo test --workspace` and the final `cargo test --workspace --release` passed with 18 storage
  and 10 worker tests in each configuration. Explicit Debug and Release worker builds passed.
- Debug and Release .NET builds from `C:\Windows\Temp` against the absolute solution path passed
  with zero warnings and errors under installed SDK 10.0.400; the unavailable pinned 10.0.303 SDK
  and `global.json` were unchanged. Each serialized configuration passed 48 Core, 22
  Infrastructure, and 3 WPF tests, with the real-provider Infrastructure test intentionally
  skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed the accepted exact-path,
  any/all extension/no-extension, facet, summary/location, paging, next/previous-set focus,
  Explorer, cloud-fail-closed, and deterministic shutdown workflows. The new notification events
  are covered by the loaded-peer STA regression.
- `git diff --check` passed. Repository-wide `cargo fmt --all -- --check` remains blocked only by
  the accepted pre-existing formatting drift in unchanged CLI/FFI files; those files were
  preserved, and this accessibility-only slice changes no Rust source.
- This slice does not claim the complete accessibility or performance gates. Remaining Milestone 8
  work is further richer worker-owned filters only after explicit mappings or boundary-aware range
  keys and indexes exist; exact-folder member-query and broader screen-reader review plus the
  supported minimum-size/multi-DPI audit; and representative-hardware warm-query/bounded-memory
  profiling.

#### Exact-folder member-query screen-reader announcement slice (2026-08-19)

##### Audit and scope decision

No richer worker-owned filter has both a sufficiently small complete contract and the mapping or
range indexes required across group rows, total, summary, both cross-facets, and all three cursor
signatures. File-type classification still requires a versioned Rust-owned mapping plus explicit
migration/reclassification behavior and remains distinct from filename extension. Canonical
prefix-or-self and descendant-only matching still require separator/segment boundaries, root-self
behavior, case and Unicode normalization, and stored range keys. Selected-root-relative matching
remains separate and requires an exact selected root plus indexed root/relative keys. Accepted
exact equality and case-insensitive literal member-path substring search remain unchanged and are
not relabeled as boundary-aware path filtering.

The contract, SQL/index, cursor, paging, cache, cancellation, generation, prefetch,
stale-response, virtualization, performance, supported-width, and multi-surface DPI audit found no
smaller complete data or layout slice. The broader screen-reader review found the next concrete
defect in the exact-folder member channel: selecting a folder group or displaying another bounded
member page settled asynchronously without an explicit completion or worker-failure notification.
This is smaller than a complete multi-surface DPI change and closes the exact-folder member-query
gap named by the previous slice while leaving the broader accessibility gate open.

##### Implemented interaction

- Core publishes one concise completion message after a displayed current-generation exact-folder
  member page settles. It identifies the selected representative path and folder-copy count and
  includes an explicit no-copies state. Identical displayed results increment a monotonic version,
  so a displayed prefetched-cache page remains announceable.
- WPF raises `ActionCompleted` with `MostRecent` processing for success and `ActionAborted` with
  `ImportantMostRecent` processing for current worker failure under the stable
  `DuplicateFolderMemberQuery` activity ID. The detail error uses the system control-text brush so
  the new assertive status does not retain a hard-coded foreground under high contrast.
- Non-displayed prefetch, cancellation, and stale generations remain silent. An Explorer or
  clipboard action can continue to show its existing detail error, but does not increment the
  member-query failure version. Displaying a valid cached member page clears such an action error
  before raising the query-completion notification.
- The slice changes no Rust, protocol field, SQL predicate, schema/index, cursor, page size,
  five-page cache, cancellation source, query generation, two-page directional prefetch bound,
  stale-response rule, virtualization setting, focus behavior, or collection bound. It performs no
  new filesystem or Cloud Files read and adds no preview, thumbnail, validation, durable decision,
  deletion, `scanned_file.marked_deleted`, or Milestone 10/11 behavior.

##### Acceptance result

- Both focused extension/backfill storage tests and the focused worker cursor-signature test
  passed. All 7 exact-folder Core tests passed, including repeatable cached-page announcements,
  explicit empty results, worker failures, separation from Explorer-action errors, and
  stale-generation silence. The focused loaded-peer STA test passed success/error kind,
  processing, activity metadata, system-brush use, and real loaded-peer notification events for
  both exact-folder member status elements.
- The unchanged 100,000-group regression passed focused runs in 2.84 seconds Debug and 1.34 seconds
  optimized Release, with every internal five-second query assertion passing. The complete storage
  suites finished in 3.14 seconds Debug and 1.47 seconds Release on this host. These runs do not
  replace representative-hardware warm-query or bounded-memory profiling.
- `cargo test --workspace -- --test-threads=1` and
  `cargo test --workspace --release -- --test-threads=1` passed, each including 18 storage and 10
  worker tests. Explicit Debug and Release worker builds passed.
- Debug and Release .NET builds from `C:\Windows\Temp` against the absolute solution path passed
  with zero warnings and errors under installed SDK 10.0.400; the unavailable pinned 10.0.303 SDK
  and `global.json` were unchanged. Each serialized configuration passed 50 Core, 22
  Infrastructure, and 3 WPF tests, with the real-provider Infrastructure test intentionally
  skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed the accepted exact-path,
  any/all extension/no-extension, facet, summary/location, paging, next/previous-set focus,
  Explorer, cloud-fail-closed, and deterministic shutdown workflows. The new exact-folder member
  notification events are covered by the loaded-peer STA regression.
- `git diff --check` passed. Repository-wide `cargo fmt --all -- --check` remains blocked only by
  the accepted pre-existing formatting drift in unchanged CLI/FFI files; those files were
  preserved, and this accessibility-only slice changes no Rust source.
- This slice does not claim the complete accessibility or performance gates. Remaining Milestone 8
  work is further richer worker-owned filters only after explicit mappings or boundary-aware range
  keys and indexes exist; the rest of the broader screen-reader and supported
  minimum-size/multi-DPI review; and representative-hardware warm-query/bounded-memory profiling.

#### Exact-folder minimum-width filter-reflow accessibility slice (2026-08-19)

##### Audit and scope decision

No richer worker-owned filter has both a sufficiently small complete contract and the mapping or
range indexes required across group rows, total, summary, both cross-facets, and all three cursor
signatures. File-type classification still requires a versioned Rust-owned mapping plus explicit
migration/reclassification behavior and remains distinct from filename extension. Canonical
prefix-or-self and descendant-only matching still require separator/segment boundaries, root-self
behavior, case and Unicode normalization, and stored range keys. Selected-root-relative matching
remains separate and requires an exact selected root plus indexed root/relative keys. Accepted
exact equality and case-insensitive literal member-path substring search remain unchanged and are
not relabeled as boundary-aware path filtering.

The contract and implementation audit reconfirmed the accepted Rust-owned predicates and indexes,
cursor signatures, bounded paging and five-page caches, independent cancellation and generations,
two-page directional prefetch, stale-response rejection, performance bounds, virtualization,
focus restoration, and query-announcement behavior. The broader screen-reader review found no
smaller concrete regression outside the accepted announcement gates. The supported-width audit did
find a bounded layout defect: the exact-folder heading and two fixed-width filters plus Apply action
shared one non-wrapping row, so that surface could exceed the 620-DIP workspace left by the
application's 900-DIP minimum window. A focused STA regression reproduced the defect before the
XAML change.

##### Implemented interaction

- The exact-folder heading and explanatory text now occupy their own row, the explanation wraps,
  and the existing path, minimum-size, and Apply controls use a wrapping panel below it. The panel
  has a stable accessible name, and the controls retain their existing automation IDs/names,
  bindings, commands, tooltips, document order, and server-owned filter behavior.
- The focused STA regression hosts the exact-folder surface at 620 DIPs, requires the filters to
  reflow below the heading, and verifies the path, minimum-size, and Apply controls remain inside
  the workspace. WPF layout remains DIP-based across scale factors; this regression closes the
  concrete minimum-width defect without claiming the broader physical multi-monitor/DPI gate.
- The slice changes no Core state, Rust, protocol field, SQL predicate, schema/index, cursor,
  page size, five-page cache, cancellation source, query generation, two-page prefetch bound,
  stale-response rule, virtualization setting, focus behavior, or collection bound. It performs no
  filesystem or Cloud Files read and adds no preview, thumbnail, validation, durable decision,
  deletion, `scanned_file.marked_deleted`, or Milestone 10/11 behavior.

##### Acceptance result

- The focused 620-DIP STA regression failed against the old fixed-row layout and passed after the
  reflow. Both focused extension/backfill storage tests, the focused worker cursor-signature test,
  all 7 DuplicateFolders Core tests, and all 3 WPF STA tests passed.
- The unchanged 100,000-group regression completed in 3.03 seconds Debug and 1.20 seconds optimized
  Release, with every internal five-second assertion passing. The complete storage suites finished
  in 4.94 seconds Debug and 2.79 seconds Release on this host. These runs do not replace
  representative-hardware warm-query or bounded-memory profiling.
- Serialized `cargo test --workspace -- --test-threads=1` and
  `cargo test --workspace --release -- --test-threads=1` passed, each including 18 storage and 10
  worker tests. Explicit Debug and Release worker builds passed.
- Debug and Release .NET builds from `C:\Windows\Temp` against the absolute solution path passed
  with zero warnings and errors under installed SDK 10.0.400; the unavailable pinned 10.0.303 SDK
  and `global.json` were unchanged. Each serialized configuration passed 50 Core, 22
  Infrastructure, and 3 WPF tests, with the real-provider Infrastructure test intentionally
  skipped.
- Real Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed the accepted exact-path,
  any/all extension/no-extension, facet, summary/location, paging, next/previous-set focus,
  exact-folder, Explorer, cloud-fail-closed, and deterministic shutdown workflows.
- `git diff --check` passed. Repository-wide `cargo fmt --all -- --check` remains blocked only by
  the accepted pre-existing formatting drift in unchanged CLI/FFI files; those files were
  preserved, and this accessibility-only slice changes no Rust source.
- This slice does not claim the complete accessibility or performance gates. Remaining Milestone 8
  work is further richer worker-owned filters only after explicit mappings or boundary-aware range
  keys and indexes exist; the rest of the broader screen-reader and supported
  minimum-size/multi-DPI review; and representative-hardware warm-query/bounded-memory profiling.

#### Read-only milestone closure audit (2026-08-19)

##### Automated performance and memory profile

- `representative_review_workspace_profile` now reuses the 100,000-group fixture and is ignored by
  default so ordinary Debug/Release suites do not turn a hardware target into a CI timing test. The
  explicit Release profile warms the query path, samples 100 group/summary, selected-root-facet,
  and drive-facet first pages, enforces a 100 ms p95 for each, and requires private-memory growth
  below 32 MiB across the 300 bounded queries. Run it with:

  ```powershell
  cargo test -p super-duper-core --release --test storage_tests representative_review_workspace_profile -- --ignored --exact --nocapture --test-threads=1
  ```

- This host is Windows build 22631.6199 with an Intel Core i5-9600K, 6 logical processors,
  31.9 GiB RAM, and a 96-DPI interactive desktop. Its stable 100-sample run measured a 71.63 ms
  group/summary median, 140.47 ms group/summary p95, 59.68 ms selected-root-facet p95, 37.48 ms
  drive-facet p95, and 815,104 bytes of private-memory growth. The facet and 32 MiB memory bounds
  pass; the group/summary p95 does not meet the 100 ms target, so the warm-query gate remains open.
- The measured sub-megabyte growth complements the existing five-page group/member/facet cache
  assertions, two-page directional prefetch bounds, 200-row visible collections, and recycling
  virtualization checks. The read-only bounded-memory gate is closed for the current page/cache
  design; future Milestone 10 plan caches require their own measurements.
- To close the warm-query gate, run the command above from an already-built Release tree on the
  designated representative Windows 11 x64 machine under normal background load. Preserve the
  printed 100-sample metrics and require all three p95 values below 100 ms without a retry-only
  pass. If group/summary still fails, optimize the existing normalized summary predicate and query
  plan with `EXPLAIN QUERY PLAN` evidence; do not weaken the target or omit summary/location data.

##### Warm-query query-plan stabilization (2026-08-20)

- The group summary no longer performs one correlated across-drive member probe for every matching
  duplicate group. It now streams the indexed member relation once, applies the exact same shared
  filter predicate, groups by duplicate-set identity, and counts only groups with more than one
  distinct non-empty drive. When `Across drives` is itself the active filter, the matching-group
  total remains the exact across-drive count.
- Non-name sorts now select the bounded keyset candidate page before calculating representative
  name and distinct selected-root/drive detail. Representative-name sorting retains its complete
  member-derived sort before the bound. Focused regressions page every sort through ties and compare
  the concatenated keyset pages with the complete stable ordering; existing run/filter/summary,
  exact-path, extension, facet, forward/backward, and 100,000-group assertions remain unchanged.
- On the same 6-logical-processor development host, the pre-change session baseline was 75.45 ms
  p50, 136.64 ms p95, and 176.07 ms p99 for group/summary. Two final optimized 100-sample runs
  passed at 54.77/62.11/116.70 ms and 55.22/93.01/199.87 ms p50/p95/p99, with 716,800 and 610,304
  bytes of retained private growth. A third final run measured 55.11/198.72/283.01 ms and failed;
  selected-root and drive facets simultaneously rose to 79.93 and 122.30 ms p95, versus roughly
  31 ms in the first final run. This is evidence of reduced stable query cost plus unresolved
  host-wide tail contention, not a retry-only acceptance.
- The 100 ms representative-hardware gate therefore remains open. Close it only with a qualifying
  normal-load run on the designated machine; retain failures and p99 diagnostics. This slice does
  not change cache/page bounds, protocol, storage schema, filesystem access, review decisions,
  preflight, Recycle Bin execution, or any Milestone 11 production wiring.

##### Retained warm-query contention diagnostics (2026-08-20)

- The existing ignored Release profile now emits an optional schema-v1 diagnostic document through
  the acceptance collector. It retains all 500 ordered query intervals, p50/p75/p90/p95/p99/max
  per category, and 101 test-process snapshots with cumulative CPU, private/working-set memory,
  and operation/transfer I/O counters. The ordinary test suite still does not run the hardware
  profile.
- Acceptance evidence schema v2 adds a time-aligned host JSONL sidecar with persistent native CPU,
  memory, paging, disk, queue, context-switch, process/thread, and competing-process CPU/memory/I/O
  samples. The sampler PID is explicit. A new or empty directory is mandatory, so passing, failing,
  partial, and unavailable-counter runs cannot overwrite earlier evidence.
- The retained instrumented development-host run failed the unchanged target at
  52.68/54.76/94.74/140.76/243.87/728.16 ms group p50/p75/p90/p95/p99/max. Other p95 values were
  63.42 ms selected-root, 69.71 ms drive, 68.81 ms review-plan, and 5.19 ms review-groups; private
  growth was 880,640 bytes. The deterministic executor contract passed 31/31 before the profile.
  All structured diagnostics and the failed matrix were written before the collector returned the
  profile's exit code 101.
- Three coarse samples from the initial host sampler overlapped that 17.44-second query window and
  showed unrelated backup activity around 54-66 MB/s and up to 60% of one logical processor; one
  sample recorded a processor queue of 10. This distinguishes a lower stable-cost body from
  concurrent host pressure without claiming that the pressure caused every tail. The initial
  formatted-counter sampler was then replaced with persistent lower-overhead counters and native
  process deltas, verified by a separate read-only probe. Observer cost remains visible through the
  sampler PID.
- Host context is diagnostic only. It cannot waive the 100 ms p95 target, turn this development
  host into the designated representative machine, or justify retrying until a pass. No
  representative-hardware run, large admitted operation, Cloud Files fixture, filesystem access,
  review/preflight/operation mutation, Shell call, Recycle Bin mutation, schema/protocol change, or
  production wiring was added.
- Focused distribution coverage passed in Debug and optimized Release. The finalized default
  non-mutating collector passed all 31 deterministic executor tests and emitted schema-v2 evidence;
  parser, sampler-continuity, and non-overwrite-guard probes also passed. The final path-only
  reparse-point hardening parsed and passed its guard check, but an otherwise redundant post-change
  collector rerun was unavailable when the execution environment exhausted its approval allowance.
- The complete Debug/Release Rust workspace and serialized .NET matrix passed. Each .NET
  configuration passed 66 Core, 56 Infrastructure, and 3 WPF tests; five explicitly gated real
  provider/Shell tests remained skipped. Real Debug and Release WPF smoke passed. The first full
  Release verifier retained an unchanged 100,000-set preview failure at 5.284 seconds against its
  five-second ceiling under the already observed host load, after which the shared fixture lock
  poisoned two tests. All three affected tests passed individually without changing a ceiling, and
  the second complete Release publish verification passed, including packaged WPF smoke. This
  verification rerun does not replace or close the independently failed warm-query gate.
- `git diff --check` passed. Repository-wide `cargo fmt --all -- --check` remains blocked only by
  the accepted pre-existing formatting drift in unchanged CLI/FFI files; the modified Rust test
  file is formatted and those unrelated files were preserved.

##### Deterministic WPF focus follow-up (2026-08-22)

- Two real-smoke focus failures were investigated as timing defects rather than retried as an
  acceptance strategy: Debug had intermittently retained focus instead of moving it to the newly
  opened rule-application confirmation heading, and Release had intermittently failed to return
  focus from `Previous set` to the selected virtualized group row. One pre-change evidence run in
  each configuration passed, confirming that neither failure was a stable command/state defect.
- Confirmation focus is still initiated only by the corresponding view-model confirmation
  transition, but now yields through dispatcher idle turns until the heading is both loaded and
  actually visible before bringing it into view and assigning keyboard focus. This avoids racing
  the visibility binding/render pass and avoids focus theft during initial view realization.
- Set navigation now retries the existing scroll/layout/cell-focus operation across bounded
  dispatcher/layout turns and stops only when the grid actually contains keyboard focus. The
  previous fixed 50 ms delay was removed; paging, selection, virtualization, worker protocol,
  review state, and all cache/cancellation bounds are unchanged.
- Focused WPF surface tests passed 3/3 in Debug and 3/3 in Release. The post-change real Debug and
  Release WPF smoke workflows each passed once, including both next/previous-set focus restoration
  and application/reversal confirmation focus. Recycle Bin execution remained disabled and all
  disposable fixture files remained unchanged.

##### Accessibility findings and remaining operator evidence

- Keyboard automation passes for the explicit 30-control duplicate-file tab order, bounded
  group/member/facet paging, next/previous-set focus restoration, and virtualized grids. Minimum
  width is covered at the 900-by-600-DIP main-window minimum and its 620-DIP content workspace:
  the primary file filters, Session Setup editors, and exact-folder filters all have focused STA
  reflow regressions and remain within the workspace.
- Screen-reader automation passes for stable names/IDs and repeatable current-generation
  completion/failure notifications on duplicate-file groups, members, both facets, and exact-folder
  groups/members. Non-displayed prefetch, cancellation, stale generations, and Explorer-only errors
  remain intentionally silent. This proves the UI Automation contract but is not a Narrator or
  NVDA listening pass.
- High-contrast code inspection found the review surfaces use dynamic Windows system brushes for
  borders, backgrounds, and actionable error text; loaded STA tests verify the resolved error
  brushes. This session did not change the operator's Windows theme, so a physical high-contrast
  visual pass remains open.
- The available desktop reports only 96 DPI. WPF layout and the minimum-width regressions use DIPs,
  but no physical 100/150/200-percent multi-monitor transition was available, so clipping, focus,
  and popup placement across real per-monitor DPI changes remain open.
- Current real-WPF smoke attempts reached the selected-root/drive keyboard facet interaction, then
  the session rejected `SendKeys.SendWait` with `The operation completed successfully.` Both
  headless Debug/Release worker smokes and all three STA tests passed. This is the documented
  input-injection restriction, not evidence for the remaining assistive-technology gate.
- To close the operator accessibility gate, use an interactive Windows 11 x64 desktop with
  Narrator or NVDA, a Windows high-contrast theme, and physical 100%, 150%, and 200% monitor scales.
  At both the default and 900-by-600-DIP minimum sizes, traverse Setup, Duplicate files, and
  Duplicate folders without a mouse; exercise every filter, facet, sort, page, next/previous-set,
  copy, and Explorer action; move the window between differently scaled monitors; and record the
  spoken names/status/error announcements, visible focus order, popup placement, contrast, and any
  clipped or unreachable control. A pass requires no focus loss, duplicate/stale announcement,
  inaccessible action, horizontal workspace overflow, or DPI-transition clipping.

#### User outcome

The results surface answers three questions without requiring repeated Explorer investigation:

1. What is duplicated?
2. Where are the copies?
3. Which sets are worth reviewing first?

#### Summary

- Potential recoverable space.
- Matching set/copy counts, largest recoverable opportunity, and aggregate selected-root/drive
  coverage for the current immutable query.
- Entry points such as `1 GB or larger`, `Three or more copies`, and `Across drives`.

#### Duplicate-set list

- Representative name or folder, with wording that representative does not mean original.
- One-copy size, copy count, recoverable size, distinct locations, drives, and selected roots.
- Server-side filters for literal or exact member path, extension/no-extension any/all matching,
  exact selected root, exact drive, minimum size, minimum copy count, and cross-drive sets.
- Stable server-side sorts and paged facet counts.

#### Selected-set detail

- Persistent set header explaining that exact content was verified at scan time.
- Location/root and shortened breadcrumb presentation, with the complete path always accessible.
- Size, modified time, selected-root, relative-path, and drive columns.
- Commands for next/previous set, copy path, and reveal.
- Focus restoration and keyboard shortcuts for continuous review.

#### Engine and protocol

- Group/member DTOs include selected-root and drive/location context plus immutable query summary.
- Add summary/facet queries without materializing member rows.
- Preserve the existing bounded cursor-cache and query-generation rules.
- Do not load members until a set is selected.

#### Acceptance criteria

- A user can inspect thousands of sets without losing selection, focus, or filter state.
- A 100,000-group fixture stays responsive and memory remains bounded by page/cache settings.
- No UI operation binds the complete result or facet dataset.
- Late result or facet responses cannot replace a newer query generation.
- Accessibility names and keyboard actions cover all read-only review navigation and actions.

Durable decisions are intentionally absent from these read-only acceptance criteria and begin in
Milestone 10. Live-state badges, validation, and changed/resolved behavior begin in Milestone 12.

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

#### Refined first vertical slice: durable manual file decisions (2026-08-19)

##### User story and bounded scope

A user reviewing one completed immutable run can mark each visible duplicate-file member `Keep`,
`Remove`, or `Undecided`, see selected-set and whole-plan summaries, close and restart the app, and
resume from the same durable decisions. This first slice supports exactly one active plan per run
and manual file decisions only. It does not add rules, exact-folder decisions, live validation,
preflight, Recycle Bin integration, or any deletion action.

##### Schema v5 and migration

- Advance the Rust-owned product schema transactionally from v4 to v5. `review_plan` belongs to one
  immutable completed run, has an `active`/reserved-future-state boundary, and carries a monotonic
  revision. A partial unique index permits only one active plan per run while leaving room for
  archived plans later.
- `review_decision` belongs to a plan, duplicate-file group, and scanned-file member. It stores the
  explicit decision, `manual` provenance, decision time, and the immutable canonical path, file
  identity, size, modified time, and content hash used when the decision was made. It never reads or
  derives state from `scanned_file.marked_deleted` or `deletion_plan`.
- `review_command` is a bounded-payload idempotency ledger keyed by plan and caller-supplied
  operation ID. It stores the exact run/group/file/decision/expected-revision request and applied
  revision. An exact replay is a no-op with the original applied revision; reuse with different
  payload is rejected.
- Migration uses `BEGIN IMMEDIATE`/rollback, creates all foreign keys/checks/indexes before setting
  `user_version = 5`, preserves every v4 row, and keeps newer-schema rejection fail closed. Deleting
  a run cascades its plans, decisions, and command ledger; truncation clears the new tables before
  immutable run data.

##### Safety and concurrency contract

- `review_decision.set` is allow-listed and accepts `operationId`, `runId`, `groupId`, `fileId`,
  `decision`, and `expectedRevision`. Only `keep`, `remove`, and `undecided` are accepted, and this
  slice always records provenance as `manual`.
- The mutation runs in one immediate transaction: verify a completed run and exact run/group/member
  ownership; get or create its active plan; resolve an idempotent replay; reject a stale expected
  revision; enforce the survivor rule; upsert the snapshot-backed decision; advance the revision;
  and record the command result.
- A `remove` is rejected if the resulting set has no independently accessible physical survivor.
  Non-empty `scanned_file.file_identity` values define physical identity; when identity was not
  captured, canonical path is the conservative distinct fallback. Hard-link aliases therefore
  cannot be counted as independent survivors, while an undecided or kept alias of the same physical
  item means that physical item is still accessible.
- Structured errors distinguish invalid parameters, non-completed/missing run, wrong group/member
  ownership, stale review revision, idempotency-key conflict, and unsafe last-survivor removal.
  Details include only bounded IDs/revisions/counts.
- Review mutation remains SQLite-only and serialized by the worker command path. It does not touch
  the filesystem, hydrate/validate/preview excluded cloud placeholders, or infer current live state.

##### Bounded queries and caching

- `review_plan.get` returns the active plan (or a virtual revision-zero empty plan) and one
  SQLite-owned summary: decided groups, explicit keep/remove/undecided counts, planned removal
  bytes, and remaining independent physical copies.
- `review_group.page` keyset-pages at most 500 duplicate groups with explicit decision counts and
  survivor counts. Its forward-only opaque cursor signature includes run, active plan, revision,
  and page size so a mutation makes an old cursor stale rather than mixing revisions.
- Existing duplicate-member pages left-join only the active plan and expose each visible member's
  decision/provenance plus the current plan revision and selected-group summary. Members remain
  limited to 200 in WPF/500 in protocol and are not loaded until selection.
- Core retains the existing five-page group/member cache bounds. A successful decision clears only
  decision-sensitive member pages, reloads the visible member page and plan summary, and
  rejects late pre-mutation responses through separate review/member generations. No decision
  dictionary grows with the run.

##### WPF interaction and accessibility

- The selected-set header adds a concise set review summary, and the workspace adds an overall
  plan summary. Each virtualized member row shows its current decision and keyboard-native `Keep`,
  `Remove`, and `Undecided` buttons with accessible names containing the complete path.
- Only one mutation is submitted at a time. While it is pending, decision controls are disabled;
  navigation and already displayed results remain coherent. Success announces the new decision and
  refreshed set summary. Structured safety/conflict failures use the existing actionable assertive
  detail-error surface without changing the durable decision.
- Restart reloads plan/member decisions from the worker. Navigating away or changing runs rejects
  late UI responses; the worker/client contract permits an uncertain operation to be retried with
  the same operation ID without applying twice.

##### Performance and acceptance tests

- Storage tests cover v4-to-v5 migration/rollback shape, completed-run ownership, restart
  persistence, all three manual states, snapshot/provenance, idempotent replay/conflict, stale
  revision, and physical-survivor enforcement with hard-link aliases.
- Worker tests cover allow-listing, structured errors, decimal-safe DTOs, cursor revision binding,
  and restart persistence without filesystem access. Infrastructure tests cover exact JSON
  contracts and error propagation.
- Core/WPF tests cover commands and accessible names, successful refresh/announcement, safety and
  stale-generation errors, cache invalidation/bounds, and late mutation/member/summary rejection.
- A disposable 100,000-group/large-plan fixture must keep plan summary and first/next group pages
  within the existing five-second regression ceiling and the explicit Release profile records warm
  plan-query and private-memory evidence. Standard Debug/Release Rust and .NET matrices plus real
  worker/WPF smoke remain required before acceptance.

##### Acceptance result

- Schema v5, the transactional v4 migration, durable snapshots/provenance, all three decisions,
  idempotent replay/conflict, stale revision, completed-run ownership, restart persistence, and
  hard-link-aware survivor enforcement are implemented in Rust-owned SQLite. Standard storage
  runs passed 20 tests with the operator profile intentionally ignored; worker protocol runs passed
  all 10 tests in Debug and Release.
- `review_plan.get`, revision-bound `review_group.page`, and `review_decision.set` are allow-listed
  with bounded request validation and structured errors. Existing member pages expose decision
  state and selected-set summaries. The real typed Infrastructure lifecycle covers virtual-plan
  state, plan creation, exact replay, stale rejection, member refresh, diagnostics, and persistence.
- Core adds bounded, generation-checked mutation/refresh behavior and three native member-row
  commands. The selected-set and whole-plan summaries are polite live regions; mutation failures
  use the existing assertive detail surface. WPF STA coverage validates decision columns, keyboard-
  focusable buttons, and complete-path accessible names.
- The dedicated optimized 100,000-group regression passed in 2.47 seconds, including first and
  next review-group pages. The explicit 100-sample Release profile measured review-plan p95 at
  38.90 ms, review-group p95 at 5.37 ms, and 753,664 bytes peak private-memory growth. That profile
  still returns failure for the independently open Milestone 8 warm-query gate: on this run the
  existing group/summary p95 was 470.50 ms and drive-facet p95 was 137.97 ms against 100 ms; root-
  facet p95 was 83.40 ms. These measurements do not close the representative-hardware gate.
- Full `cargo test --workspace` and `cargo test --workspace --release` passed. Debug and Release
  Windows solution runs passed 53 Core, 22 Infrastructure, and 3 WPF tests with the one real cloud-
  provider test intentionally skipped. Explicit Debug and Release worker builds passed.
- Real Debug and Release WPF/worker smoke passed. Each recorded a durable `Remove`, observed both
  summaries refresh, proved the disposable fixture file still existed, and retained the accepted
  filters, facets, focus restoration, Explorer, cloud-fail-closed, and shutdown coverage. No
  deletion or live-state behavior was exposed.

#### Refined second vertical slice: manual exact-folder copy decisions

##### User story and bounded scope

A user reviewing the visible copies in one exact-folder set can mark one immutable folder copy
`Keep`, `Remove`, or `Undecided`, see how that choice overlaps existing manual file decisions, and
resume the same review after an application or worker restart. Folder and file mutations share the
one active review plan and its monotonic revision. This slice remains review-only: it does not read
the current filesystem, validate a tree, access excluded cloud placeholders, create an executable
schedule, or expose deletion.

##### Schema v6 and immutable folder-copy snapshot

- Migrate the Rust-owned product database transactionally from v5 to v6 with `BEGIN IMMEDIATE`.
  Create every table, foreign key, check, and index before setting `user_version = 6`; rollback
  leaves a valid v5 database unchanged. Existing plans, file decisions, command replays, immutable
  results, and legacy tables are preserved without reinterpretation.
- Add a separate `review_folder_decision` table. Do not overload `review_decision`: a folder choice
  belongs to one plan, exact-folder group, and immutable `duplicate_folder_group_member` copy, and
  also references that member's `directory_node`. Its unique key is `(plan_id, folder_member_id)`.
- Snapshot manual provenance, decision time, canonical folder path, descendant logical byte/file
  totals, and the group's structural and verified fingerprints into the decision row. The stable
  folder-member/directory IDs establish immutable ownership; snapshot fields explain the exact
  scanned tree on which the choice was made. No snapshot field is refreshed from the live tree.
- Add a distinct `review_folder_command` ledger whose payload is
  `(run, folderGroup, folderMember, decision, expectedRevision)`. Operation IDs are 1--128
  characters and are idempotent within this command family: an exact replay returns its original
  applied revision, while reuse for another folder payload is rejected. The file-specific
  `review_command` shape and replay contract remain intact rather than accepting nullable or
  polymorphic folder fields.
- Run deletion cascades through both ledgers and both decision tables. Database truncation clears
  the ledgers and decisions before plans and immutable run data. Neither table uses
  `scanned_file.marked_deleted` or legacy `deletion_plan` as review truth.

##### Folder decisions and overlap safety

- `Keep` protects the complete snapshotted folder copy: no effective file or folder removal may
  contain that root or any of its descendants. `Remove` expresses review intent for every
  snapshotted file path under that copy. `Undecided` clears either effect while retaining an
  explicit reversible manual row and snapshot; a missing row is also effectively undecided.
- Determine containment from the immutable run-owned `directory_node.parent_id` hierarchy and join
  files by the snapshotted directory path. Never use string-prefix containment or enumerate the
  filesystem. A recursive CTE may walk one addressed subtree or its ancestors, but no complete
  tree is materialized in Core or WPF.
- Evaluate the proposed mutation against the effective removal union in the same immediate
  transaction. That union contains file paths explicitly marked `remove` plus paths beneath folder
  copies marked `remove`. Reject a file/folder `Remove` already covered by another removed folder,
  a nested removed-folder overlap, and any `Keep`/`Remove` containment conflict. The actionable
  error identifies the conflicting decision kind and bounded immutable IDs so the user can clear
  the older decision first.
- After applying the proposal virtually, every exact-folder set, including retained suppressed
  nested groups, must have at least one intact independently accessible folder root. A root is
  intact only when neither it nor an ancestor folder copy is removed and no effective file removal
  lies in its snapshotted subtree. Suppressed groups are protected by this invariant but remain
  non-addressable until the existing advanced visibility toggle is implemented.
- Also re-evaluate every duplicate-file set touched by a folder subtree, and extend file mutation
  safety to folder decisions. At least one accessible alias of one physical item must remain.
  Non-empty immutable `file_identity` values identify hard-link aliases; canonical path remains the
  conservative fallback. Removing one hard-link path does not remove a surviving alias, and aliases
  never inflate the independent-physical-survivor count.
- Combined totals are derived from the effective union, not by adding folder and file subtotals.
  Logical target paths are distinct by immutable file ID; planned physical items and bytes are
  distinct by physical key. Thus nested trees, hard links, and a file decision overlapping a folder
  can neither be counted nor later scheduled twice. This slice exposes no schedule or execution
  command.

##### Shared revision, commands, and structured errors

- Add `review_folder_decision.set` rather than widening `review_decision.set`. It validates a
  completed run and exact run/group/member/directory ownership, creates the active plan lazily,
  resolves a folder-command replay, checks `expectedRevision`, performs overlap and both survivor
  checks, snapshots/upserts the folder choice, advances the same `review_plan.revision`, and records
  the replay result in one transaction.
- Extend `review_decision.set` so a file proposal performs the same cross-kind overlap and folder-
  survivor checks before advancing that shared revision. Existing exact replays remain replayable
  after later folder mutations because replay resolution precedes the current-revision check.
- Keep `review_generation_conflict`, `idempotency_conflict`, and the existing file-member errors.
  Add separate wrong-folder-group/member ownership, unsafe-folder-survivor, and review-overlap error
  codes. Details contain only bounded IDs, current/expected revisions, and conflict kinds; WPF maps
  each to a concrete recovery action instead of displaying raw protocol text.

##### Bounded, revision-consistent reads

- `review_plan.get` continues to return one active/virtual plan and now returns a combined summary
  for the same revision: explicit file and folder Keep/Remove/Undecided counts, decided file/folder
  set counts, distinct effective logical targets, distinct planned physical items/bytes, remaining
  file physical survivors, and intact exact-folder copy count. The aggregation is SQLite-owned and
  returns one fixed-size row.
- Add forward-only `review_folder_group.page`, limited to 500 visible exact-folder groups, with
  Keep/Remove/Undecided copy counts and intact-copy count. Its opaque keyset cursor binds run,
  active plan ID, shared revision, page size, and visibility mode; a mutation invalidates it.
- Extend the bounded exact-folder member query to left-join only the active plan and return each
  visible copy's decision/provenance/time plus the selected folder-set summary. Its existing maximum
  remains 500 (200 in WPF), and both next/previous cursor signatures bind the active plan ID and
  shared revision. Old revision cursors fail instead of mixing member generations.
- Plan/combined-summary, folder-group, and member responses all carry the plan ID/revision they
  describe. Queries cover visible groups by default; suppressed rows participate only in invariant
  evaluation until their separately designed visibility entry point exists.

##### Core caching, restart, and stale-response behavior

- Reuse the exact-folder workspace's independent five-page group and five-page member LRU bounds.
  Decision metadata lives only on cached/visible pages; no run-wide folder-decision dictionary is
  introduced. Add at most a five-page review-folder-summary cache if the UI needs a separate
  summary channel.
- A successful mutation cancels/increments folder-review and member generations, clears only
  decision-sensitive folder pages, refreshes the displayed member page and combined plan summary,
  then announces success. File pages are invalidated when a folder choice changes their effective
  state; folder pages are invalidated when a file choice changes intact-copy state.
- Run changes, navigation, cancellation, and worker restart reject late plan, group, member, and
  mutation responses by run ID, selected immutable IDs, generation, and returned review revision.
  Restart reconstructs every decision and summary from SQLite. An uncertain folder operation is
  retried only with its original folder operation ID and exact payload.

##### WPF interaction and accessibility

- The exact-folder workspace shows a combined plan summary and a selected-set folder summary. Each
  virtualized folder-copy row displays its decision and native `Keep`, `Remove`, and `Undecided`
  buttons whose accessible names include the complete immutable folder path.
- Buttons remain reachable in deterministic tab order and use native Space/Enter activation;
  arrow/page navigation and existing next/previous-page focus restoration remain unchanged. While
  one review mutation is pending, decision buttons are disabled without moving keyboard focus or
  blocking other already displayed navigation.
- Success raises one polite, repeatable announcement containing the folder path, new decision, and
  refreshed set summary. Overlap, stale-revision, and unsafe-survivor failures use the assertive
  detail-error surface with an actionable instruction such as clearing the named contained file or
  folder decision first. Automation names/help text never imply that review performs deletion.
- Database, worker, filesystem, Explorer, and Shell work stays off the WPF dispatcher. This slice
  adds no filesystem work; existing explicit Explorer commands retain their established background
  service boundary.

##### Verification and performance bounds

- Storage tests cover v5-to-v6 migration/rollback shape, snapshot ownership, all three states,
  restart, folder-command replay/conflict, shared stale revisions, nested/ancestor overlap,
  suppressed-group survival, contained file decisions, hard-link aliases, and deduplicated combined
  counts. Worker tests cover allow-listing, exact DTOs, structured errors, decimal-safe totals, and
  revision-bound folder-group/member cursors.
- Core/Infrastructure/WPF tests cover typed folder commands, five-page cache bounds, cross-workspace
  invalidation, late-generation/revision rejection, keyboard-focusable row controls, complete-path
  accessible names, announcements, and actionable errors.
- A disposable fixture with 100,000 visible groups plus nested suppressed groups and overlapping
  file/folder aliases must keep the combined plan row and first/next folder-review pages under the
  existing five-second regression ceiling. Record optimized warm p95 and private-memory growth for
  the new queries without claiming the independently open Milestone 8 representative-hardware,
  Narrator/NVDA, OS high-contrast, or multi-monitor DPI gates.

##### Acceptance result

- Schema v6 and the transactional v5 migration now preserve the v5 review plan/file command
  contract while adding separate snapshot-backed folder decisions and folder-command replay.
  Storage coverage exercises all three states, snapshot provenance, restart, exact replay and
  conflicts, the shared revision, hard-link physical survivors, nested and suppressed exact-
  folder sets, contained file decisions, and combined logical/physical deduplication.
- `review_folder_group.page` and `review_folder_decision.set` are separate bounded protocol
  commands. Exact-folder member cursors bind the active plan revision and return decision metadata
  plus the selected-set summary. Structured overlap, ownership, stale-revision, and unsafe-folder
  errors remain review-only and perform no live filesystem or cloud-placeholder access.
- Core keeps the existing five-page group/member cache bounds, rejects late run/revision responses,
  and propagates a successful shared revision between the file and folder workspaces so the peer
  tab clears decision-sensitive pages and refreshes its visible summary. Native WPF row buttons
  expose complete-path names, native keyboard activation, non-deleting help text, polite success
  announcements, and actionable assertive failures.
- The optimized 100,000-visible-group regression kept its initial combined plan and first/next
  folder-review pages under five seconds. Its 100-sample Release profile measured combined-plan
  p95 at 6.40 ms, folder-review-page p95 at 14.50 ms, and no observed private-memory growth
  growth on this development host. This is bounded regression evidence, not representative-
  hardware evidence and does not close any independent Milestone 8 gate.
- Full Debug and Release Rust and Windows matrices passed with 24 storage tests plus one intentional
  representative-profile ignore, 10 worker tests, 57 Core tests, 22 Infrastructure tests plus one
  intentional provider skip, and 3 WPF STA tests. Explicit worker builds and real Debug/Release
  smoke passed; smoke recorded a folder `Keep`, restarted the worker, observed the durable choice,
  activated the accessible WPF control, and proved the disposable directory still existed.
- No command exposes a deletion schedule or execution path, uses `marked_deleted` or legacy
  deletion plans as review truth, validates live state, or hydrates an excluded cloud placeholder.

#### Refined third vertical slice: ordered preferred scan-root rule preview (2026-08-19)

##### User story, boundary, and persistence decision

A user can save a named ordering of scan roots, choose an explicit completed-run scope, and inspect
which immutable duplicate-file paths the rule would prefer to keep or remove before deciding
whether that rule is useful. The preview explains ties, roots absent from the rule, manual-decision
precedence, hard-link aliases, and folder-decision conflicts. Closing and restarting the app keeps
the named rule; rerunning the preview reconstructs the same result when the immutable run, rule
revision, and review-plan revision are unchanged.

Schema and named-rule persistence land with preview because an unnamed, restart-volatile ordering
cannot provide the required recovery or explanation contract. Saving rule configuration is a
separate, bounded metadata mutation: it never creates, changes, or clears a review decision and
never advances the review-plan revision. Rule application remains deferred to a later separately
designed mutation slice. This slice adds no rule-produced provenance row, executable schedule,
live validation, filesystem access, deletion, or Recycle Bin operation.

##### Schema v7 and named ordered rules

- Migrate the Rust-owned product database transactionally from v6 to v7 with `BEGIN IMMEDIATE`.
  `preference_rule` stores a unique case-insensitive name, the fixed kind
  `ordered_preferred_scan_roots`, an `active`/reserved `archived` state, a monotonic rule revision,
  and created/updated timestamps. `preference_rule_root` stores a dense zero-based order and the
  exact root value. It is separate from review plans/decisions, immutable run/live-state tables,
  legacy deletion staging, and future execution state.
- Root values are nonblank absolute Windows paths of at most 32,767 Unicode scalar values. Saving
  trims no path characters and performs no filesystem normalization, canonicalization, existence
  check, or Cloud Files access. Values are de-duplicated with locale-independent Unicode
  case-insensitive comparison, matching immutable `scanned_file.root_path`; an exact duplicate is
  rejected rather than silently reordered. A rule contains 1--64 roots and a name contains 1--128
  trimmed Unicode scalar values.
- `preference_rule.save` uses an operation ID and optional rule ID/expected revision. Creating with
  no ID requires expected revision zero; updating requires the current positive revision. The rule
  row, complete replacement root list, revision advance, and idempotency record commit together.
  Exact replay returns the original rule/revision; reuse with another payload is rejected. Rule
  configuration revision is independent of the active review-plan revision.
- `preference_rule.list` and `preference_rule.get` are bounded SQLite-owned reads. V1 lists at most
  200 active named rules ordered by case-insensitive name and ID and returns fixed-width metadata;
  `get` returns at most 64 ordered roots. Archiving/deleting and rule application are deferred, so
  no saved rule can disappear through this surface.
- Migration creates all foreign keys, checks, and indexes before setting `user_version = 7`, rolls
  back cleanly to v6 on failure, preserves every schema-v6 row, and continues to reject unknown
  newer schemas. Run deletion and database truncation do not delete reusable rule configuration;
  explicit test cleanup removes rule tables before session data without reinterpreting decisions.

##### Exact ordered-root and virtual-decision semantics

- Matching is exact, locale-independent Unicode case-insensitive equality between a rule root and
  each member's immutable `scanned_file.root_path`. The preview does not infer containment from
  path strings. A root absent from the rule is unranked; a rule root absent from the addressed run
  is reported once in fixed-size summary counts but is not an error.
- Evaluate each duplicate-file set from the immutable snapshot plus the active manual file/folder
  decisions at the requested plan revision. Explicit manual `Keep` and `Remove` always win and are
  never rewritten in the virtual result. Explicit or implicit `Undecided` remains rule-eligible.
  A manual `Remove` cannot become a preferred survivor; ranking therefore considers rule-eligible
  members not already effectively removed by manual file or folder decisions.
- If no eligible member has a ranked root, the set is unaffected with reason `no_ranked_root`.
  Otherwise the lowest numeric rank present is preferred. Every eligible logical path at that same
  best rank is virtually kept; the rule never breaks a tie by path, member ID, timestamp, or other
  hidden criterion. Other eligible paths are virtually removed, including unranked paths. Existing
  manual Keeps remain kept even when lower-ranked, so preview may intentionally show more than one
  survivor and explains `manual_keep_precedence`.
- Non-empty immutable `file_identity` is the physical key; canonical path is the conservative
  fallback when identity was not captured. Hard-link aliases participate as separate logical paths
  but one physical item. A preferred alias can protect the physical item while a lower-ranked alias
  is a logical removal target; aliases never inflate survivor or physical-target totals. If all
  aliases of one physical item are virtually removed, that item contributes once to physical item
  and byte totals.
- Manual folder `Keep` protects its complete immutable directory subtree and blocks a conflicting
  virtual removal. Manual folder `Remove` remains effective and cannot be undone by a virtual keep.
  Folder containment uses the run-owned `directory_node.parent_id` hierarchy and immutable file
  parent membership, never a string prefix or live enumeration. Suppressed exact-folder groups
  continue to participate in safety even though they are not preview rows.
- Apply the existing file/folder overlap, at-least-one-independent-physical-file-survivor, and
  at-least-one-intact-exact-folder-copy invariants to the complete virtual set. A set whose proposed
  changes violate an invariant is `blocked`, makes no virtual rule changes, and carries a stable
  actionable reason plus bounded conflicting file/folder/group IDs. Preview never silently skips a
  conflicting proposal while counting it as applicable.

##### Explicit scopes and complete filter signature

- `preference_rule.preview` requires exactly one scope: `selected_sets`, `current_filter`, or
  `completed_run`. All forms require one completed immutable run and one active saved rule.
- `selected_sets` contains 1--500 distinct duplicate-file group IDs, all owned by the run. The
  canonical signature sorts IDs ascending so selection order does not change cursor identity.
- `current_filter` carries the complete duplicate-file group filter: search/path-match, extension/
  extension-match, minimum one-copy size, minimum copy count, across-drives, exact selected root,
  and exact selected drive. It uses the existing normalization, indexed predicates, and scalar
  limits. Omitting a field receives the same accepted default as `duplicate_file_group.page`; a
  partial client-side approximation or only the currently loaded page is not a valid scope.
- `completed_run` addresses every duplicate-file set in the immutable run. Scope never implies the
  currently selected WPF rows or cached pages. Scope metadata and explanations always state which
  of the three forms was evaluated.

##### Revision-bound paging, fixed summaries, and errors

- The response keyset-pages at most 500 affected or blocked sets in stable group-ID order. An
  opaque cursor binds command kind, run, rule ID/revision, active-or-virtual plan ID/revision,
  normalized scope signature, and page size. A saved-rule edit or manual file/folder mutation makes
  old cursors return `invalid_cursor`; a late page can never mix generations.
- Each row contains bounded IDs/counts, status (`applicable` or `blocked`), best rank when present,
  tied preferred logical-path count, proposed logical keep/remove counts, proposed physical-remove
  item count/bytes, manual Keep/Remove counts, and a stable primary explanation code. A separate
  member-detail command is deferred; previews do not materialize run-wide path collections in WPF.
- One fixed-size SQLite-owned summary accompanies every page and uses the same rule, plan revision,
  and complete scope: scoped sets/logical paths/physical items/bytes; affected and blocked sets;
  proposed Keep/Remove logical paths; distinct proposed physical items/bytes; manual Keep/Remove
  paths; tied sets; sets with no ranked root; missing configured roots; and conflict counts by
  overlap, file-survivor, and folder-survivor class. Logical paths de-duplicate by immutable file
  ID; physical items and bytes de-duplicate by physical key across the complete scope, not by
  adding per-set totals.
- Expected structured errors distinguish missing/non-completed run, missing/archived rule, stale
  rule revision, stale review revision, invalid/foreign selected sets, invalid complete filter,
  invalid scope, invalid cursor, and bounded preview complexity. V1 caps one evaluation at 100,000
  sets and 500,000 logical paths and returns `preview_too_complex` rather than a partial preview.
  Blocked set rows contain stable conflict explanations; request-level errors never expose raw SQL
  or unbounded path data.

##### Core lifetime, cancellation, restart, and performance

- Core owns independent cancellation sources and query generations for rule list/get/save and
  preview. It retains at most five preview pages and one fixed summary for the selected run/rule/
  scope/revisions. Changing run, rule, rule order, scope, filter, or review revision cancels current
  waits, clears decision-sensitive preview pages, and rejects late responses by the complete
  generation key. No run-wide decision, path, or physical-identity dictionary is introduced.
- Host cancellation stops awaiting a page without treating the read as a mutation. The bounded
  SQLite read may finish in the worker after its correlation wait is cancelled; Core rejects that
  response through the complete generation key, and no partial preview state exists to reconcile.
  Worker exit likewise leaves no preview mutation behind. Restart reloads the saved rule and
  recomputes preview solely from SQLite.
- Disposable coverage includes schema migration/rollback, save replay/conflict/revision/restart,
  all three scopes, complete filter/cursor signatures, ties, missing roots, hard-link aliases,
  manual precedence, folder containment, suppressed groups, overlap/survivor conflicts, and global
  logical/physical de-duplication. A 100,000-set fixture records first/next page and fixed-summary
  optimized p95 plus private-memory growth under the existing five-second regression ceiling.
  This evidence cannot close the independent representative-hardware Milestone 8 gate.

##### WPF interaction and accessibility

- The duplicate-file workspace exposes a named-rule editor with explicit Move up/Move down buttons
  for the ordered root list, Save, scope selection, and Preview. Native controls have deterministic
  tab order and Space/Enter activation; list selection plus the move buttons provides a complete
  keyboard reordering path without drag-and-drop. Each list item exposes the root's complete
  immutable value, and the ordered list plus Move up/Move down labels explain its explicit rank.
- The preview heading states that nothing has been applied or deleted. A fixed summary identifies
  the scope and counts applicable/blocked/tied/manual-precedence outcomes. Virtualized paged rows
  expose concise reasons such as `Kept because D:\\Photos ranks above E:\\Backup`, `Kept because
  both paths share the highest-ranked root`, and `Blocked because a manual folder Keep protects a
  contained path`; no member is called the original.
- Successful save and preview completion use repeatable polite announcements. Invalid rule input,
  stale revisions, worker failure, and blocked-result details use the established actionable
  assertive surface. Navigation remains usable while a read is pending; only the specific save or
  preview controls are disabled. Database, worker, filesystem, Explorer, and Shell work remains off
  the WPF dispatcher.

##### Acceptance boundary

Acceptance requires schema-v7, storage/worker/Core/Infrastructure/WPF coverage, restart persistence,
large-fixture bounds, Debug/Release matrices, and real non-deleting smoke. It does not create rule
decisions, modify manual decisions, advance a review plan, use `scanned_file.marked_deleted` or
legacy `deletion_plan`, validate live state, read excluded placeholders, or expose/schedule/execute
deletion. A later application slice must separately design rule snapshot provenance, idempotent
plan mutation, reversal, conflicts after preview, and revalidation of the same invariants.

##### Acceptance result

- Schema v7 and the read-only preview are accepted. Named ordered rules persist independently from
  review/live/execution state, exact idempotent save replay returns its originally applied revision,
  and restart reconstruction preserves rule and preview behavior. No rule command writes manual
  file/folder decisions or advances the shared review-plan revision.
- Storage and worker coverage exercises exact rank/tie/missing-root behavior, all three explicit
  scopes, complete filter and cursor signatures, hard-link de-duplication, manual precedence,
  folder containment, overlap and survivor conflicts, stale revisions, and 100,000-set bounds.
  The final Debug and Release Rust suites each passed 29 storage tests with the two operator
  profiles intentionally ignored and 11 worker tests.
- Core uses a separate five-page cache, cancellation source, and query generation; rule/run/scope/
  filter/review changes invalidate decision-sensitive pages and reject late responses. The final
  Debug and serialized Release Windows matrices each passed 59 Core, 22 Infrastructure plus the
  intentional provider skip, and 3 WPF STA tests.
- The isolated optimized 100-sample development-host profile evaluates 100,000 real sets and
  200,100 logical paths. Its complete-run first-page preview plus fixed summary measured 870.42 ms
  p95 with 2,199,552 bytes of retained private-memory growth, below the five-second query and 32 MB
  repeated-growth ceilings. This is regression evidence only and does not close the
  representative-hardware gate.
- Real interactive Debug and Release smoke passed with a saved rule, a non-empty read-only preview,
  worker restart persistence/reconstruction, accessible WPF completion text, and disposable files
  still present. No command exposes deletion, reads live filesystem state, or accesses excluded
  cloud-placeholder content.

#### Refined fourth vertical slice: bounded ordered-rule application and reversal provenance (2026-08-19)

##### User story, confirmation boundary, and non-goals

A user who has completed an ordered preferred-root preview can review one fixed confirmation
summary, apply exactly that rule revision to exactly that immutable run/scope/review revision, and
later reverse that application without erasing manual review choices made before or after it.
Application and reversal update only durable review intent. They do not validate a live path,
hydrate an excluded cloud placeholder, create an executable schedule, invoke Shell or the Recycle
Bin, or delete anything.

The worker accepts no implicit current selection or currently loaded page. The application request
resubmits the complete canonical scope plus the preview signature, rule revision, and source review
revision. Core enables confirmation only for the most recent complete preview generation. Editing
the rule, scope, filter, selected sets, run, or any review decision cancels confirmation and
requires another preview.

##### Schema v8 and immutable application provenance

- Migrate Rust-owned SQLite from v7 to v8 in one `BEGIN IMMEDIATE` transaction. Preserve all v7
  rule configuration, manual file/folder decisions, command replays, and immutable scan results.
  Create every table, column, foreign key, check, and index before setting `user_version = 8`;
  failure rolls back to an unchanged v7 database and unknown newer schemas remain fail closed.
- Add `manual_revision` to `review_decision`, defaulting migrated rows to zero. New manual file
  mutations store their resulting shared plan revision. Manual `Keep` and `Remove` always outrank
  rule output. A manual `Undecided` recorded after an application also clears that application's
  effective choice without becoming rule-owned; an older `Undecided` remains rule-eligible, as it
  was during preview.
- `review_rule_application` belongs to one active review plan and snapshots the application ID,
  operation ID, run, rule ID/revision/name/kind, exact ordered roots JSON, canonical scope kind and
  complete scope JSON/signature, source preview signature, source review-plan revision, applied
  revision, fixed apply counts/bytes, state, and timestamps. The snapshot remains explainable if
  the reusable rule is later edited. Reusable rule configuration remains independent and is never
  rewritten by application or reversal.
- `review_rule_decision` is separate from `review_decision` and
  `review_folder_decision`. It belongs to exactly one application and snapshots the immutable
  file/group identity, `keep` or `remove`, stable explanation code and rank, and file metadata used
  during evaluation. At most one active rule-produced row may own a plan/file at a time. Rule rows
  never masquerade as manual provenance and never use `scanned_file.marked_deleted` or legacy
  `deletion_plan`.
- `review_rule_reversal_command` stores the exact operation ID, application, run, expected current
  review revision, and applied reversal revision. Application operation IDs and reversal operation
  IDs are each bounded to 1--128 characters and independently idempotent. Run deletion cascades
  application decisions, applications, and reversal commands with the plan; explicit truncation
  clears them before manual review and immutable run data while preserving reusable rules until
  their existing explicit cleanup phase.

##### Effective-decision and later-manual-override semantics

- Effective file review state is one bounded SQL-owned overlay. A manual `Keep`/`Remove` wins
  regardless of age. A manual `Undecided` wins only when its `manual_revision` is newer than the
  rule application's applied revision. Otherwise an active rule-produced row is effective; with no
  effective row the file is implicitly undecided. Folder review remains manual-only.
- A later manual file mutation writes only `review_decision`, with manual provenance and its own
  immutable snapshot/revision. It never edits or adopts the corresponding `review_rule_decision`.
  This preserves the exact application record while allowing immediate user control. Reversing the
  application later deletes the hidden/superseded rule row and leaves the manual row untouched.
- Existing member pages and plan/group summaries expose effective decision/provenance plus an
  optional application ID. Fixed plan summaries distinguish effective Keep/Remove totals from
  manual file/folder counts and active rule-produced Keep/Remove counts. They continue to globally
  de-duplicate effective logical removals and physical targets.
- A new application may address multiple disjoint scopes over time, but it cannot replace a rule
  row owned by another active application. Any applicable proposal that overlaps an active
  application returns one structured `rule_application_overlap` conflict and writes nothing. The
  user can reverse the earlier application or make a manual override; provenance is never silently
  transferred between applications.

##### Apply transaction, exact replay, and conflict behavior

- `preference_rule.apply` requires `operationId`, `runId`, `ruleId`, `ruleRevision`,
  `sourceReviewRevision`, `previewSignature`, and exactly the same canonical scope accepted by
  preview. V1 retains the 100,000-set and 500,000-logical-path limits. The worker recomputes the
  signature from the complete normalized inputs; a partial filter, loaded page, changed selected
  set, or stale preview signature is invalid.
- In one immediate transaction, validate the completed immutable run and active rule; create the
  active plan only when required; resolve an exact operation replay before current-generation
  checks; require the current rule and plan revisions to equal the submitted source revisions;
  rerun the exact preview evaluation; reject complexity or active-application overlap; stage rule
  rows only for preview-`applicable` sets; and rerun file/folder overlap, physical-survivor, and
  intact-folder-copy invariants across the complete proposed effective plan.
- Preview-blocked sets remain unchanged and are recorded only in the application's fixed counts.
  If there is no applicable Keep or Remove row, return `rule_application_empty`. Otherwise all
  provenance, rule decisions, the single plan-revision advance, and the idempotency result commit
  together. Any validation, generation, overlap, invariant, insertion, or commit failure rolls the
  entire operation back; there is no per-set partial success.
- An exact replay returns the original application ID, applied revision, and fixed outcome with
  `replayed: true`, even after later plan revisions or reversal. Reusing the operation ID with a
  different run, rule/revision, review revision, preview signature, or canonical scope returns
  `idempotency_conflict`. Drift before a first application returns the specific rule/review/
  preview conflict and makes no change.

##### Bounded idempotent reversal and restart recovery

- `preference_rule.application.reverse` requires a new operation ID, run/application IDs, and the
  expected current review revision. It resolves exact replay first, verifies plan/run ownership and
  that the application is active, deletes only `review_rule_decision` rows owned by that
  application, marks its provenance record reversed, advances the shared plan revision once, and
  records the reversal in one immediate transaction. It never deletes or rewrites manual file or
  folder decisions, another application's rows, reusable rule configuration, live state, or
  execution state.
- Reversing an already reversed application with a new operation ID returns
  `rule_application_already_reversed`; retrying the original exact reversal returns its original
  revision. A stale expected revision or ownership mismatch changes nothing. Reversal reruns the
  effective overlap/survivor invariants before commit; although restoring undecided paths is
  normally monotonic, the invariant check remains part of the fail-closed contract.
- `preference_rule.application.page` keyset-pages at most 200 application summaries for one
  completed run, optionally narrowed to a rule, in descending application-ID order. Its cursor
  binds run, optional rule, active plan ID/current revision, state filter, and page size. Each row
  returns only fixed provenance/count fields; exact roots and scope JSON remain available through a
  bounded single-application detail command. Restart reconstructs active/reversed history,
  effective decisions, summaries, and reversal availability solely from SQLite.

##### Shared revisions, cancellation, caching, and performance

- Successful apply or reverse advances the same `review_plan.revision` exactly once and invalidates
  file, folder, plan-summary, preview, and application-history cursors. Core propagates that
  revision through the existing cross-workspace notification path. A successful later manual
  mutation likewise invalidates preview/application confirmation while preserving application
  history.
- Core uses independent cancellation sources and generations for preview, apply, reverse, and
  application history. It keeps at most five application-history pages plus the existing five
  preview/group/member page bounds and fixed summaries. Run/rule/scope/filter/revision changes
  cancel waits and reject late results by run, immutable IDs, operation ID, generation, and returned
  revision. No run-wide decision or provenance dictionary is introduced.
- Cancellation of an uncertain apply/reverse wait never invents success or retry with a new ID.
  Core retains the original operation ID and exact payload for explicit retry; replay makes worker
  exit/restart recovery deterministic. Database, worker, filesystem, Shell, and Explorer work stay
  off the WPF dispatcher.
- Disposable tests cover v7 migration/rollback, application snapshot shape, all scopes, exact
  signature/replay/conflict, rule/review drift, manual precedence and later overrides, overlapping
  applications, blocked sets, file/folder overlap, hard-link physical survivors, suppressed-folder
  survivors, reversal isolation/replay/staleness, restart recovery, revision-bound paging, and
  global logical/physical de-duplication. A 100,000-set/500,000-path upper-bound fixture records
  optimized apply, reversal, plan-summary, and first/next history-page time plus retained private-
  memory growth under explicit development-host ceilings. It cannot close the independent
  representative-hardware gate.

##### WPF confirmation and accessibility

- After a complete preview, native `Apply rule` enters an inline confirmation region rather than
  applying immediately. The region states rule name/revision, run, exact scope label, source review
  revision, applicable/blocked set counts, rule Keep/Remove path counts, physical removal items/
  bytes, and that this changes review decisions only. `Confirm application` and `Cancel` are native,
  keyboard-focusable buttons with deterministic tab order and Space/Enter activation; focus moves
  to the confirmation heading on entry and returns to `Apply rule` on cancel.
- Apply remains unavailable for a partial/late preview, zero applicable decisions, stale rule or
  review revision, or an in-flight mutation. Confirmation never labels a path the original and
  never implies deletion. Success announces the applied rule, decision counts, and new review
  revision politely, refreshes bounded summaries/history, and offers a native `Reverse this
  application` action.
- Reversal uses its own inline confirmation naming the application and the exact rule-produced
  Keep/Remove counts that will be cleared. Success announces that manual choices were preserved.
  Stale generation, idempotency reuse, overlapping application, already-reversed, and invariant
  failures use the established assertive actionable-error region. Navigation remains available
  while only the relevant mutation controls are disabled, and virtualized preview/history rows
  retain concise complete accessible names and explanations.

##### Acceptance boundary

Acceptance requires schema-v8/storage/worker/Core/Infrastructure/WPF coverage, restart recovery,
large-fixture regression evidence, Debug/Release matrices, and real non-deleting smoke proving
application, later manual override, reversal, restart persistence, accessible confirmation, and
unchanged disposable files. Until those checks pass this section is design, not a completed gate.
It cannot close representative-hardware warm queries or physical Narrator/NVDA, OS high-contrast,
or multi-monitor DPI-transition verification, and it introduces no Milestone 11/12 execution or
live-state behavior.

##### Acceptance result

- Schema v8 now preserves manual rows while storing immutable application snapshots, separate
  rule-produced rows, later-manual revision precedence, exact apply/reversal command replay, and a
  SQL-owned effective-decision overlay. Apply and reverse rerun complete file/folder overlap,
  physical-survivor, and intact-folder-copy invariants in one immediate transaction; conflicts
  write nothing and reversal removes only its application's rule rows.
- The worker now exposes exact preview-bound apply, revision-bound fixed-summary history pages,
  bounded single-application detail, isolated reversal, preview signatures, member application
  provenance, structured conflicts, and shared review-plan invalidation. Core/Infrastructure keep
  history and preview caches at five pages, retain operation IDs for deterministic replay, and
  reject cancelled or stale run/rule/revision generations. WPF adds native inline apply/reversal
  confirmations, deterministic heading focus, accessible summaries/explanations, polite success
  announcements, assertive actionable errors, and no deletion affordance.
- `cargo test --workspace` and `cargo test --workspace --release` passed. Storage passed 31 tests
  with three intentional operator profiles ignored; worker passed 11 tests. Explicit Debug and
  Release worker builds passed. Serialized Debug and Release Windows matrices each passed 60 Core,
  22 Infrastructure plus one intentional provider skip, and 3 WPF STA tests under installed SDK
  10.0.400 from `C:\Windows\Temp`; pinned unavailable SDK 10.0.303 remained unchanged.
- The isolated optimized development-host profile applied one completed-run rule across 100,000
  sets and 200,100 logical paths in 2,766.08 ms, read its history page in 0.17 ms, reversed it in
  703.08 ms, and retained 46,256,128 private bytes. Those results remain below the explicit 20 s
  apply, 10 s reversal, and 128 MB retained-growth ceilings, but are regression evidence only and
  do not close the representative-hardware gate.
- Real interactive Debug and Release `Invoke-WindowsSmoke.ps1 -SkipBuild` runs passed direct-worker
  apply/later-manual-override/restart/reversal coverage and accessible WPF completed-run preview,
  confirmation focus, application, manual-preserving reversal, and announcements. Every disposable
  file and directory remained present. PowerShell parsing and `git diff --check` passed. No path was
  live-validated, no excluded cloud placeholder was read or hydrated, and no deletion was exposed,
  scheduled, or executed.
- Representative-hardware warm-query evidence, physical Narrator/NVDA verification, OS
  high-contrast verification, and multi-monitor DPI-transition verification remain independently
  open Milestone 8 gates.

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

Status: The first bounded slice is implemented and accepted. The second-slice design is refined and
its first strictly non-mutating foundation is implemented below. It adds durable operation-domain
state and reconstruction, but deliberately exposes no scheduling, Recycle Bin/Shell mutation, real
partial execution, recovery action, or Milestone 12 working-state mutation.

#### Refined first-slice implementation plan (2026-08-20)

##### User story and non-goals

A user who has reviewed at least one removal can inspect an exact plan-revision summary, explicitly
confirm a metadata-and-content preflight, watch bounded cancellable progress, and understand every
ready, changed, missing, unavailable, or safety-conflict observation after restart. Completing this
slice proves only that the frozen targets matched their immutable scan snapshots during that
preflight. It does not authorize, queue, schedule, or perform deletion, invoke Windows Shell or the
Recycle Bin, promise that observations remain current, or move results into Milestone 12
changed/resolved states.

##### Frozen plan and sources of truth

- `preflight.start` requires a completed run, an active non-empty review plan, and its exact
  `expectedReviewRevision`. In one Rust-owned SQLite transaction it revalidates the combined manual
  file, manual exact-folder, and active rule-produced effective decisions, enforces the accepted
  overlap/survivor rules, and materializes a read-only preflight snapshot.
- The snapshot records `planId`, `reviewRevision`, an exact deterministic `snapshotSignature`, and
  fixed logical-path, physical-item, folder, byte, and affected-group counts. Later review edits do
  not rewrite it. Queries return the current review revision and `isCurrent`; a mismatch invalidates
  confirmation and any future execution handoff without mutating historical observations.
- Materialized target rows refer only to immutable `scanned_file`, duplicate-group, and exact-folder
  snapshots. Effective review decisions select actions, but `scanned_file.marked_deleted` and
  `deletion_plan` are never consulted. Rule configuration, rule-application provenance, manual
  review rows, preflight snapshots/observations, future operation state, and scan history remain
  separate tables and contracts.
- File and exact-folder removals are flattened to physical-file targets for I/O. Logical aliases and
  contributing file/folder decisions remain bounded source rows. A stable file identity is the
  physical key when present; otherwise the normalized snapshot path is the conservative key.
  Folder rows retain their structural and verified fingerprints so the complete current tree can be
  compared without turning the observation into review or execution state.

##### Exact validation semantics

- Before any target access, validation compares the canonical snapshot path against the immutable
  run's effective location exclusions. A path inside an excluded registered/manual subtree is a
  `conflict/excluded_location` observation and is never opened, enumerated, canonicalized, hashed,
  or passed to a native identity API.
- On Windows, a non-opening attribute/reparse classification precedes ordinary metadata. A Cloud
  Files placeholder, recall-on-access entry, reparse point, symlink, junction, directory at a file
  path, or file at a folder path becomes a structured conflict. Placeholder contents are never
  opened or hashed, and this slice has no hydration opt-in.
- A physical file is `ready` only when its path is an ordinary file and stable identity, exact byte
  length, nanosecond Unix modified time, and complete xxHash64 all equal the scan snapshot. Missing
  stable identity in either snapshot or observation is a conflict rather than a path-only success.
  Hashing is cancellable in bounded chunks and metadata/identity are checked again after hashing;
  a before/after mismatch is `changed/during_validation`.
- `missing` means the exact path is absent. `changed` means an accessible ordinary file differs in
  identity, size, timestamp, or hash. `unavailable` means access or I/O failed without evidence that
  the path is absent. `conflict` covers wrong type, unsafe link/reparse/placeholder state, excluded
  location, snapshot inconsistency, physical-target alias disagreement, folder-tree mismatch, or a
  survivor invariant failure. Every non-ready row has a stable reason code and bounded safe detail;
  OS error numbers may be retained, but error text is not used as the contract.
- All aliases selected for removal from one physical key must still resolve to the same identity.
  The physical content is hashed once per validation generation and the observation is projected to
  its logical source paths. Divergent aliases conflict the physical target instead of silently
  selecting one path.
- For every affected duplicate group, at least one independently accessible physical survivor not
  selected for removal is revalidated with the same identity/type/size/time/hash contract. A hard
  link alias to a removal target is not a survivor. A target is not ready when its group has no ready
  survivor, even if the target itself still matches.
- A removed exact-folder copy is ready only when bounded re-enumeration finds exactly the immutable
  relative file set, no unexpected directories/reparse entries/placeholders, and every physical file
  validates. Added, missing, renamed, changed, inaccessible, or type-changed descendants produce a
  folder conflict. At least one intact non-removed folder copy and every duplicate-file survivor
  invariant must still hold.

##### Schema v9, lifecycle, and recovery

- Schema v9 adds preflight headers, physical file/folder items, and logical-source provenance with
  foreign keys and bounded-query indexes. The header's unique operation ID is the replay record.
  The v8-to-v9 migration is one transaction, rolls back on failure, preserves all historical rows,
  and keeps newer unknown schema rejection.
- Header states are `pending`, `running`, `cancelling`, `completed`, `cancelled`, `interrupted`, and
  `failed`. Item outcomes are append-safe observations for one immutable validation generation;
  preflight never writes review decisions, scan rows, rule rows, or future operation rows.
- `operationId` plus the complete canonical start payload is idempotent. Exact replay returns the
  existing preflight; payload reuse returns `operation_conflict`. Only one preflight validation may
  perform filesystem I/O in a worker process, and scan start is rejected while it is active.
- The worker persists `running` before launching background I/O, updates fixed counters in batches,
  checks cancellation before every item, directory batch, and hash chunk, and publishes coalesced
  progress no faster than ten updates per second. `preflight.cancel` is idempotent and affects only
  the named active preflight.
- Worker startup reconciles abandoned `running`/`cancelling` headers to `interrupted`; already
  committed item observations and summaries remain queryable. Retry deliberately creates a new
  operation and validation generation from the still-current review revision rather than mixing
  new observations into the interrupted generation.
- `preflight.get` returns the fixed header/summary and current-revision comparison.
  `preflight.item.page` uses an opaque signature-bound cursor, stable outcome/kind/path/id ordering,
  a maximum page size of 200, and no filesystem access. Completed, cancelled, interrupted, and
  failed generations remain reconstructible after restart.

##### Protocol, Core, Infrastructure, and WPF

- Worker methods are `preflight.start`, `preflight.get`, `preflight.item.page`, and
  `preflight.cancel`; events are coalesced `preflight.progress` plus one terminal
  `preflight.completed`, `preflight.cancelled`, or `preflight.failed`. Allow-listed DTOs reject
  unknown fields, oversized IDs/cursors, invalid limits, stale revisions, and malformed replays with
  structured codes such as `review_generation_conflict`, `preflight_busy`,
  `preflight_not_found`, `preflight_not_cancellable`, and `operation_conflict`.
- Rust owns snapshot construction, SQLite, filesystem validation, hashing, physical de-duplication,
  survivor evaluation, folder comparison, status aggregation, paging, and progress. Core owns only
  contracts and the preflight view model. Infrastructure owns JSONL transport. No filesystem,
  database, worker-process, or future Shell work runs on the WPF dispatcher.
- The WPF surface is a `Preflight` workspace for the selected completed run. Before start it presents
  immutable plan/revision counts and says plainly that no files will be deleted. The primary action
  opens an accessible confirmation naming metadata, hash, and possible local disk reads; it never
  describes Recycle Bin execution.
- While running, the surface shows one aggregate progress bar, checked/total text, current phase,
  and `Cancel preflight`. Focus moves to progress after confirmation and to the summary heading on a
  terminal result. Escape does not silently cancel; cancellation requires the named button and a
  confirmation once validation has started.
- The terminal summary separates ready, changed, missing, unavailable, and conflict counts. A
  virtualized bounded list exposes outcome, target kind, path, stable explanation, and source
  context with keyboard paging. `Enter`/`Space` invoke buttons, `Ctrl+Home` returns to the summary,
  and retry is enabled only for a current review revision.
- UI Automation names never encode meaning by color alone. Start, cancellation, terminal summary,
  stale-revision invalidation, paging, and structured failures raise repeatable coalesced
  notifications. High-contrast resources and supported narrow/DPI layouts use existing dynamic
  system brushes and reflow patterns; physical Narrator/NVDA, OS high-contrast, and multi-monitor
  DPI gates remain independently open.

##### Bounds and acceptance

- Snapshot construction and paging never materialize the full plan in WPF. Rust materializes only
  the frozen target/survivor/source set in its transaction, validates one physical item at a time,
  and pages observations from indexed durable rows. The Core cache retains at most five 100-item
  pages and rejects stale run/preflight/query generations. Cancellation disposes the prior query
  token.
- Automated disposable-fixture coverage must include v8 migration/rollback/newer-schema rejection,
  empty/stale/idempotent starts, recovery, paging signatures, cancellation, changed/missing/wrong
  type/reparse/placeholder/excluded paths, hard-link alias de-duplication and survivor loss,
  file/folder overlap, exact-folder tree drift, no-hydration seams, worker restart, bounded caches,
  stale generations, confirmations, focus, keyboard behavior, announcements, and structured errors.
- Acceptance requires Debug and Release Rust/.NET matrices and real non-deleting WPF smoke over
  disposable files. Before/after fixture bytes, identities, timestamps, allocation/placeholder state,
  and provider transfer counters must remain unchanged where a real Cloud Files fixture is used.
  Development-host timing is regression evidence only and does not close representative-hardware or
  physical accessibility gates.

#### Refined second-slice design plan (2026-08-20)

##### User story, whole-plan boundary, and sources of truth

A user with one current, successfully completed preflight can inspect one fixed execution summary,
confirm an immediate Recycle Bin operation for exactly that reviewed-plan revision, and later see
durable per-item outcomes even when Windows completes only part of the work. This slice is designed
as whole-plan admission: it never silently drops an ineligible removal, never lets the user execute
only the convenient subset, and never schedules work for later. Partial state exists only because
Shell work, provider failure, cancellation, process loss, or root loss can occur after mutation has
started; it is not a selectable execution mode.

The operation binds `runId`, active `planId`, exact `reviewRevision`, `preflightId`, preflight
`snapshotSignature`, and a deterministic operation-intent signature. It never consults
`scanned_file.marked_deleted` or legacy `deletion_plan`. Reusable rule configuration, immutable
rule-application provenance, later manual review choices, immutable preflight observations,
operation intent/results/recovery evidence, future Milestone 12 live state, and immutable scan
history remain distinct Rust-owned sources. Operation results do not rewrite any of the others.

Preparation is non-destructive. It may create only a future operation intent and run bounded,
non-mutating eligibility checks. No path is sent to Windows Shell and no operation can enter a
submitted state until the accessible final confirmation succeeds. Once submitted, execution starts
immediately; background scheduling, permanent delete, automatic restore, and user-selected subset
execution remain unavailable.

##### Exact revision binding and freshness leases

- `recycle_operation.prepare` will require the latest generation for the run to be `completed`, to
  address the still-active plan and exact current review revision, and to contain no pending item.
  A later preflight generation in any state, a changed review revision, or another active
  filesystem mutation invalidates preparation. Exact replay is resolved before freshness checks.
- The initial policy is deliberately short-lived: preparation must begin within five minutes of
  the preflight `completedAt`; a wall-clock rollback, sleep/resume discontinuity, app restart, or
  inability to prove that interval expires the generation. In-process elapsed time uses a monotonic
  clock; persisted UTC is used only to fail closed across restart. Expiration never changes the old
  observations and asks the user to run preflight again.
- Preparation freezes counts and creates bounded batches, then Infrastructure performs only the
  planned non-opening Recycle Bin capability classification. The worker issues a fixed
  confirmation summary only after all classifications are durably reported. That summary expires
  after 60 seconds; any revision, generation, capability, root-availability, or app-lifecycle change
  closes it and requires a fresh preflight rather than extending the lease.
- The explicit confirmation must be submitted to the worker within the same 60-second lease. The
  worker rechecks plan/preflight ownership and commits `submitted` before returning an execution
  token. Infrastructure must receive that acknowledged token before requesting a batch and must
  never infer submission from a cancelled or disconnected request.
- A review/rule mutation while an operation is `prepared` or `awaiting_confirmation` expires that
  intent without changing the review edit. Once `submitted` is durably acknowledged, mutations of
  the bound plan and deletion of its run/session are rejected until a non-ambiguous terminal state;
  execution keeps the exact frozen revision rather than adopting later intent. A
  `recovery_required` operation retains this lock until an explicit future recovery workflow
  resolves or administratively preserves its evidence.
- Before each batch is released, Rust repeats excluded-location, ordinary-item, exact identity,
  type, size, nanosecond time, complete-hash, folder-tree, and affected-survivor checks against the
  frozen snapshots. The batch admission expires after 30 seconds; expiry, suspension, or delay
  reruns admission. Infrastructure repeats the positive Recycle Bin capability decision and
  non-opening reparse/placeholder classification before Shell item creation, then repeats
  identity/type/size/time checks from the `PreDeleteItem` callback. The remaining path-swap interval
  inside Windows Shell is an explicit residual TOCTOU risk, not a promise of filesystem atomicity.

##### Eligibility and fail-closed admission

- Every `remove` item must have a `ready` observation in the bound preflight and must pass the
  fresh batch admission. `changed`, `missing`, `unavailable`, and `conflict` removals block the
  whole operation; they are never treated as already resolved, skipped, or eligible for retry under
  the old generation. Non-ready survivor observations may be disclosed, but every affected file
  group must still have a newly ready independent physical survivor and every affected folder group
  a newly ready intact folder copy.
- A target is `non_recyclable` when Infrastructure cannot positively establish support for a
  Recycle Bin-only Shell operation for its current local volume/root, or when Shell item creation
  would require a provider, elevation, hydration, fallback, or interactive error decision. UNC,
  disconnected, unavailable, or unrecognized provider-backed roots fail closed in the first
  implementation; removable/mapped roots require explicit positive capability evidence rather
  than drive-letter inference. One non-recyclable target blocks confirmation for the whole plan.
- Excluded registered/manual subtrees are rejected lexically before any capability or target I/O.
  Offline/recall Cloud Files placeholders and every reparse/link target remain ineligible. The
  worker never places them in an operation batch, and Infrastructure independently refuses to call
  Shell item creation or `IFileOperation` for them. There is no hydration opt-in or permanent-delete
  fallback.
- Hard-linked removals are hashed and safety-checked once per physical identity, but Shell mutation
  is per selected directory entry: every selected alias has its own durable operation item and
  result. An alias outside the removal plan neither inflates the independent-survivor count nor is
  silently removed. Fixed summaries report logical paths, Shell entries, unique physical items,
  and physically de-duplicated bytes separately.
- An exact-folder action is eligible only when the entire root and every immutable descendant pass
  the fresh full-tree admission and all file/folder survivor invariants. Any drift blocks that
  folder and therefore the whole plan before submission. Existing file/folder and nested-folder
  overlap rules ensure no separate Shell item is queued inside that root.

##### Final accessible confirmation

The confirmation heading says **Move reviewed items to the Recycle Bin** and states that the action
starts now. Its fixed worker-owned summary names the session/run and review revision, preflight
completion time and expiry, logical removal paths, Shell entries, exact folders, unique physical
items, de-duplicated bytes, affected duplicate groups, and the number of distinct affected
locations. It labels the byte total as planned content, not guaranteed Recycle Bin allocation or
recoverability.

Affected configured roots/volumes and their Recycle Bin capability are available through a
signature-bound paged list in the confirmation region; the fixed heading names the first bounded
set and total, and `Review all affected locations` opens the virtualized list without materializing
the plan. The confirmation also names the frozen registered/manual exclusion count, says that zero
excluded paths or Cloud Files placeholders will be accessed, and blocks rather than hides any
eligibility failure.

The disclosure says plainly that Windows can recycle some items before another item fails or the
app closes, cancellation cannot undo completed Shell work, exact-folder recycling is not
transactional, Recycle Bin capacity/provider behavior can still fail, and the app does not promise
restore. `Move to Recycle Bin now` and `Cancel` are native buttons with deterministic tab order and
Space/Enter activation. Focus moves to the confirmation heading on entry and returns to the
invoking control on cancellation or expiry; Escape never confirms or silently cancels.

##### Provisional schema-v10 intent, states, and idempotency

A possible schema v10 will add only operation-domain records. The proposed `recycle_operation`
header snapshots the operation ID and canonical payload, run/plan/revision/preflight binding,
intent/confirmation signatures, policy/freshness versions, fixed counts/bytes/location summary,
state, timestamps, cancellation request, and bounded structured terminal detail.
`recycle_operation_batch` stores stable bounded ordinals, item-list signatures, admission expiry,
Shell-attempt identity, and batch state. `recycle_operation_item` stores one immutable top-level
Shell path/action plus physical group and logical provenance references. Separate append-safe
eligibility, result-report, and recovery records retain capability codes, per-item outcomes,
HRESULTs as unsigned numeric evidence, callback/aborted flags, report operation IDs, and attempts.
No operation table becomes review or live-state truth.

The header state machine is `prepared -> awaiting_confirmation -> submitted -> executing`, with
`cancelling` as an active stop request and terminal `expired`, `cancelled`, `completed`,
`partially_completed`, `failed`, or `recovery_required`. Batches are `pending`, `admitted`,
`shell_started`, `reported`, `skipped`, or `ambiguous`; items are `pending`, `recycled`, `failed`,
`cancelled`, or `unknown`. `completed` requires every item to be positively reported recycled.
`failed` means no mutation was reported and every item has a known non-success result.
`partially_completed` requires at least one recycled item and no unresolved ambiguity.
`recovery_required` covers an unreported `shell_started` batch, missing/inconsistent callbacks,
an unexpected non-Recycle-Bin outcome, or any other state where mutation may have occurred.

Preparation, confirmation, cancellation, batch begin, and result report each have bounded
idempotency keys and canonical payload signatures. Exact replay returns the durable original
outcome even after restart; reuse with different content is `idempotency_conflict`. A cancelled
client wait retains the same key and payload. A new operation ID never grants permission to repeat
a batch already marked `recycled`, `shell_started`, `reported`, or `ambiguous`.

##### Protocol, ownership, and dedicated STA execution

The planned worker surface is `recycle_operation.prepare`, `.eligibility.report`, `.confirm`,
`.get`, `.item.page`, `.batch.next`, `.batch.begin`, `.batch.report`, and `.cancel`, with bounded
coalesced progress and one terminal/recovery event. Exact method/DTO names remain provisional until
the protocol slice, but all requests use allow-listed fields, decimal strings for large byte
values, signature-bound cursors, 1 MiB framing, structured errors, and explicit replay markers.
The worker alone constructs intent/batches, owns SQLite, grants admission, serializes scan,
preflight, and operation I/O, aggregates outcomes, and decides terminal state.

Infrastructure owns the Windows-only executor and no product persistence. One long-lived dedicated
thread initializes COM as STA, creates one `IFileOperation` instance per batch, advises one
`IFileOperationProgressSink`, queues only the admitted top-level `DeleteItem` calls, calls
`PerformOperations`, always queries `GetAnyOperationsAborted`, unadvises, releases COM objects on
that thread, and reports the bounded result. Official Shell contracts require STA, make
`DeleteItem` declarative until `PerformOperations`, expose actual per-item HRESULTs through
`PostDeleteItem`, and allow a successful `PerformOperations` HRESULT even when work was aborted;
the design must preserve all four facts rather than infer success from the outer call alone.

The executor sets an explicit reviewed flag set including `FOFX_RECYCLEONDELETE`,
`FOFX_ADDUNDORECORD`, `FOF_NOCONFIRMATION`, `FOF_NOERRORUI`, `FOF_SILENT`, and
`FOFX_EARLYFAILURE`; it never accepts defaults or a permanent-delete fallback. A successful
`PostDeleteItem` counts as `recycled` only when the callback also supplies the newly created Recycle
Bin Shell item. A null recycled item, unsupported flag, Shell UI request, or inconsistent callback
is a critical unknown outcome that stops later batches. This follows the documented
[`IFileOperation`](https://learn.microsoft.com/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation),
[`SetOperationFlags`](https://learn.microsoft.com/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperation-setoperationflags),
[`PostDeleteItem`](https://learn.microsoft.com/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperationprogresssink-postdeleteitem),
and [`GetAnyOperationsAborted`](https://learn.microsoft.com/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperation-getanyoperationsaborted)
contracts.

##### Bounded batches, transport, and cancellation limits

- A normal batch contains at most 32 top-level Shell entries and must fit the existing protocol
  frame independently of logical-source detail. An exact-folder root is isolated in its own
  one-item batch. Hard-link alias entries may span batches but retain one physical-group key. WPF
  receives fixed summaries and at most five 100-row result/location pages, never operation batches
  or a run-wide result dictionary.
- Infrastructure must obtain a durable `batch.begin` acknowledgement before calling
  `PerformOperations`. If acknowledgement is lost, it does not call Shell. Once acknowledged, the
  worker records `shell_started` before mutation is possible. Infrastructure sends callback
  results in bounded chunks and one signed final report; report retry uses the same attempt/report
  IDs and can never queue Shell work again.
- Before submission, cancellation is immediate and terminal with zero mutation. After submission
  but before `batch.begin`, it skips every pending batch. Between batches it stops future work;
  already recycled items remain recycled and the terminal state reflects partial cancellation.
- After `PerformOperations` starts, the current Shell call is not treated as synchronously
  cancellable. A cancellation request means stop before the next item when `PreDeleteItem` can
  safely return a cancellation HRESULT, and always stop before the next batch. The app never kills
  the STA thread, releases live COM objects from another thread, or claims that completed work was
  undone. WPF changes the action text to `Stop after current Shell work` and announces the
  non-cancellable phase.

##### Shell outcomes, ambiguity, recovery, and survivor safety

Per-item mapping uses the `PostDeleteItem` HRESULT, recycled-item presence, `FinishOperations`
HRESULT, `PerformOperations` HRESULT, and `GetAnyOperationsAborted` flag together. Stable reason
codes distinguish user/system cancellation, access denied, sharing violation/locked item, root
disconnection, item disappearance, provider failure, unsupported recycling, unexpected permanent-
delete evidence, and unmapped Shell failure; localized Shell text is diagnostic only. Pending
items after an abort become `cancelled` only when no callback or other evidence suggests mutation;
otherwise they are `unknown`.

Every acknowledged `shell_started` batch lacking a complete accepted report at worker/app restart
becomes `ambiguous`, and its operation becomes `recovery_required`. Pending later batches remain
unsubmitted. The worker never replays that batch automatically, never converts source-path absence
into proof of app recycling, and never repeats an item whose outcome may be success. Infrastructure
may reconnect and idempotently resend a report it already captured; otherwise the UI preserves the
unknown items and directs the user to inspect the source locations and Recycle Bin. A future
explicit reconciliation design may resolve ambiguity, but this slice does not write Milestone 12
state or invent certainty.

After each fully reported batch, Rust performs operation-scoped rechecks of the still-required
survivors before admitting another batch. Survivor disappearance, identity/content drift, or an
unexpected hard-link relationship stops remaining work and creates a structured partial or
recovery result. These checks append operation recovery evidence only; they do not rewrite the
preflight, review plan, immutable run, or future live overlay. A failed selected hard-link alias is
reported separately, a successful alias is never repeated, and independently accessible survivors
are re-evaluated from physical identity rather than remaining path count.

Exact-folder admission is all-or-nothing only before Shell begins. The root is submitted as one
isolated Shell item so the Recycle Bin can preserve folder shape, but Windows may mutate descendants
before a provider/error/cancellation result. Only a positive root `PostDeleteItem` with a recycled
Shell item is complete success. Any failure, abort, callback gap, or missing recycled item is
reported as partial/unknown for the folder; the app does not delete leftover descendants, roll back
already recycled children, or call the folder resolved.

##### Accessible progress, results, and recovery UI

After confirmation, focus moves to an operation-progress heading. The view shows phase, batch and
item counts, planned versus positively recycled bytes, current affected location, a native stop
button, and a persistent explanation of whether stopping is immediate, between-batch, or waiting
for current Shell work. Progress and UI Automation notifications are coalesced to at most ten per
second; no Shell callback creates one dispatcher update.

Terminal focus moves to a summary heading that separately names recycled, failed, cancelled,
unknown, and not-submitted counts/bytes, affected locations, exact-folder outcomes, and whether
manual inspection is required. Virtualized pages expose path, source decision/provenance,
physical/hard-link context, stable explanation, numeric Shell code when useful, batch, and recovery
status. `Open Recycle Bin` is offered only after at least one positive recycled result and says that
restore is owned by Windows. Failed/partial/recovery summaries are assertive; ordinary progress and
success are polite. `Enter`/`Space`, deterministic tab order, `Ctrl+Home`, Escape behavior, high-
contrast resources, narrow reflow, focus restoration, and stale-generation rejection receive WPF
STA coverage. Physical Narrator/NVDA, OS high-contrast, and multi-monitor DPI remain operator gates.

##### Schema v10 migration, rollback, and compatibility

Rust migrates v9 to v10 in one `BEGIN IMMEDIATE`
transaction, creating operation tables, constraints, foreign keys, and bounded-query indexes before
setting `user_version = 10`. Failure leaves an unchanged valid v9 database; supported older schemas
still migrate in order and unknown newer schemas fail closed. No v9 preflight/review/scan row and no
legacy deletion field is rewritten or adopted. Explicit truncation and run deletion will remove
operation result/report/source rows in foreign-key order before their bound preflight and plan.

Before the first successful v10 open, the release workflow must take a consistent SQLite backup of
the database/WAL state and document its location. A previous binary cannot safely open v10 and
there is no in-place downgrade that drops operation evidence. Operational rollback therefore means
closing the app/worker and restoring the complete pre-migration v9 backup; after any Shell mutation,
rollback must preserve the v10 database and logs as evidence and use a copy for diagnosis rather
than erasing results. Protocol negotiation must prevent an older Windows client from driving a v10
worker operation surface. Schema v10 is now implemented; automated backup orchestration and every
real Shell mutation/recovery behavior remain outside this foundation.

##### Questions that remain implementation gates

- Confirm on supported Windows 11 filesystems the exact non-mutating evidence that positively
  establishes Recycle Bin capability; no undocumented drive-type heuristic may become execution
  authority. Until proven, an unclassified root remains non-recyclable.
- Measure whether five-minute preflight, 60-second confirmation, 30-second batch admission, and
  32-entry batches are safe and usable on the large disposable fixture. Tightening is compatible;
  loosening requires a documented safety review and versioned freshness policy.
- Verify with real disposable folders and hard links how `PostDeleteItem`, its recycled-item value,
  outer/finish HRESULTs, and the abort flag behave for successful, partial, locked, oversized,
  capacity-limited, and cancelled operations. The fake adapter cannot close this Shell-contract
  evidence gate.
- Decide after that evidence whether `FOFX_ADDUNDORECORD` remains in the final flag set. Any choice
  affects Windows' own undo integration only; the app still must not promise or implement restore.

##### Disposable verification and acceptance boundary

- Storage/protocol tests cover v9-to-v10 migration and injected rollback at every DDL phase,
  unknown-schema rejection, exact binding/freshness/expiry, latest-generation rules, canonical
  replay/conflict, operation locks, state transitions, batch/item signatures, paged results,
  report replay, and every crash boundary before/after `shell_started` and accepted result commit.
- A fake Shell adapter injects queue, `PreDeleteItem`, `PostDeleteItem`, `FinishOperations`, outer
  HRESULT, abort flag, missing callback, provider, lock, disconnect, null recycled-item, and delayed
  cancellation outcomes. Infrastructure tests assert the dedicated STA apartment/thread and COM
  lifetime, exact flags, one advised sink, 32-item/one-folder batches, no call before durable begin,
  no retry after ambiguous start, and no Shell item creation for excluded/reparse/placeholder paths.
- Disposable filesystem tests cover identity/metadata/hash drift in every freshness window,
  path-swap seams, hard-link aliases across batches, survivor loss after a successful batch,
  exact-folder pre-admission drift and partial callbacks, inaccessible/locked files, disconnected
  removable/mapped/UNC roots, and bounded failure reporting. Cloud seams assert zero target access;
  real-provider no-hydration remains a separate unclosed operator acceptance gate.
- Core tests retain operation IDs across cancellation/restart, bound all caches/generations, and
  reject late reports. WPF STA tests cover the complete disclosure, all counts/bytes/locations and
  exclusions, expiry, keyboard/focus/announcements, non-cancellable wording, structured failures,
  partial/unknown summaries, and `Open Recycle Bin` eligibility without invoking real Shell.
- Protocol smoke stays non-mutating with a fake executor. A separate interactive operator workflow
  may later use only generated disposable local fixed-drive fixtures, verify before/after identities
  and survivors, inspect positive items in the real Recycle Bin, exercise locked/failure/cancel/
  restart boundaries, and clean up explicitly. It must never target user files, excluded cloud
  roots, or permanent delete. Debug/Release Rust/.NET matrices, protocol/publish smoke, focused
  formatting/parsing/diff checks, and documented recovery are required before exposure.

This design does not accept or implement deletion. Implementation acceptance additionally requires
bounded large-plan regression evidence, but that evidence cannot be claimed as representative-
hardware performance. It cannot close real-provider preflight/operation no-hydration acceptance or
the independent Milestone 8 representative-hardware, physical Narrator/NVDA, OS high-contrast, and
multi-monitor DPI-transition gates without new qualifying operator evidence.

##### Implemented non-mutating foundation (2026-08-20)

- Schema v10 transactionally migrates v9 into separate operation intent, bounded batch/item,
  canonical report-replay, and per-item recovery records. Tests cover migration success, rollback,
  older migration chains, and unknown-newer rejection. Downgrade requires restoring a complete
  closed-process v9 database/WAL/SHM backup; there is no in-place downgrade.
- Rust prepares only against the latest current completed preflight and exact immutable review
  revision. Every removal must be `ready`; one non-ready/non-recyclable item fails the whole plan.
  Canonical IDs/signatures make preparation and reports exactly replayable. Review/provenance/new-
  preflight mutations expire unsubmitted intent; submitted or ambiguous operations lock them.
  Run/session deletion stays blocked while operation evidence is active or ambiguous.
- The implemented five-minute preparation, 60-second confirmation, 30-second submission/admission,
  and 32-file batch limits are explicitly provisional. Confirmation and batch begin recheck the
  current revision/latest preflight. Exact folders are isolated; file entries are bounded and
  hard-link-aware. No fresh target access or exact-folder revalidation was added in this foundation.
- Startup expires intent that never reached submission. A persisted `shell_started` test boundary
  is conservatively reconstructed as `recovery_required`; pending items become `unknown` with
  recovery records. The lock prevents a retry from repeating a mutation that might have completed.
  This is state-machine evidence only, not a real Shell recovery workflow.
- Worker DTOs reject unknown fields and expose bounded prepare/get/item/eligibility/confirmation/
  cancellation/batch/report transitions with structured errors. Every response advertises
  `executorEnabled:false`. Result transitions support deterministic injection and cannot perform
  filesystem or Shell work.
- Core adds bounded operation contracts, a five-page/100-item cache, linked cancellation
  generations, and stale operation/page rejection. WPF reconstructs fixed counts, bytes, locations,
  exclusions, partial/ambiguous risk, progress/results, structured errors, announcements, keyboard
  paging, and focusable headings, while `CanSubmit` is unconditionally false and no “Move now”
  action exists.
- Infrastructure adds only an injected disabled capability executor. It deterministically reports
  `non_recyclable/executor_disabled` and does not inspect a path or call any Shell API. No
  `IFileOperation`, `SHFileOperation`, move, delete, recycle, scheduling, hydration, or permanent
  deletion implementation was exposed by this foundation.

This foundation does not close positive Recycle Bin capability, real `PostDeleteItem`/abort and
provider mapping, `FOFX_ADDUNDORECORD`, residual Shell TOCTOU, real-provider no-hydration, or
representative large-plan operation performance. It also does not close representative-hardware
warm queries, physical Narrator/NVDA, OS high contrast, or multi-monitor DPI transition.

##### Implemented separately gated executor slice (2026-08-20)

- Rust `batch.next` revalidates each bounded target against immutable type, identity, size,
  nanosecond time, and complete content hash; exact-folder batches rerun the complete tree check;
  and affected physical-file and exact-folder survivors are revalidated before a batch becomes
  `admitted`. Admission failure durably fails the addressed item, marks remaining work not
  submitted, and never invokes Shell. A 30-second expiry returns the batch to `pending` so the
  checks must run again.
- Admitted file entries project only immutable identity/size/time for Infrastructure to repeat a
  no-content-read check in `PreDeleteItem`. Selected hard-link aliases remain separate Shell
  entries; revalidation uses the specific pending alias while independent survivors remain
  physical-identity based. Exact folders remain isolated one-item Shell batches.
- `WindowsRecycleOperationExecutor` owns one long-lived dedicated STA thread and creates one
  `IFileOperation` per batch. It repeats non-opening offline/recall/reparse/type classification and
  positive local-root Recycle Bin evidence, queues declarative `DeleteItem` calls, obtains the
  caller's durable `batch.begin` acknowledgement, then calls `PerformOperations`. A lost or failed
  acknowledgement never starts Shell mutation.
- Capability is positive only for a local fixed/removable root for which documented
  `SHQueryRecycleBinW` succeeds. UNC, remote, missing, offline/recall, reparse, wrong-type,
  unavailable, and unrecognized roots fail closed. This does not generalize to mapped drives, real
  Cloud Files providers, or other provider namespaces.
- Flags are explicit `FOFX_RECYCLEONDELETE`, `FOF_NOCONFIRMATION`, `FOF_NOERRORUI`, `FOF_SILENT`,
  and `FOFX_EARLYFAILURE`. `FOFX_ADDUNDORECORD` is intentionally omitted pending evidence review;
  there is no permanent-delete fallback or permanent test cleanup.
- One advised progress sink records `PreDeleteItem`, `PostDeleteItem`, `FinishOperations`, outer
  `PerformOperations`, and `GetAnyOperationsAborted` independently. Positive recycling requires a
  successful per-item HRESULT and non-null recycled Shell item. Missing/inconsistent callbacks and
  post-start exceptions become `unknown`; access, sharing, disappearance, root, cancellation, and
  unmapped HRESULTs retain stable reason codes plus numeric evidence.
- Cancellation can prevent queued STA work before it starts. After durable Shell start it is
  observed at `PreDeleteItem` and always returns a reportable batch result; the task/STA is never
  killed. The executor never retries a started batch, preserving ambiguous-start non-retry.
- Focused tests cover STA ownership, positive capability without mutation, offline/missing no-
  content-access classification, expired admission, target and file/folder survivor drift, and
  disabled-executor non-acknowledgement. Opt-in real-Recycle-Bin acceptance proved successful hard-
  link alias and exact-folder recycling with byte-identical survivors and post-ack cancellation
  that left its source unchanged. A locked source produced documented copy-engine HRESULT
  `0x80270027`, mapped to `sharing_violation`, and also remained byte-identical.

Production still injects `DisabledRecycleOperationCapabilityExecutor`, every worker response still
reports `executorEnabled:false`, WPF `CanSubmit` remains false, and no `Move to Recycle Bin now`
action exists. This slice does not complete Milestone 11. Real-provider no-hydration,
locked/capacity/provider mappings, final provisional constants, `FOFX_ADDUNDORECORD`, residual
path-swap TOCTOU, representative large-plan performance, accessible final-confirmation/operator
wording, and recovery-resolution UI remain gates.

##### Implemented operator/provider evidence foundation (2026-08-20)

- `Invoke-WindowsRecycleBinAcceptance.ps1` now runs the non-mutating deterministic executor
  contract by default and writes a versioned JSON matrix, Markdown report, command logs, host
  context, and TRX evidence beneath the ignored `artifacts/windows-recycle-bin-acceptance/` tree.
  Missing physical/provider prerequisites remain `not_run` or `open`; every report fixes
  `productionEnabled:false` and `milestone11Complete:false`.
- Stable automated coverage now locks every documented cancellation, access, sharing,
  disappearance, disconnect, capacity, unsupported, long-path, Recycle Bin, provider, and unmapped
  Shell HRESULT reason code. It also locks the exact current flag set and proves
  `FOFX_ADDUNDORECORD` remains omitted. These are contract regressions only, not fabricated real
  Shell/provider outcomes.
- A separately gated registered-provider test accepts only explicit root/local/offline fixtures,
  proves the exact Cloud Files registration, and compares non-opening attributes, logical and
  allocated sizes, timestamps, provider process membership, and provider transfer counters before
  and after executor inspection. It never crawls the root, opens either file, reads content, or
  submits the placeholder to Shell.
- `docs/windows-recycle-bin-acceptance.md` records exact local mutation, provider no-hydration,
  failure, disconnect, capacity, ambiguous-start, warm-query, large-plan/constants,
  `FOFX_ADDUNDORECORD`, and physical accessibility procedures plus their evidence boundaries.
- Three collector runs on the current 6-logical-processor/31.9-GiB Windows 11 development host show
  why the warm-query gate remains open. Group p95 was 93.50 ms initially, then 115.82 and 154.63 ms
  after the hardening workload; the last two fail the 100 ms ceiling. The final run measured group
  p50/p99 of 77.23/401.62 ms, selected-root/drive facet p95 of 51.19/43.85 ms,
  review-plan/group p95 of 30.23/4.88 ms, and 983,040 bytes retained private growth. All three are
  regression evidence, none is newly accepted as representative-hardware evidence, and none is
  large-plan Shell-operation performance.
- Release hardening now serializes the solution test projects with `-m:1`. Two concurrent runs
  reproduced a 16-17-second existing WPF STA dispatcher timeout while the exact test passed in about
  two seconds alone and the complete serialized Release solution passed. Serialization matches the
  accepted matrix procedure and prevents loaded Infrastructure tests from creating a false WPF
  failure; it does not loosen the WPF test's timeout or assertions. The serialized run also exposed
  a race where the optimized worker could finish the recovery test's 6-MiB fixture before `Kill`;
  that disposable fixture now includes a 1-GiB sparse file, making interruption deterministic with
  negligible allocation. Focused Debug/Release recovery tests and final Release hardening passed.

No qualifying provider fixture, controlled access-denied/disconnect/capacity/path-swap/process-loss
campaign, representative large-plan operation profile, or physical accessibility environment was
available merely by adding this tooling. Those rows remain open and the provisional constants and
Windows Undo decision remain unaccepted.

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

## Recommended Next Implementation Slice

Keep the remaining physical Narrator/NVDA, high-contrast, multi-monitor DPI, and representative-
hardware Milestone 8 procedures operator-gated; they can close independently from later review
work. The non-mutating foundation and separately gated dedicated-STA executor are implemented, but
the disabled production composition remains intentional. Use the evidence collector and dedicated
operator guide to collect real local access/disappearance/capacity and representative provider
evidence, run the explicit-fixture Cloud Files no-hydration procedure, profile a representative
large admitted plan, decide
`FOFX_ADDUNDORECORD`/the provisional constants, and complete operator review of final confirmation,
progress, cancellation, partial/unknown, and recovery wording. Do not expose `Move to Recycle Bin
now` or replace `executorEnabled:false` until those gates and ambiguous-start non-retry are accepted.
Keep
Milestone 12 changed/resolved mutation separate, and preserve rule configuration, application
provenance, manual review state, preflight observations, operation state, and immutable scan
history as distinct sources of truth.

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
| 2026-08-19 | Refined and accepted the bounded exact-duplicate-folder group-query screen-reader announcement slice after focused storage/worker/Core/STA coverage, the final Debug/Release Rust and serialized .NET matrix, and real Debug/Release WPF smoke passed; loaded-peer testing also found and fixed the accepted duplicate-file group-error `Border` peer gap. The final .NET configurations each passed 48 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | No richer filter has both a small complete contract and its required versioned mapping or boundary-aware range indexes. Raise repeatable `ActionCompleted`/`ActionAborted` notifications only for displayed current-generation exact-folder group results and failures, keep prefetch/cancellation/stale generations silent, and provide a generic peer only for status elements lacking one. Accepted protocol, SQL/indexes, cursors, cache bounds, cancellation/generations, prefetch, stale-response handling, virtualization, focus, and Milestone 8/10/11 boundaries remain unchanged. Remaining gates are further explicitly indexed filters, exact-folder member-query and broader screen-reader plus supported minimum-size/multi-DPI review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-19 | Refined and accepted the bounded exact-folder member-query screen-reader announcement slice after focused storage/worker/Core/loaded-peer STA coverage, the full serialized Debug/Release Rust and .NET matrix, and real Debug/Release WPF smoke passed; each final .NET configuration passed 50 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | No richer filter has both a small complete contract and its required versioned mapping or boundary-aware range indexes. Raise repeatable `ActionCompleted`/`ActionAborted` notifications only for displayed current-generation exact-folder member results and worker failures, including displayed prefetched-cache pages, while non-displayed prefetch, Explorer-action errors, cancellation, and stale generations remain silent. Accepted protocol, SQL/indexes, cursors, five-page cache, cancellation/generations, two-page prefetch, stale-response handling, virtualization, focus, and Milestone 8/10/11 boundaries remain unchanged. Remaining gates are further explicitly indexed filters, the rest of the broader screen-reader and supported minimum-size/multi-DPI review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-19 | Refined and accepted the bounded exact-folder minimum-width filter-reflow accessibility slice after reproducing the defect in a focused 620-DIP STA regression, then passing focused storage/worker/Core/STA coverage, the full serialized Debug/Release Rust and .NET matrix, and real Debug/Release WPF smoke; the unchanged 100,000-group regression completed in 3.03 seconds Debug and 1.20 seconds optimized Release, and each .NET configuration passed 50 Core, 22 Infrastructure, and 3 WPF tests with the real-provider test intentionally skipped. | No richer filter has both a small complete contract and its required versioned mapping or boundary-aware range indexes, and the broader screen-reader audit found no smaller regression outside accepted gates. Move the exact-folder heading above a wrapping filter panel and wrap its explanation so path, minimum-size, and Apply controls stay inside the supported 620-DIP workspace without changing protocol, SQL/indexes, cursors, paging, five-page caches, cancellation/generations, two-page prefetch, stale-response handling, virtualization, focus, announcements, or Milestone 8/10/11 boundaries. Remaining gates are further explicitly indexed filters, the rest of the broader screen-reader and supported minimum-size/multi-DPI review, and representative-hardware warm-query/bounded-memory profiling. |
| 2026-08-19 | Added an explicit 100-sample Release profile for the 100,000-group read-only workspace and reconciled the Milestone 8 boundary with concrete keyboard, screen-reader-contract, high-contrast, minimum-width, and DPI findings. The current host passed both facet p95 targets and grew private memory by 815,104 bytes across 300 queries, but its 140.47 ms group/summary p95 missed the 100 ms target. | Close the bounded-memory gate through measured process growth plus fixed cache/collection/virtualization regressions. Keep the warm group/summary and physical Narrator/NVDA, high-contrast, and multi-monitor DPI gates explicitly open with exact operator procedures. Do not make richer filters mandatory without their full mappings/indexes/contracts, and let the read-only boundary close independently of Milestone 10 durable decisions and Milestone 12 live state. |
| 2026-08-19 | Refined and accepted the first Milestone 10 durable manual-file-decision slice: schema v5, snapshot-backed `Keep`/`Remove`/`Undecided`, idempotent revision-checked worker mutations, bounded plan/group/member summaries, hard-link-aware survivor enforcement, accessible WPF controls, restart persistence, and real Debug/Release non-deleting smoke. | Establish a separate reversible review source of truth without using `marked_deleted`/legacy deletion plans, touching live files or excluded cloud placeholders, or exposing deletion. The new plan/group queries met their warm targets and memory stayed bounded; the independently open Milestone 8 group/drive warm-query and physical accessibility gates remain explicit. |
| 2026-08-19 | Refined and accepted the second Milestone 10 manual exact-folder-decision slice: transactional schema v6, separate folder snapshots/command replay, shared revision and cross-workspace invalidation, nested/suppressed/file overlap and hard-link safety, deduplicated combined summaries, accessible WPF controls, restart persistence, a 100,000-group profile, and real Debug/Release non-deleting smoke. | Keep folder review distinct from file decisions and execution while preserving at least one intact exact-folder copy and one physical file survivor. The isolated development-host profile measured 6.40 ms combined-plan p95, 14.50 ms folder-page p95, and no observed private-memory growth; it is regression evidence only, so the representative-hardware and physical accessibility gates remain open. The next design slice is a bounded, read-only ordered-preferred-root rule preview. |
| 2026-08-19 | Refined and accepted the third Milestone 10 slice: transactional schema v7 named ordered-root rules, revision-bound read-only preview over selected-set/current-filter/completed-run scopes, manual file/folder precedence, virtual overlap/survivor enforcement, hard-link-aware de-duplication, bounded Core caching and stale-response rejection, accessible WPF explanations, restart reconstruction, and real Debug/Release non-deleting smoke. | Persist reusable rule metadata with preview while keeping application deferred and separate from manual decisions, live state, and execution state. The isolated optimized 100-sample fixture evaluated 100,000 real sets/200,100 logical paths at 870.42 ms p95 with 2,199,552 bytes retained private-memory growth, within its development-host ceilings; it is regression evidence only. The next slice is design-first, idempotent rule application/reversal provenance without deletion or live validation; representative-hardware and physical accessibility gates remain open. |
| 2026-08-19 | Refined and accepted the fourth Milestone 10 slice: transactional schema v8 rule-application provenance, manual-revision precedence, exact preview-bound idempotent apply, fixed-summary history plus bounded detail, isolated replayable reversal, shared revision invalidation, accessible WPF confirmations, restart recovery, and real Debug/Release non-deleting smoke. | Keep reusable rules, rule-produced decisions, manual review choices, live state, and execution state separate while making an exact reviewed rule outcome durable and reversibly attributable. The isolated optimized 100,000-set/200,100-path profile completed apply in 2,766.08 ms and reversal in 703.08 ms with 46,256,128 retained private bytes, within its development-host ceilings only. The next slice is design-first bounded Milestone 11 preflight without scheduling, Recycle Bin interaction, deletion, or Milestone 12 changed/resolved UI; all four independent Milestone 8 operator gates remain open. |
| 2026-08-20 | Refined the first bounded Milestone 11 slice: schema v9 immutable review-revision snapshots, exact physical-file/folder and survivor validation, cloud-placeholder no-open behavior, durable cancellable generations, idempotent commands, recovery, paging, bounded Core caching, and an accessible non-deleting WPF workspace. | Make plan-time validation independently reviewable and restart-safe while keeping observations separate from rules, provenance, manual review, scan history, future execution, and Milestone 12 live state. Scheduling, Shell/Recycle Bin APIs, deletion, partial execution/recovery, changed/resolved mutation, and the four independent Milestone 8 gates remain explicitly out of scope. |
| 2026-08-20 | Implemented and accepted the first bounded Milestone 11 preflight slice: transactional schema v9, immutable review-revision snapshots, exact metadata/identity/time/hash and folder-tree checks, hard-link-aware physical targets and survivor re-evaluation, excluded-location and Cloud Files no-open classification, idempotent worker lifecycle/recovery/paging, bounded Core caching, accessible WPF confirmation/progress/results, and real Debug/Release non-deleting smoke. | Establish durable current-filesystem observations without turning them into review truth, execution authority, or Milestone 12 live state. All disposable fixtures remained present and unchanged; the four independent Milestone 8 operator gates remain open. The next slice is design-only refinement of a revision-bound Recycle Bin operation contract before any scheduling, Shell mutation, deletion, or partial-execution recovery is exposed. |
| 2026-08-20 | Refined the second Milestone 11 design as a revision/preflight-bound, freshness-leased, whole-plan Recycle Bin contract with fail-closed eligibility, accessible final confirmation, provisional schema-v10 intent/batch/result/recovery records, dedicated-STA `IFileOperation` ownership, bounded Shell batches, explicit cancellation limits, per-item result mapping, crash ambiguity, hard-link and exact-folder safety, and migration/rollback/test gates. | Keep the accepted v9 preflight immutable and keep rule configuration, application provenance, manual review, operation evidence, future live state, and scan history separate. This is design only: no schema/protocol/operation/UI implementation and no Shell mutation were added. Start implementation with the non-mutating durable contract and injected fake executor; expose the real Recycle Bin executor only in a later reviewed slice. Real-provider no-hydration, representative large-plan performance, and the four independent Milestone 8 operator gates remain open. |
| 2026-08-20 | Implemented the strictly non-mutating second-slice foundation: transactional schema v10 intent/batch/item/report/recovery evidence, exact revision/latest-preflight binding, provisional freshness and batch bounds, fail-closed eligibility, canonical replay, operation locks, restart ambiguity, bounded worker/Core contracts, disabled Infrastructure capability injection, and accessible read-only WPF reconstruction. | Establish durable operation-domain contracts without granting filesystem or Shell mutation authority. No Shell deletion API or path inspection was added and `CanSubmit` remains false. A separately reviewed real dedicated-STA executor slice is next; positive capability, real callbacks/abort/TOCTOU, no-hydration provider acceptance, representative operation performance, and all independent Milestone 8 operator gates remain open. |
| 2026-08-20 | Implemented the separately gated real Windows executor slice: fresh target/hash/exact-folder and affected-survivor admission, projected callback metadata, positive local-root `SHQueryRecycleBinW` evidence, dedicated-STA `IFileOperation`, durable-start acknowledgement, explicit recycle-only flags, callback/finish/outer/abort mapping, cancellation boundaries, ambiguous-start non-retry, and opt-in disposable hard-link/exact-folder Recycle Bin acceptance. | Keep application composition disabled and WPF read-only while proving the native seam on real Windows. `CanSubmit` remains false and every worker response remains `executorEnabled:false`; Milestone 11 is not complete. Real-provider no-hydration, locked/capacity/provider mappings, final constants, `FOFX_ADDUNDORECORD`, residual TOCTOU, representative operation performance, accessible operator acceptance, recovery resolution, and independent Milestone 8 gates remain open. |
| 2026-08-20 | Added a fail-closed Recycle Bin acceptance evidence collector, exhaustive stable HRESULT/flag regressions, an explicit-fixture registered-provider no-hydration test, and a dedicated operator/provider/performance/accessibility guide. | Make every available or missing gate reviewable without enabling production or fabricating physical/provider acceptance. Real access-denied/disconnect/capacity/provider/path-swap/process-loss outcomes, representative large-plan operation performance, final freshness/batch constants, `FOFX_ADDUNDORECORD`, physical accessibility, recovery resolution, and independent Milestone 8 gates remain open. |
| 2026-08-20 | Stabilized the read-only duplicate-group warm path by replacing the per-group correlated across-drive summary probe with an indexed member-stream aggregate and bounding non-name detail enrichment after keyset candidate selection; focused all-sort paging equivalence and existing 100,000-group coverage passed. | Reduce the stable development-host group/summary baseline without weakening the 100 ms p95 target or changing protocol/schema/cache bounds. Final runs passed at 62.11 and 93.01 ms p95 but a retained third run failed at 198.72 ms while unrelated facets spiked too, so representative-hardware warm-query acceptance remains open rather than being closed by retries. This slice adds no filesystem, review-mutation, preflight, Shell, Recycle Bin, or production-executor behavior. |
| 2026-08-20 | Added retained warm-query distributions plus time-aligned test-process and host contention diagnostics to the non-mutating acceptance collector; the instrumented development-host run failed at 140.76 ms group p95 and preserved all 500 query samples, 101 process snapshots, host context, and its 31/31 deterministic contract result. | Separate the stable-cost body from observable concurrent tails without weakening or auto-overriding the 100 ms p95 target. Evidence schema v2 refuses to overwrite earlier runs, and the lower-overhead sampler exposes its own PID. This is development-host diagnostic evidence only: representative hardware, large-plan operation performance, physical/provider campaigns, all production mutation wiring, and Milestone 11 completion remain open. |
| 2026-08-22 | Replaced two intermittent real-WPF focus races with bounded dispatcher-state checks after focused Debug/Release WPF coverage and one passing post-change real smoke in each configuration. | Wait for confirmation headings to be loaded/visible and for virtualized group rows to actually contain keyboard focus instead of relying on an early `Input` callback or a fixed 50 ms delay. Preserve all worker, review, preflight, deletion, and disabled-executor boundaries. |
| 2026-08-22 | Added state-specific read-only Recycle Bin cancellation and ambiguous-recovery disclosures with Core and STA automation coverage. | Explain pre-start, active Shell-boundary, terminal, and `recovery_required` behavior; make the recovery warning assertive, forbid retry, and direct review of every unknown source/Recycle Bin item while preserving `CanSubmit:false`, disabled production composition, and immutable evidence. Physical Narrator/NVDA, high-contrast, multi-monitor DPI, provider, representative performance, constants, Undo, and recovery-resolution gates remain open. |
| 2026-08-22 | Added a selectable, path-free recovery evidence handoff summary with Core and STA automation coverage. | Let an operator copy stable operation/run/preflight/revision identifiers, outcome counts, and stored errors for diagnosis without filesystem inspection, replay, resolution, executor wiring, or a submission action. Physical inspection and recovery resolution remain open alongside the existing provider, accessibility, representative-performance, constants, Undo, and residual-TOCTOU gates. |
| 2026-08-22 | Extended the path-free recovery handoff with the durable policy version, immutable preflight/intent signatures, lifecycle times, and cancellation-request state. | Improve correlation of preserved database, log, and operator evidence without querying item paths, inspecting the filesystem, resolving or replaying an operation, or changing the disabled executor boundary. Physical recovery, provider, accessibility, representative-performance, constants, Undo, and residual-TOCTOU gates remain open. |
| 2026-08-22 | Focused recovery-required result reconstruction on the stored unknown-item subset with explicit paging status and Core coverage. | Make every ambiguous result directly available for operator triage without live filesystem inspection, resolution, replay, executor wiring, or a submission action. Physical inspection and recovery resolution remain open with the existing provider, accessibility, representative-performance, constants, Undo, and residual-TOCTOU gates. |
| 2026-08-22 | Added durable item, preflight, batch, source-snapshot, result-time, numeric Shell, and recycled-item-presence correlation to read-only operation rows and their automation names. | Improve operator evidence correlation for unknown-result inspection without querying live filesystem state, resolving or replaying an operation, enabling an executor, or adding submission. Physical recovery and accessibility acceptance remain open with the existing provider, representative-performance, constants, Undo, and residual-TOCTOU gates. |
| 2026-08-22 | Added exact range/total status and repeatable polite announcements to bounded unknown-result paging. | Let an operator account for every stored ambiguous result during read-only triage without filesystem inspection, resolution, replay, executor wiring, or submission. Physical Narrator/NVDA, high-contrast, multi-monitor DPI, and recovery-resolution acceptance remain open with the existing provider, representative-performance, constants, Undo, and residual-TOCTOU gates. |
| 2026-08-22 | Locked previous-page announcement repeatability and stale/cancelled paging silence with Core regressions. | Announce only a committed read-only recovery page, including backward navigation, without allowing superseded work to produce misleading accessibility output. All physical accessibility and recovery-resolution gates remain open. |
