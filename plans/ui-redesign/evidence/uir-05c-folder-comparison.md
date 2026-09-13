# UIR-05c adjustable folder comparison evidence

Date: 2026-09-13
Disposition: locally implemented; UIR-05 remains `in_progress` for later integrated/native acceptance

## Outcome

The Folders result surface now uses the remaining page height for an adjustable 36/64 exact-set /
folder-copy comparison. Both bounded, virtualized lists use one wrapping column and disable horizontal
scrolling. At narrow width the surface becomes an explicit set -> copies -> selected-copy journey,
with Back to copies and Back to sets restoring focus to the visible list. The complete selected path,
folder relationship, descendant scope, current decision and all five review/path actions remain
reachable through bounded vertical scrolling.

Folder filters now expose visible path and minimum-one-copy-byte labels, Apply, Enter, Clear and a
truthful applied-query summary. Drafts cannot retarget paging. A replacement query becomes applied
only when its page is accepted; a failed replacement retains the old rows and applied summary. Clear
restores the default size-descending query. These contracts extend the UIR-05a draft/applied snapshot
without changing worker query semantics, units, page sizes or caches.

## Local acceptance coverage

- **A04:** loading a folder-copy page leaves selection neutral. Selecting a row changes no decision.
  Keep, Mark for removal and Reset decision name the exact folder copy and descendant scope, wait for
  worker confirmation, preserve selected-member identity across refresh, and retain existing stale,
  rejected and overlap behavior.
- **A05:** no selected scan, running/pending/cancelling, cancelled, failed, loading, initial query
  error, retained-results error, default empty and filtered empty are distinct. The enlarged-text
  Light/Dark fixture proves the empty state does not overlap a visible result header.
- **A06:** both inputs have visible labels; ordinary copy explains exact-content equivalence, lack of
  an inferred original, descendant consequences and overlap counting. The selected path is complete,
  wrapped, read-only and separately exposed to automation.
- **A11 (local):** set/member paging, bounded current-page Explorer selection, Back to copies and Back
  to sets restore keyboard focus to the visible comparison. The splitter is keyboard-focusable and
  selected-run/group identity survives navigation. Full NVDA/physical 200% and integrated native
  coverage remain UIR-08.
- **A15:** decisions still record review intent only. Production injects
  `DisabledRecycleOperationCapabilityExecutor`, `RecycleOperationViewModel.CanSubmit` remains false,
  and no **Move to Recycle Bin now** action was added.

At 1180x760 the fixture asserts the initial 36/64 side-by-side split and proves that changing the
split to 45/55 grows the set pane before restoring the default. At 900x600 it exercises the three
explicit navigation levels, complete long local/fictional UNC paths, five selected-copy actions,
descendant scope, zero horizontal scrolling, bounded reveal and focus restoration. The same run
retains UIR-05a/b query, file comparison, toolbar-stress, theme and 150% text-enlargement coverage.
Accepted captures are under `artifacts/uir05c/captures-accepted-reviewed` (105 PNGs).

## Preserved contracts

- Folder group/member requests remain bounded at 200 rows with the existing caches; current-page
  Explorer selection remains grouped by parent and single-folder reveal remains bounded.
- Selected-run identity, immutable group/member IDs, review revision ownership and combined worker
  totals remain authoritative. No engine, worker, protocol, database or filesystem behavior changed.
- UIR-05a Apply/Enter/chips/Clear, exact binary units and all file-filter semantics remain covered.
  UIR-05b file comparison, narrow navigation, complete paths, decisions and focus remain covered.
- UIR-04 monitoring/rescan behavior, production state and every SOP/physical-authority boundary are
  unchanged. Full A17 and integrated/native/operator acceptance remain later.

## Verification and retained corrections

| Check | Result |
|---|---|
| `dotnet test ...SuperDuper.Windows.Core.Tests.csproj --artifacts-path artifacts/uir05c/core-accepted` | **210 passed**, `artifacts/uir05c/results/core-accepted.trx` |
| `dotnet test ...SuperDuper.Windows.Smoke.Tests.csproj --artifacts-path artifacts/uir05c/wpf-accepted-reviewed` | **3 passed**, `artifacts/uir05c/results/wpf-accepted-reviewed.trx` |
| `dotnet build ...SuperDuper.Windows.RedesignFixture.csproj --artifacts-path artifacts/uir05c/fixture-accepted-reviewed` | passed, **0 warnings / 0 errors** |
| Production-lock source audit and `git diff --check` | passed before checkpoint update |

The Core total is the retained 207 plus three focused accepted-query/decision-selection contracts.
Corrective results are retained under `artifacts/uir05c/results/retained-iterations`: initial fresh
`--no-restore` artifact setup failures, old ListBox/action-label expectations, narrow selected-detail
reachability, toolbar stress and the enlarged empty-state viewport. Passing outputs use fresh names;
no running output was overwritten.

## Runtime and remaining scope

PID 67748 was re-audited immediately before the accepted fixture build. It remains responsive at the
exact older `artifacts/uir03f/fixture` executable and was not reused, focused, closed or overwritten.
No other Super Duper app or worker was running. The UIR-05c fixture was built but not launched; test
windows closed. No production app, database, cache, log, scan or user file was touched.

UIR-05a/b/c are now locally implemented. UIR-05 remains `in_progress` until later integrated/native
acceptance. The next dependency-ready local slice is UIR-06a: compose the dedicated Review overview
from worker-owned combined plan totals and separate bounded Files/Folders review queries, including
revision-aware non-deleting validation status and exact-set return links. Rules, reversal and the
rest of UIR-06 remain separate follow-on work; no execution control is authorized.
