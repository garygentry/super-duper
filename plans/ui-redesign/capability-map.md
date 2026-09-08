# Capability mapping and implementation feasibility

Source baseline: `deefa40`. Audit restricted to the redesigned surfaces and their existing contracts.
This is a design feasibility assessment, not a re-acceptance of historical gates.

## Ownership

- `apps/windows/src/SuperDuper.Windows`: WPF views, visual resources, focus and UI Automation.
- `SuperDuper.Windows.Core`: semantic navigation/context, view-model composition, presentation state.
- `SuperDuper.Windows.Infrastructure`: process, protocol, native path and Explorer services.
- Rust worker: query/filter/sort/summary/review/preflight truth. Product/status databases remain
  worker-owned. `super-duper-ffi` and engine algorithms are outside redesign scope.

## Surface mapping

| Proposed capability | Existing implementation/contract | Work classification and constraint |
|---|---|---|
| Four-area shell and selected-run context | `MainWindow.xaml`, `ShellViewModel`, `RunHistoryViewModel` | WPF/Core composition; replace numeric tab coupling with explicit destinations, retain generations |
| Independent loading | `ShellViewModel.SelectSessionAsync` awaits Performance, file, folder and Preflight loads | Core lifecycle change; characterize order/generation and remove global loading dependency without duplicate requests |
| Setup and safe repeat policy | `SessionSetupViewModel.EnsureSavedAsync`, root/cloud services | Presentation; preserve save-before-start and validation gates |
| Long-scan live summary/details | `ScanProgressViewModel`, `ScanProgressProjection`, progress counters/funnel/rates/current path and folder substage contract | Core/WPF composition; local receipt-time freshness; no per-file byte percentage or invented overall ETA |
| Scan again with persistent hash reuse | Engine fresh discovery; `RepeatHashCache` v3; `reuse_verified` / `revalidate_content`; immutable run policy | Existing backend capability; preserve configured cache across runs/restarts, explain reuse and test changed membership; no new cache/database or run-diff API |
| Compact file results | `DuplicateFilesViewModel`, group/member/facet APIs | Presentation; existing server-side sort/filter meanings unchanged |
| Folder comparison | `DuplicateFoldersViewModel`, exact group/member APIs | Presentation; retain current-page Explorer selection and exact folder relationships |
| Manual Keep/Remove/Reset | `SetReviewDecisionAsync`, `SetReviewFolderDecisionAsync` | Recompose existing mutation flow; expected revision and operation identity stay authoritative |
| Review overview | `GetReviewPlanAsync`, `GetReviewGroupsAsync`, `GetReviewFolderGroupsAsync`, member pages | New Core/WPF composition of existing endpoints; separate Files/Folders pages, no full-plan materialization |
| Location preferences | `PreferenceRulesViewModel`, preview/application/reversal APIs | Move/disclose UI; preserve preview scope, signature, confirmation and provenance |
| Check marked copies | `PreflightViewModel`, latest/start/items/cancel APIs | Recompose existing whole-plan non-deleting workflow |
| Current-copy checks | `ValidateReviewFilesAsync`, dirty-root reconciliation APIs | Explicit bounded action; distinct from preflight, no automatic full-run crawl |
| Warning details | `RunWarningDrilldownViewModel`, `GetRunWarningsAsync` | Reuse one contextual component; preserve current/terminal revision boundaries |
| Performance detail | `PerformanceViewModel`, snapshot/run-page APIs | Recompose summaries; no time-series endpoint or trend claims |
| History paging | `ListRunsAsync(offset,limit)`; History currently loads up to 500 | Core/WPF paging may be required; do not pretend the current 500-run collection is unlimited |
| Operation/recovery evidence | `RecycleOperationViewModel`, `RecoveryReviewViewModel` | Keep existing evidence/observation access when relevant; execution and resolution remain unavailable |

## Boundaries to preserve

| Channel | Existing presentation limits / behavior |
|---|---|
| File groups and members | 200 rows per page, five-page caches; query generations reject stale data |
| File root and drive facets | 25 options per page, independent five-page caches |
| Folder groups and members | 200 rows per page, bounded caches; keep native reveal bounds |
| Warning drilldown | 25 rows, five cached pages, at most three retained representative examples per aggregate |
| Performance | 25 history rows, six phase rows, up to 64 device rows; no raw sample arrays |
| Product history | Current 500-row fetch; proposed page navigation remains bounded using existing API |
| Progress | At most ten ordinary visual updates/second, latest-only application; separate UIA cadence |
| Review overview | Proposed 200 groups per Files/Folders page and existing 200-member detail pages; no global dictionary |

Check exact constants in the owning source at each implementation slice; a changed dependency is
a documented reason to revise this map. Do not increase a cap solely to simplify the new layout.

## Deliberate limits in the design

The current review API returns group summaries and paged members. A global flat removal table,
arbitrary 'all undecided' filter, per-drive planned-savings breakdown, unified cross-mode sorted
list, full-run select-all, and arbitrary undo stack would require contract/product work. They are
not required or advertised in this design. Review shows separate group pages and the existing
combined totals. Reversal uses the existing rule-application contract; Reset returns a manual
decision to undecided.

The initial findings about loading and counter context require isolated characterization before
causal bug claims. The design should eliminate mixed-context presentation regardless of whether
the original delay is a query, filesystem validation, or an artifact/version mismatch.

## Existing regression anchors

Use `ShellSessionWorkflowTests`, `ShellViewModelTests`, `SessionSetupViewModelTests`,
`DuplicateFilesViewModelTests`, `DuplicateFoldersViewModelTests`, `PreferenceRulesViewModelTests`,
`PreflightViewModelTests`, `RecycleOperationViewModelTests`, `RecoveryReviewViewModelTests`,
`RunWarningDrilldownViewModelTests`, `PerformanceViewModelTests`, `WorkerClientLifecycleTests`, and
`WpfSurfaceSmokeTests`. Extend these around observable behavior rather than rewriting them to
assert new control names or mirroring implementation details. Preserve contract/scale evidence.
