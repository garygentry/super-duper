# UIR-06a Review overview and validation-status evidence

Date: 2026-09-13
Disposition: locally implemented; UIR-06 remains `in_progress` for location preferences and later
integrated/native acceptance

## Outcome

Review is now a dedicated plan overview for the selected completed scan. It identifies the scan and
plan revision, separates file-copy and folder-copy decision counts, and presents the worker-owned
combined distinct-file, physical-item and planned-byte totals. The explanation names overlap and
hard-link de-duplication without reconstructing totals in the UI.

Files and Folders use independent 200-group worker queries, cursor histories and five-page caches.
Each virtualized list has its own loading, empty, paging and retained-results error state. Failed
replacement reads leave the last accepted bounded page visible. Exact-set links return to the
currently resolvable bounded Results set and copy; an unresolved link reports the bounded-page
limit and returns to Review rather than crawling or materializing every result.

Whole-plan validation is explicitly labelled **Check marked copies** and is distinguished from the
Files visible-page **Check these copies** action. The status separates freshness from outcome:
current revision, **Plan changed — check again**, **Ready**, **Blocked**, and **Needs review** all
come from stored worker preflight state and reasons. Review also states both “Marking copies does
not delete files.” and “This build can review and check a removal plan. Moving files to the Recycle
Bin is not available.”

## Local acceptance coverage

- **A01/A15:** Review names the selected scan/date/state; exact return navigation rejects a changed
  selected run. Marking and validation remain non-deleting, and no execution action was added.
- **A04:** file and folder counts remain separate, selected-set links identify immutable group/member
  IDs, and the existing decision workflow is unchanged.
- **A10:** combined totals are displayed directly from `WorkerReviewPlanSummary`; independent bounded
  pages do not become a global flat plan. A saved preflight for an older review revision cannot show
  current or ready. Conflict is blocked; changed, missing and unavailable outcomes need review.
- **A11 (local):** Review lists recycle virtualized containers, disable outer horizontal scrolling,
  remain vertically reachable at 1180x760 and 900x600, and restore focus to the exact Files/Folders
  result list after a resolvable return link. Full NVDA/physical 200% remains UIR-08/A11.

## Preserved contracts

- Review file/folder group requests remain 200 rows with independent five-page caches; preflight
  observation pages remain 100 rows with the existing five-page cache. No cache or page ceiling grew.
- The worker owns review plans, revisions, combined overlap/survivor totals and preflight outcomes.
  No engine, worker, protocol, database, query semantics or filesystem behavior changed.
- UIR-05a draft/applied queries, units, chips and filters; UIR-05b file comparison/navigation; and
  UIR-05c folder comparison/decision/reveal behavior remain covered by the full Core/WPF passes.
- Existing location-preference authoring, preview, application and reversal were not moved or changed.
  Execution, recovery resolution and UIR-07 remain outside this slice.
- Production still injects `DisabledRecycleOperationCapabilityExecutor`; `CanSubmit` and
  `executorEnabled` remain false, operation evidence remains read-only, and there is no execution action.

## Verification and retained corrections

| Check | Result |
|---|---|
| `dotnet test ...SuperDuper.Windows.Core.Tests.csproj -p:BaseOutputPath=.../artifacts/uir06a/core-accepted/bin` | **214 passed**, `artifacts/uir06a/results/core-accepted.trx` |
| `dotnet test ...SuperDuper.Windows.Smoke.Tests.csproj --artifacts-path artifacts/uir06a/wpf-accepted-reviewed` | **3 passed**, `artifacts/uir06a/results/wpf-accepted-reviewed.trx` |
| `dotnet build ...SuperDuper.Windows.RedesignFixture.csproj --artifacts-path artifacts/uir06a/fixture-accepted-reviewed` | passed, **0 warnings / 0 errors** |
| Production-lock source audit and `git diff --check` | passed before commit |

The retained 210 Core baseline gained four focused contracts for combined/separate review paging,
revision-aware outcome presentation, exact bounded file/folder return links, and failed-page
retention. The three WPF methods retain all earlier shell/query/comparison/theme/text checks and add
Review automation, recycling, 1180x760/900x600 reachability and non-execution boundary checks.
There are 105 PNGs under `artifacts/uir06a/captures-accepted-reviewed`, including
`populated-Review-1180x760.png` and `populated-Review-900x600.png`. Initial fresh `--no-restore`
asset-file failures and earlier corrective runs remain under `artifacts/uir06a`; passing evidence
uses isolated final paths.

## Runtime and remaining scope

PID 67748 was re-audited immediately before final verification. It remains responsive in session 1
at the exact older `artifacts/uir03f/fixture` executable and was not reused, focused, closed or
overwritten. No other Super Duper app or worker was running. The UIR-06a fixture was built but not
launched; test windows closed. No production app, database, cache, log, scan or user file was touched.

UIR-06a is locally implemented. UIR-06 remains `in_progress`. The next dependency-ready local slice
is UIR-06b: present the existing Location preferences workflow as a focused Review panel while
preserving saved ordered roots, virtual preview scope/revision/signature, explicit application
confirmation, provenance, reversal and later manual overrides. It must not change decision semantics,
add arbitrary undo, enable execution, resolve recovery outcomes or pull UIR-07 forward.
