# UIR-05b adjustable file comparison evidence

Date: 2026-09-12
Disposition: locally implemented; UIR-05 remains `in_progress`

## Outcome

The Files result surface now gives the bounded result rows the remaining page height instead of
stacking fixed-height grids inside a page scroller. At standard width it presents an adjustable
36/64 duplicate-set/copy comparison with a keyboard-focusable splitter. The two virtualized grids
use one wrapping, meaningful column each and disable horizontal scrolling. Server-owned sorting,
paging, cache ceilings and the UIR-05a accepted-query snapshot remain unchanged.

Below the narrow breakpoint the surface becomes explicit navigation rather than a squeezed
side-by-side table: duplicate sets lead to **Compare selected set**, copy comparison retains
**Back to sets**, and selecting one copy opens its exact path, Keep / Mark for removal / Reset and
Copy path / Show in Explorer actions with **Back to copies**. The selected-copy surface scrolls
vertically and never horizontally. Returning from either level restores keyboard focus to the
visible list. A confirmed decision refresh preserves the selected copy by immutable member ID;
loading a set still does not imply a review choice.

## A03 verification

- At 1180x760 the populated loaded-STA fixture measured the comparison workspace at
  `279.0 / 400.3 DIPs`, or **69.7%** of the usable Files view height, above the required 60%.
  The initial panes measured `307.4 / 546.6 DIPs` (36%/64%). Changing the GridSplitter-backed
  columns to 45%/55% grew the set pane by more than 20 DIPs before restoring the default.
- At 900x600 the fixture verifies list-only entry, Compare selected set, copy selection,
  selected-copy detail, Back to copies and Back to sets. Long relative/root paths remain meaningful
  in rows; the complete path is exposed in a wrapped read-only field and automation name. All five
  decision/path actions are reached through the bounded vertical scroller. The set grid, copy grid,
  selected-copy detail and advanced query region each report zero horizontal scrollable width.
- Actual focus handlers are exercised after validation, next-set navigation, Back to copies and
  Back to sets. Selected set, run identity and group scroll offset survive navigation away and back.
- Long path, delayed query, draft/applied filter, exact unit, chip removal, Clear/default sort,
  theme, text-size, toolbar-stress, monitoring and latest-only automation checks remain in the same
  three-method WPF run. The 900x600 enlarged-text physical/native matrix remains UIR-08; no 200%
  or NVDA claim is made here.

Accepted captures are under `artifacts/uir05b/captures-accepted-final`, including the 1180x760
side-by-side state and separate 900x600 set, copy-list, exact-path and decision/path states.

## Preserved contracts

- UIR-05a keeps explicit Apply and Enter, removable chips, Clear, draft/applied snapshots, exact
  B/KiB/MiB/GiB/TiB conversion, every existing filter, selected-run identity and server sort.
- Group/member paging remains 200 rows per request with the existing five-page caches. No eager
  full-result collection, engine, worker or protocol change was introduced.
- Keep/Remove/Undecide still record durable intent only. Row selection is harmless, stale decision
  continuations remain rejected, and production Recycle Bin execution remains disabled.
- UIR-04 monitoring/terminal behavior and SOP authority boundaries are unchanged.

## Verification and retained corrections

| Check | Result |
|---|---|
| `dotnet test ...SuperDuper.Windows.Core.Tests.csproj --no-restore --artifacts-path artifacts/uir05b/core` | **207 passed**, `artifacts/uir05b/results/core-accepted.trx` |
| `dotnet test ...SuperDuper.Windows.Smoke.Tests.csproj --no-restore --artifacts-path artifacts/uir05b/wpf` | **3 passed**, `artifacts/uir05b/results/wpf-accepted-final.trx` |
| `dotnet build ...SuperDuper.Windows.RedesignFixture.csproj --artifacts-path artifacts/uir05b/fixture-final` | passed, **0 warnings / 0 errors** |
| `git diff --check` | passed before checkpoint update |

Failed and corrective WPF iterations are retained in `artifacts/uir05b/results` and
`artifacts/uir05b/results/retained-iterations`. They record replacement of old multi-column/fixed-
height assumptions, the first successful 69.7% measurement with clipped actions, dispatcher-affinity
focus corrections, narrow-detail geometry, toolbar stress, one incorrect test-helper name and the
final vertical-scroll/action viewport corrections. Passing evidence was written to fresh names; no retained result was
overwritten.

## Runtime and remaining scope

PID 67748 was re-audited immediately before the fixture build. It remains the responsive fictional
UIR-03f executable under `artifacts/uir03f/fixture`; it was not reused, focused, closed or overwritten.
The final UIR-05b fixture is built, not running. Test windows closed. No production app, worker, database,
hash cache or user file was touched.

UIR-05 remains open. The exact next local slice is UIR-05c: adjustable folder-results list/detail
comparison and remaining folder decision/path verification for A04/A05/A06/A11/A15. Full A17,
integrated/native/operator acceptance, NVDA and physical 200% remain later gates.
