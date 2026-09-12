# UIR-05a compact file query controls and filtered totals

2026-09-12. Baseline `9c528fb` (UIR-04c), implemented on `codex/ui-redesign`.
Local A03/A06/A13 query/header scope is implemented. UIR-05 remains in progress; adjustable
comparison and full A03 measurement belong to UIR-05b. No operator/native acceptance is claimed.

## Behavior and review

- Path search, Apply, Filters, Clear filters and three compact filtered totals are exposed above
  the results. They are outside disclosures. The existing page scroll preserves access when the
  minimum viewport, toolbar allowance or enlarged text leaves little room. This is not a pinned
  header: scrolling to comparison can scroll it offscreen. Empty/unknown totals show a dash until
  a completed-run query succeeds; a true zero response remains distinct from unavailable results.
- Filters contains exact-path mode, minimum one-copy size, the correctly named 1 GiB preset,
  three-or-more copies, across drives, any/all-member extension and no-extension semantics,
  independent selected-root/drive facets, compact facet sort selectors, optional location coverage/
  largest opportunity and the retained preference-rule workflow. No folder query is changed.
- Binary B/KiB/MiB/GiB/TiB conversion uses integer rational arithmetic, rejects fractional bytes
  and signed-64-bit overflow without rounding, and preserves max(explicit minimum, 1 GiB).
  The editor bounds size input to 256 characters; worker byte/search/extension limits are unchanged.
  Exact GiB bytes appear in help and applied size chips; potential-savings exact bytes remain in a tooltip.
- Previously paging, facet sorting and preference scope rebuilt their filter from draft fields.
  They now use the accepted query snapshot. Apply/Enter sends a captured draft; rows, selected set,
  copies, totals, review plan and chips remain under the previous applied filter until replacement
  succeeds. Typing during a delayed query does not alter that request or overwrite newer drafts.
  Failed/invalid queries retain accepted results; failed sort restores the old cursor's sort and
  glyph. Cancellation clears obsolete facet loading indicators. Generation guards reject late results.
- Up to seven removable applied chips name the actual server semantics. Removing one resets its
  editor fields and applies only that removal; unrelated drafts remain unapplied. Clear restores
  default filters and highest-savings-first group sorting. Applied filter changes invalidate rule
  previews; rule scope never borrows an unapplied draft. Review decisions remain worker-owned.
- Enter applies only from path/size/extension editor text boxes. Ctrl+F focuses/selects path search;
  Escape closes Filters and returns focus. Removing the focused chip moves focus before starting
  the query; completion does not steal focus from a newer target. Native set/member paging,
  selection, decisions and MostRecent query announcements remain.

Source review covered query snapshots, cancellation, rule scope, exact conversion, focus handlers,
facet paging, sort indicators, native virtualization and the changed test expectations. The 200-row
group/member, 25-row facet, five-page cache and bounded neighbor-prefetch contracts remain unchanged.
No Rust, worker, protocol, infrastructure, production database/cache/state or deletion wiring changed.
UIR-04 freshness/terminal/phase/UIA contracts and SOP boundaries remain intact.

## Verification

All outputs use `artifacts/uir05a`; no running output directory was rebuilt. Tests use fictional
in-memory worker/service responses. No real scan, Explorer, filesystem decision or deletion runs.
NuGet.Config access was denied in the sandbox; approved isolated test commands used the existing cache.
Git uses per-command safe.directory; no global Git configuration was changed.

