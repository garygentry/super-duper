# UIR-07b contextual Performance evidence

Date: 2026-09-14

Scope: local WPF/Core implementation of the Performance portion of S05 for A12/A13/A15 and the
exact-run context portions of A16. UIR-07 remains in progress for integrated/native acceptance.

## Implemented behavior

Performance is reachable in one action from the highlighted History run, active Progress run, or
stopped-run Summary. The detail header names that exact scan, its saved session, date and state, plus
the worker telemetry row and metrics-contract version. A mismatched product-run snapshot or selected
comparison snapshot is rejected instead of being displayed under the requested scan. Leaving a
contextual view through the tabs clears its temporary override, so direct Performance navigation
returns to the stable opened workspace run.

Close restores focus to the exact History, Progress or Summary entry that opened the detail. Direct
tab navigation returns to the opened run in History. Active performance inspection never retargets
an older completed Results/Review workspace run.

The detail composes existing worker-owned summaries in this order: at most six phase rows; partial
and full cache hit/miss/error/store summaries; logical candidate versus partial/full actual bytes
read; host CPU and memory; then a virtualized selector of at most 64 recorded devices. Selecting one
device exposes capacity/free-at-start and labeled current/peak read, operations, latency, active-time
and queue summaries without a wide 16-column grid. Missing counters remain **Unavailable**. Logical
candidate bytes are explicitly not labeled as disk throughput.

Retained comparison stays bounded to 25 rows. It names both product-run and telemetry identities and
labels volume/device, scan-input or software-build differences before showing values; differing
contexts explicitly are not like-for-like. The screen states that current and peak values are
persisted summaries and that raw samples and time-series data are unavailable. No chart, raw sample,
per-file timing or time-saved claim was added.

## Preserved contracts

- UIR-07a keeps explicit highlighted/opened/active identities, immutable recorded settings, bounded
  500-run History pages, 25-row/five-page current/terminal warnings, retained accepted pages, stable
  completed-run result navigation and exact History/Progress/Summary warning return focus.
- UIR-06a keeps worker-owned combined totals, separate 200-row Files/Folders queries, independent
  five-page caches, exact links and validation states. **Check marked copies** remains whole-plan;
  **Check these copies** remains visible-page.
- UIR-06b keeps ordered roots, virtual preview scope/revision/signature, scope/count confirmation,
  provenance, fresh-preview enforcement, **Reverse rule application**, later manual overrides and
  Reset semantics. UIR-04/05 monitoring, rescan, query, comparison, decision, navigation, reveal and
  focus contracts remain covered.
- No worker/protocol/database/cache contract, recovery outcome, full A17 behavior or execution path
  changed. Production still uses `DisabledRecycleOperationCapabilityExecutor`; `CanSubmit=false`;
  `executorEnabled=false`; no production execution action exists.
- No production state, physical/provider/performance campaign or parked release-validation work was
  touched. PID 67748 was re-audited responsive at the exact older UIR-03f fixture executable and was
  not reused, focused, closed or overwritten.

## Verification

| Check | Result |
|---|---|
| Full Core test project, isolated under `artifacts/uir07b/core-final-2` | **220 passed**, `artifacts/uir07b/results-final/core-final.trx` |
| Three loaded-STA WPF surface/bounds methods, isolated under `artifacts/uir07b/wpf-final-2` | **3 passed**, `artifacts/uir07b/results-final/wpf-final.trx` |
| Redesign fixture build under `artifacts/uir07b/fixture-final-2` | passed, **0 warnings / 0 errors** |
| Production-lock/PID source audit and `git diff --check` | passed before commit |

The new Core coverage proves exact History/Progress/Summary run and return origin, stable opened-run
navigation after an abandoned contextual view, snapshot identity rejection, explicit unavailable
values, selected-device retention and exact 25/6/64 boundaries. The loaded-STA fixture exercises real
MainWindow focus, all three contextual entries, device selection, comparison qualification,
vertical-only reachability and virtualization at 1180x760, 900x600 and the toolbar stress viewport.
There are 125 PNGs under `artifacts/uir07b/captures-final-2`; representative exact-context,
selected-device and comparison states were visually reviewed.

Retained diagnostics include one Core compile failure caused by a misplaced test helper, one initial
WPF binding failure from a read-only fake selected-device property, and sandbox-only Windows SDK read
denials. Each was corrected or rerun with the same isolated scope; failed outputs were not overwritten.

## Remaining scope

UIR-07a/b are locally implemented; UIR-07 remains `in_progress`. The next dependency-ready slice is
UIR-08 local integration regression and acceptance-matrix preparation across UIR-04 through UIR-07.
That work must retain honest physical/skipped states and request any distinct operator authority
before a native action. NVDA and physical 200% remain `unrun_unavailable`; full A17, native/operator
acceptance, recovery resolution, production execution and parked release validation remain open.
