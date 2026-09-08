# Screen and interaction specification

Version 2, accepted high-level direction with long-scan/rescan refinements. IDs below map to [validation](validation.md). They specify behavior,
not a particular XAML control implementation. Prototype omissions are listed in its readme.

## S01 Shared shell and context

- Header: saved-scan name, selected run's start date/time and state, and location count. The name
  alone is never the run identifier. Show 'No scan yet' for a new definition.
- Persistent navigation: Scan, Results, Review, History. Results contains Files and Folders modes.
- The selected saved scan and selected historical run are independent from the globally active run.
  While browsing old results, show a compact active-scan strip with its name, phase and 'View progress'.
- Cancellation always targets the active run and names it. Starting a second scan remains disabled
  with a visible reason. Preserve current restrictions on definition mutations during active scans.
- Selecting a history row only selects it. An explicit 'Open scan' changes workspace context. Opening
  a completed scan goes to Results; a cancelled/interrupted/failed scan goes to its Scan summary.
- On completion, a user watching that run sees a summary and 'View results'. A user elsewhere sees
  a quiet completion notice; do not steal focus or replace their selected historical run.
- Preserve filters, selected group, and scroll position during navigation within one run using
  bounded state. Run changes invalidate cursors and never carry decisions across run IDs.
- The footer describes the visible context, or explicitly labels global engine status. It never
  presents an old run's counts under a newly selected name. While a new context loads, display that
  context's loading state. Late responses cannot update it.
- New saved scans receive a readable suggested name. Explicit Save persists setup; Start uses the
  existing save-before-start workflow. Leaving dirty setup asks Save / Discard / Stay. This is an
  intentional configuration decision, not an additional confirmation for every navigation.

## S02 Scan setup and monitoring

The detailed [long-scan and rescan contract](scan-and-rescan-experience.md) is part of this screen's
specification. It defines progress denominators, update freshness, diagnostic disclosure, Scan again,
persistent hash reuse and new/changed/deleted-file behavior. Implement against A08/A16/A17.

Setup starts with location selection, then a short exclusions summary, then the primary Start scan
action. Use one 'Add folder or drive' picker and a secondary 'Enter path' option. Each selected
location shows a friendly label/path, detection/availability status and an explicitly named Remove
location action. Display paths may omit the Windows extended-length prefix; stored paths and all
commands keep the exact canonical value. Normalization is part of validation with a readable
explanation of overlapping roots, not a technical primary button.

Default repeat policy: reuse verified hashes. Explain 'Reuse previous checks for unchanged files;
uncertain files are read again.' Preserve the explicit `revalidate_content` choice in Advanced,
labeled 'Re-read candidate content'; normal candidate filtering still applies.
Registered cloud-root exclusions remain visible before starting. Advanced also holds ignore globs
and manual exclusions, with examples and inline errors. Do not replace fail-closed detection failure
with a dismissible warning or offer the deferred cloud opt-ins.

The running view has one dominant phase label, elapsed duration, current activity, cancellation and
a compact warning entry. Disclose candidate funnel, read throughput and cache details below. ETA
is phase-qualified and shown only when trustworthy; there is no invented overall percentage based
on files discovered. Durations use days/hours/minutes for long runs. Retain exact detailed counts,
rate windows and four folder substages in technical detail. Terminal views remove active animation,
label retained metrics as historical, and give a clear next action.

## S03 Results: file and folder modes

Default file sort remains highest potential savings first. Put three compact figures above the
workspace: matching sets, copies in those sets, and potential savings. Explicitly label them as
filtered results; the persistent review total is the entire selected run's plan, not the filter.

At standard desktop width use a resizable set list and copy comparison pane. At narrow width show
the list or selected-set detail with a clear Back to sets command; restore selection and focus.
Do not squeeze essential copy actions into a horizontally scrolling grid. Keep a path search,
Files/Folders switch and Filters button visible. Filtering uses explicit Apply or Enter; the
current query remains coherent until replacement results arrive. Show applied filters as removable
chips; Clear filters restores the default query. Changes in the filter editor remain drafts until
applied. No background local filtering of a server page presented as a full-run result.