| Check | Result / retained evidence |
|---|---|
| Existing Core after initial implementation | 200 passed, `results/core-initial.trx` |
| Six query regression methods | Full Core 206 passed, `results/core-query.trx` |
| Review regressions | 206 passed / one failed: fictional rule lacked a root, `results/core-review.trx`; fixture setup corrected |
| Final Core | 207 passed, `results/core-final.trx` |
| Initial WPF build | Missing ToggleButton test import; corrected before test execution |
| WPF label update | Two passed / one old Apply-label expectation failed, `results/wpf-query.trx`; expected label updated |
| Fixed-header layout attempt | Two passed / one minimum toolbar row-reachability failure, `results/wpf-labels.trx`; compacted header |
| Compact fixed-header attempt | Two passed / one enlarged-text copy-action reachability failure, `results/wpf-compact.trx`; retained ordinary page scrolling |
| Scrolling-header verification | Three methods passed, `results/wpf-scroll.trx` |
| Disclosure cleanup | Two passed / one visual-tree lookup failed for now-hidden location disclosure, `results/wpf-review.trx`; fixture uses stable XAML name |
| Final disclosure/layout run | Three methods passed, `results/wpf-final.trx` |
| Sort-indicator review | Two passed / one failed: DataGrid clears the glyph after replacing ItemsSource, `results/wpf-verified.trx`; restore current server sort after binding |
| Final reviewed WPF | Three methods passed, `results/wpf-accepted.trx`; includes query control reachability at enlarged text and sort glyph restoration |
| Fictional desktop fixture | Built in `fixture`, zero warnings/errors; not launched |

The seven added Core methods cover delayed apply and failure, unknown versus zero, editing during
pending queries, repeated paging/cache ceilings with invalid drafts, applied rule scope, all chip
removals, Clear defaults, exact unit boundaries (including one byte in TiB), failed-sort cursor
recovery, cancelled facets, run changes and late clear/replacement responses. Retained tests cover
all prior filter modes, durable decisions, lifecycle, worker rejection and query announcements.

Loaded-STA fixtures cover 900x600 and 1180x760 long extended-UNC paths, real routed Enter handling,
delayed queries, exact search selection/horizontal scroll, focused chip removal, Escape/trigger
return, draft units, selected historical versus active run and default sort glyph restoration.
The populated shell retains minimum plus 80-DIP toolbar row/action reachability, same-run scroll/
selection and native theme/text enlargement checks. These are programmatic fixture checks, not
physical keyboard, screen-reader, DPI or operator evidence.

Reviewed final captures in `captures-accepted` show exposed compact controls/totals and useful visible rows
at minimum size, long applied-path chips, and reachable advanced controls. An early draft-unit capture
preceded binding propagation; the final verifier drains the dispatcher and asserts the unit/value
before capturing. Earlier captures and failed TRX files are retained, not represented as final passes.

Representative commands (TEMP/TMP set to `artifacts/uir05a/temp`):

```powershell
dotnet test apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj --artifacts-path artifacts/uir05a/dotnet
dotnet test apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj --artifacts-path artifacts/uir05a/wpf
dotnet build apps/windows/tools/SuperDuper.Windows.RedesignFixture/SuperDuper.Windows.RedesignFixture.csproj --artifacts-path artifacts/uir05a/fixture
```

Set `SUPER_DUPER_UIR05A_CAPTURES` to a fresh isolated capture folder. Retain UIR-04c's 200 Core/three
WPF methods and the prior Rust/Debug/Release integration baseline without rerunning a campaign.
This local query/presentation change does not claim representative large-result performance or
the final UIR-08 integration matrix.

## Runtime and next slice

The CIM audit was denied; Get-Process re-audited PID 67748/session 1, responsive at the exact older
UIR-03f fixture executable. It remains untouched (`runtime-audit.json`). UIR-04b/04c and the new UIR-05a
fixtures are built, not running. Test windows closed on completion. No production app/worker was observed or changed.

Next: **UIR-05b adjustable file-results list/detail comparison and full A03 verification**. Use S03,
design/validation and directly linked comparison/view/focus tests. At 1180x760 measure at least 60%
usable content height for list/detail; at 900x600 preserve essential paths/decisions without horizontal
scroll, using narrow list/detail navigation and Back to sets where needed. Preserve this query/editor
snapshot contract, selected-run identity, durable decisions, bounded pages/cache and focus. UIR-05
folder-mode and remaining decision/path work stay open until their own scoped evidence is recorded.

Full A17 and operator acceptance remain later. NVDA and physical 200% remain `unrun_unavailable`
for UIR-08/A11; do not troubleshoot Windows. Production deletion remains disabled; no SOP/provider/
physical campaign, branch switch, worktree, merge or push is authorized by this slice.
