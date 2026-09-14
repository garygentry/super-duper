# UIR-07a History and contextual warning evidence

Date: 2026-09-13

Scope: local WPF/Core implementation of S05 History/open-run and contextual warnings for
A07/A12/A15 plus the run-context portions of A16/A17. UIR-07 remains in progress.

## Implemented behavior

History now binds one newest-first page of at most 500 runs instead of accumulating every returned
batch in one WPF collection. **Previous scans** and **Next scans** use the existing offset/limit API,
name the exact displayed range, return focus to the realized History row and retain the accepted page
when a replacement request fails. Unsafe, mixed-session, duplicate or unbounded worker pages are rejected.

Highlighting remains harmless and distinct from opening. The selected card names the highlighted
saved scan, exact scan ID/date/state, whether another scan remains open in Results/Review, and any
separate active scan. Recorded locations, reuse policy, ignore-pattern count and exclusion count come
from immutable `WorkerRun.Parameters`; current saved-scan edits cannot rewrite that history. Existing
**Open scan** remains the explicit operation that changes the workspace run.

Warnings use the existing `RunWarningDrilldownViewModel` and worker API. The panel now names the exact
saved scan/run/date/state and whether it owns a current or terminal warning revision. Current refresh
replaces one complete revision, explains revision changes and clears older cached pages; mixed revisions
are never combined. A failed refresh retains the last accepted page and revision. Terminal snapshots
remain one-way and immutable. Pages remain 25 aggregates with the existing five-page cache, each row
shows the worker-owned phase/category/code/severity/count and at most three retained examples, and the
stable hash-warning action still resolves its exact immutable completed run.

Warning Close is contextual. A History entry returns focus to the highlighted run. An active Progress
entry returns to the exact Progress warning button without opening or retargeting the historical
Results/Review run. A stopped-run Summary entry has the corresponding Summary return target. Late
navigation, page replacement and focus generations retain their existing cancellation guards.

## Preserved contracts

- Historical results and review decisions remain immutable. An active run can progress or finish while
  an older workspace run remains selected; Start/Cancel still target only the active run.
- Warning queries remain worker-owned with `executorEnabled:false`, exact current/terminal revision
  ownership, 25 rows and five cached pages. Diagnostic application logs remain supplemental rather than
  durable warning truth.
- UIR-06a combined totals, separate 200-row/five-page Files/Folders Review caches, exact return links,
  validation states and **Check marked copies** versus **Check these copies** remain unchanged.
- UIR-06b ordered roots, virtual preview scope/revision/signature, confirmation/provenance, fresh-preview
  enforcement, **Reverse rule application**, later manual overrides and Reset semantics remain unchanged.
- UIR-04/05 monitoring, rescan, query, comparison, decision, reveal and focus contracts remain covered.
  No worker/protocol/database/cache contract, Performance detail, recovery resolution or execution path changed.
- Production still uses `DisabledRecycleOperationCapabilityExecutor`; `CanSubmit=false`; no production
  execution action exists. No production state, physical/provider/performance campaign or parked release
  validation work was touched.

## Verification

| Check | Result |
|---|---|
| `dotnet test ...SuperDuper.Windows.Core.Tests.csproj --artifacts-path artifacts/uir07a/core-final` | **218 passed**, `artifacts/uir07a/results/core-final.trx` |
| `dotnet test ...SuperDuper.Windows.Smoke.Tests.csproj --artifacts-path artifacts/uir07a/wpf-accepted-final` | **3 passed**, `artifacts/uir07a/results-accepted/wpf-accepted-final.trx` |
| `dotnet build ...SuperDuper.Windows.RedesignFixture.csproj --artifacts-path artifacts/uir07a/fixture-final` | passed, **0 warnings / 0 errors** |
| Production-lock source audit and `git diff --check` | passed before commit |

The Core baseline gained two methods: a 500/501-run paging/retained-failure contract and an exact
highlighted/open/active context plus return-origin contract. Existing warning tests now also prove
revision-change explanation and retained accepted data on regression failure. The three loaded-STA
methods exercise real MainWindow focus, delayed contexts, active and terminal warning entry, bounded
collections, warning action navigation, prior Results/Review behavior and the retained theme/text/viewport
matrix. There are 114 PNGs under `artifacts/uir07a/captures-accepted-final`; representative History
context and warning states at 1180x760 and 900x600 were visually reviewed.

Retained probes record the corrected WPF binding syntax, updated single-column aggregate assertions,
and a sandbox-only Windows SDK read denial. The same isolated WPF command then passed with SDK read
access; no product or user state was involved. PID 67748 was re-audited responsive at the exact older
UIR-03f fixture executable and was not reused, focused, closed or overwritten.

## Remaining scope

UIR-07a is locally implemented; UIR-07 remains `in_progress`. UIR-07b should compose contextual
Performance detail from the existing bounded summary APIs while preserving unavailable values,
comparison qualifiers, the 25-history/six-phase/64-device bounds, exact run identity and return focus.
Do not introduce time-series claims, recovery resolution, execution, full A17 or UIR-08 integration.
NVDA and physical 200% remain `unrun_unavailable` for UIR-08/A11.