Filter disclosure retains all existing semantics: literal path search vs exact path, minimum
one-copy file size, >=1 GiB preset (currently called 1 GB), three-or-more copies, across drives,
any/all member extension and no-extension modes, root facet and drive facet. Provide a unit selector
for size and convert exactly to bytes; use 'GiB (1,073,741,824 bytes)' in help. Folder controls get
visible labels and retain their own supported size semantics; file-only filters never leak into
folder queries. Facet lists stay independently paged and expose sort/order in a compact selector.

Set list: example filename/folder, copies, size and potential savings where supported. The selected
item has a clear focus and selection treatment. Do not describe the example label as the original.
Keep group and copy paging separate: 'Sets 1-200 of ...' versus 'Copies 1-200 of ...'. Preserve
next/previous set keyboard behavior across pages. A context menu supplements visible commands.

File comparison rows prioritize location, meaningful relative path, size/modified time, decision
and current validity. Show exact full paths in selectable details and accessible text. Selection
reveals Keep / Mark for removal / Reset decision and Show in Explorer / Copy path, with the target
path named accessibly. One mutation is pending at a time; do not claim a decision saved before the
worker confirms it. Action failure leaves the prior durable decision visible with an inline reason.

Folder comparison uses the same decision vocabulary and location treatment. Explain that contents
match exactly while root names can differ. Preserve shared-parent/differing-path explanations,
nested-group suppression, per-folder reveal and explicit current-page Explorer selection. A folder
decision describes descendant scope; overlapping file decisions do not increase totals twice.

Review shortcut: 'Review marked copies (N)' plus worker-owned planned bytes. A zero plan invites
choosing copies; it is not an error. Rules are a discoverable 'Location preferences' command opening
a focused panel, rather than a large editor permanently occupying the result area.

## S04 Review and location preferences

Review is a plan for the selected completed run. Show plan revision, file/folder decision counts,
distinct affected files and planned bytes from the existing combined summary. Browse Files and
Folders using separate bounded review-group queries; expanding a set fetches bounded members.
Do not promise a globally merged flat removal list or global review-state filter without a contract
supporting it. Each row links back to its exact set and current copy page where resolvable.

Show 'Marking copies does not delete files.' Validation's primary action is 'Check marked copies'.
Changing a decision invalidates prior validation; show 'Plan changed — check again' instead of a
green state attached to an old revision. Summarize ready, blocked and needs-review outcomes using
existing preflight reasons; paged detail explains affected paths and retained-copy constraints.
The visible-page freshness command is separately labeled 'Check these copies' and never implies a
whole-plan check. Dirty-root reconciliation states exactly which bounded batch it is checking.

Rules retain saved ordered roots, virtual preview, preview scope, revision/signature, explicit
application confirmation, and application reversal. Label preview changes separately from saved
decisions. The confirmation names the scope and number of affected sets/copies; changed preview or
plan requires a fresh preview. Reversal preserves later manual overrides per the existing contract.
Use 'Reverse rule application' for that operation. 'Reset decision' means return to undecided;
do not implement an unsupported arbitrary undo stack or imply deletion can be undone here.

Current-build notice: 'This build can review and check a removal plan. Moving files to the Recycle
Bin is not available.' No execute button. Existing read-only operation evidence and recovery
observations remain accessible under operation details when relevant. Do not infer unknown outcomes
by inspecting live paths or introduce recovery resolution. A future execution design is out of scope.

## S05 History, warnings and diagnostics

History is scoped to a saved scan, newest first, with date, status, duration if available, file count,
potential savings and warning count. Rows have explicit Open scan and warning access. Preserve the
bounded history contract; if more runs exist, provide paging rather than silently presenting a capped
list as complete. Run parameters and exclusions describe the immutable run, not edited setup.

Warnings open in the context of the run that supplied the count. During a live scan, a drawer or
dedicated detail view keeps a return path to progress. Group by existing phase/code/category, show
occurrence counts and bounded examples, and distinguish scan exclusions from failures. Only offer
actions backed by existing stable-target navigation. A stale active warning page refreshes with an
explanation, never silently combines revisions. App diagnostic logs are separate from scan warnings.

Performance remains available in one action from scan/history details. Show phase durations and
cache/read summaries first. Device list selects a device for labeled current/peak metrics instead
of a permanently enormous table. Preserve explicit unavailable values and comparable-run qualifiers.
No new time-series charts: the current API supplies summaries, not sample arrays.

## S06 State matrix

| State | Presentation | Primary next action / restriction |
|---|---|---|
| No saved scans | Short purpose statement and location picker entry | Add folder or drive |
| Setup invalid | Field-specific reason plus summary near Start | Correct highlighted field; Start unavailable |
| Cloud detection incomplete/failed | Explain exclusions cannot yet be confirmed | Retry detection; Start remains blocked |
| Pending/running | Active run identity and current phase; honest measurable work | Cancel scan; one active run only |
| Cancelling | Acknowledged request, still waiting for worker termination | No duplicate cancellation requests or completed claim |
| Completed | Outcome, coverage/warnings, selected run date | View results |
| Completed with no duplicates | 'No exact duplicates found' within scanned scope | Edit locations or view exclusions |
| Filter matches nothing | 'No duplicates match these filters' | Clear/edit filters; retain run |
| Cancelled/failed/interrupted | Results unavailable; explain completion was not reached | Start a new scan or choose an earlier completed scan |
| Context loading | Newly selected run named, only relevant region busy | Browse other available areas; never show old counts as new |
| Query failed | Error belongs to the exact query; prior same-query data labeled if retained | Retry that query |
| Worker exited | Connection loss plus affected run; saved history availability stated honestly | Restart scan engine; no automatic destructive recovery |
| No copy selected | Set context remains; decision panel has short instruction | Select a copy |
| Decision pending | Existing decision retained, saving indicator | Other decision writes disabled until resolved |
| Decision rejected/conflict | Existing decision with reason; do not overwrite intent silently | Refresh affected set/review revision |
| Missing/changed copy | Historical match plus current validity text | Check current copies / revise plan; unsafe choices disabled |
| Dirty root / uncertain freshness | Scope and bounded reconciliation state | Check next batch; no unconditional ready badge |
| Validation in progress/cancelling | Plan revision, progress and cancellation status | Cancel check; no execution |
| Validation stale/blocked | Explain changed plan or affected reason | Check again / return to exact set |
| Recovery-required operation | Stored evidence and unknown outcomes remain explicit | Existing observation workflow only; no replay/resolution |
| Performance unavailable | Explain why this run lacks retained metrics | Return to scan results; no zero or failed-scan inference |

## S07 Copy examples

| Existing wording | Proposed wording |
|---|---|
| Sessions | Saved scans |
| Scan roots | Folders and drives |
| Normalize roots | Automatic validation with a specific overlap explanation |
| Representative label | Example filename |
| Remove (review) | Mark for removal |
| Undecided (action) | Reset decision |
| Preflight | Check marked copies |
| Full immutable path | Full path at scan time |
| Full hash satisfied | Content checks completed (technical details retain precise meaning) |
| Server-paged / bounded / worker-owned | Omit from everyday copy; show item ranges and useful limits |
| Restart worker | Restart scan engine |
| Recoverable | Potential savings (results) / Marked for removal (plan) |

Accessibility names include action target and meaningful scope; they should not narrate internal
generation, storage or transport mechanics. Essential limitations remain visible, not tooltip-only.
