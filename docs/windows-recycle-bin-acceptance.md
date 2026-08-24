# Windows Recycle Bin Operator/Provider Acceptance

## Status and safety boundary

This is the repeatable acceptance procedure for the separately gated Windows
`IFileOperation` executor. It collects evidence; it does not enable the executor. Production still
injects `DisabledRecycleOperationCapabilityExecutor`, the worker still reports
`executorEnabled:false`, `RecycleOperationViewModel.CanSubmit` remains false, and the WPF app has no
**Move to Recycle Bin now** action. Milestone 11 is not complete.

Never use this workflow on user data. It must not permanently delete anything, deliberately execute
a subset of a reviewed plan, access excluded Cloud Files content, write Milestone 12 state, or use
`scanned_file.marked_deleted`/legacy `deletion_plan` as operation truth. Successful disposable
fixtures stay recoverable in the current user's Recycle Bin until the user independently manages
them in Windows. The workflow has no cleanup switch.

## Evidence collector

Run the non-mutating contract pass first:

```powershell
./scripts/Invoke-WindowsRecycleBinAcceptance.ps1 -Configuration Release
```

The command creates a timestamped ignored directory under
`artifacts/windows-recycle-bin-acceptance/` containing:

- `acceptance-evidence.json`, a versioned machine-readable host/command/matrix record;
- `acceptance-report.md`, the same matrix for review;
- one log per executed command; and
- TRX results for focused .NET acceptance tests.

Warm-query runs additionally retain `representative-review-warm-query.json`, containing every
ordered query duration and test-process snapshot, and
`representative-review-host-context.jsonl`, containing time-stamped host and competing-process
samples. Evidence schema v2 requires a new or empty output directory; the default timestamp
includes milliseconds. The collector refuses to overwrite a prior run, including a failed run.
It also rejects a reparse point anywhere between the repository and evidence directory so the
repository boundary cannot be redirected to an unrelated location.

Every report explicitly records `productionEnabled:false` and `milestone11Complete:false`. A
missing physical/provider prerequisite is `not_run` or `open`, never a pass. The collector requires
an evidence directory inside the repository so an operator cannot accidentally overwrite an
unrelated location.

The explicit local Shell campaign remains opt-in because it moves uniquely named fixtures:

```powershell
./scripts/Invoke-WindowsRecycleBinAcceptance.ps1 `
  -Configuration Release `
  -ConfirmRecycleBinMutation
```

This wraps `Invoke-WindowsRecycleBinSmoke.ps1` and covers a positive hard-link alias, an isolated
exact-folder root, cancellation at `PreDeleteItem`, and a locked source. It records real
`PostDeleteItem` recycled-item evidence, outer/finish HRESULTs, and the abort query. It does not
exercise capacity, provider, disconnect, access-denied, process-loss, or large-plan behavior.

## Real-provider no-hydration pass

Fixture selection is deliberately manual. The collector never searches a registered root or reads
file content to find candidates. Supply one locally available file and one zero-allocation
offline/recall placeholder from the same exact registered Cloud Files root, plus the provider's
actual process name:

```powershell
./scripts/Invoke-WindowsRecycleBinAcceptance.ps1 `
  -Configuration Release `
  -RunProviderNoHydration `
  -CloudRoot 'C:\Users\operator\OneDrive' `
  -LocallyAvailableFile 'C:\Users\operator\OneDrive\acceptance\local.bin' `
  -OfflinePlaceholder 'C:\Users\operator\OneDrive\acceptance\offline.bin' `
  -ProviderProcessName 'OneDrive'
```

The test first proves the exact root is returned by `StorageProviderSyncRootManager`. It then uses
only `GetFileAttributesW`, `GetFileAttributesExW`, and `GetCompressedFileSizeW` for fixture
evidence. Executor inspection repeats its non-opening attributes check and root
`SHQueryRecycleBinW` query. No file handle is opened and no content API is called. Before/after
attributes, logical size, allocated size, and last-write time must match; the offline fixture must
remain zero-allocation; and `GetProcessIoCounters` read/write/other transfer counts plus the process
set must remain identical. Pause unrelated sync activity before running so provider background work
does not create false evidence.

This pass proves the Infrastructure capability seam does not hydrate the supplied placeholder. It
does not authorize the worker to admit a cloud-root item: immutable registered/manual exclusions
still reject it lexically before target I/O, and provider-backed Shell mutation remains unsupported.

## Acceptance matrix

| Gate | Automated evidence | Required operator evidence | Current disposition |
|---|---|---|---|
| Positive local capability | Ordinary non-opening classification plus successful `SHQueryRecycleBinW` | Repeat on each supported fixed/removable filesystem | Local development-host pass only |
| Callback/abort success | Dedicated STA, durable begin, positive `PostDeleteItem`, finish/outer/abort capture | Repeat across supported Windows builds/filesystems | Local development-host pass only |
| Cancellation | Queued cancellation and real `PreDeleteItem` cancellation | Delayed/current-item and provider cancellation observations | Open |
| Locked/sharing violation | Real locked source plus stable `0x80270027` mapping | Repeat on supported filesystems/providers | Local development-host pass only |
| Access denied/elevation | Stable Win32/copy-engine mappings | Disposable ACL/elevation boundary with unchanged source | Open |
| Root disconnect | Stable Win32/copy-engine mappings | Controlled removable/mapped/provider disconnect after durable begin | Open |
| Capacity/oversized item | Stable copy-engine capacity mappings | Disposable volume/account with positive no-fallback evidence | Open |
| Provider HRESULTs | Stable unavailable/failure/paused mappings | Real provider outcomes without hydrating an excluded placeholder | Open |
| Provider no hydration | Opt-in metadata/allocation/transfer-counter test | Qualifying registered provider and explicit fixtures | Environment-gated |
| Residual Shell TOCTOU | Admission and `PreDeleteItem` identity/type/size/time checks | Controlled path swap inside the residual Shell interval | Open |
| Ambiguous start | Durable `shell_started`, restart reconstruction, non-retry tests, and exact campaign verifier | Controlled process loss plus manual Recycle Bin/source inspection through WPF Option A | Accepted on the development host; retained evidence `artifacts/windows-ambiguous-start/20260823-144048-588` |
| Large plan | Bounded database/protocol/UI caches and 32-entry validation | Representative disposable operation timing and memory | Open |
| Physical accessibility | UI Automation, keyboard, narrow-layout tests | Narrator/NVDA, OS high contrast, 100/150/200% monitor transitions | Open |

Deterministic tests now assert every documented Shell reason-code mapping, including access denied,
sharing violation, disappearance, root disconnect, capacity, unsupported recycling, long paths,
Recycle Bin unavailability, provider unavailable/failure/paused, cancellation, and the unmapped
fallback. Those tests prevent contract drift; they are not substitutes for real HRESULT evidence.

## Failure campaigns

Use one separately reviewed disposable campaign per unavailable prerequisite. Record exact fixture
creation, filesystem/provider version, the durable-start boundary, callback order, every numeric
HRESULT, recycled-item presence, finish/outer result, abort-query result, source/survivor state, and
Recycle Bin observation. A campaign is invalid if it requires disabling exclusions, reading an
offline placeholder, accepting Shell UI, falling back to permanent deletion, or clearing ambiguous
evidence.

- Access denied: use a disposable ordinary local file and a reversible ACL owned by the operator.
  Confirm the source bytes/identity remain unchanged and restore the ACL after the executor exits.
- Disconnect: use non-production removable or mapped/provider media that already has positive
  Recycle Bin capability. Disconnect only after durable batch begin; never simulate this by turning
  a production root into an unsupported heuristic.
- Capacity: use a disposable test account/volume with a controlled Recycle Bin constraint. A
  capacity result must leave the source intact; Windows prompting or permanent fallback fails the
  campaign. Do not fill the system drive or permanently empty the Recycle Bin to manufacture it.
- Provider: use provider-owned disposable files that are locally available. Offline/recall
  placeholders remain observation-only and must never be submitted to Shell.
- Ambiguous start: run `Invoke-WindowsAmbiguousStartCampaign.ps1`; after its durable-start marker,
  terminate only the disposable test host—not Explorer, a provider, or the worker database—and
  preserve the migrated v11 database/logs. Complete the real WPF Option A checklist, close WPF
  normally, then run `Verify-WindowsAmbiguousStartCampaign.ps1` against that evidence directory.
  Restart must show `recovery_required`; the batch must never be retried or replayed. Failed
  attempts remain separate timestamped bundles and never qualify as acceptance evidence.

## Performance and constants

The independent 100-sample warm-query profile can be added to an evidence run:

```powershell
./scripts/Invoke-WindowsRecycleBinAcceptance.ps1 `
  -Configuration Release `
  -SkipBuild `
  -RunWarmQueryProfile
```

This runs `representative_review_workspace_profile`. It is Milestone 8 query evidence only and does
not close representative large-plan operation performance.

The structured query file records all 500 monotonic query intervals (100 each for group/summary,
selected-root facet, drive facet, review plan, and review groups), p50/p75/p90/p95/p99/max for each
category, and 101 test-process snapshots: a baseline plus one after each iteration. Windows
snapshots contain cumulative user/kernel CPU, private and working-set memory, and read/write/other
operation and transfer counters. The host JSONL sampler uses persistent native performance
counters for CPU, available/committed memory, paging, disk throughput/utilization/queue/splits,
processor queue, context switches, process count, and thread count. It also samples competing
process CPU, working set, and transfer deltas on a nominal two-second interval. Each record includes
the measured interval and sampler PID so observer cost is visible rather than silently attributed
to the query process.

Use p50/p75/p90 to describe the stable-cost body and p95/p99/max plus the ordered intervals for the
tail. Align those offsets with the UTC profile window and host samples when investigating
simultaneous pressure. Host contention can explain a development-host tail; it cannot convert a
failed p95 into a pass, waive the unchanged 100 ms target, or make the current host representative.
Sampler initialization or counter unavailability is retained as an `unavailable` record, and a
partial final JSONL line is counted rather than causing earlier evidence to be discarded.

The 2026-08-20 read-only query-plan stabilization removed the per-group correlated across-drive
summary probe and bounds non-name member-detail enrichment after keyset candidate selection. On the
current development host, the pre-change session run was 75.45/136.64/176.07 ms p50/p95/p99 for
group/summary. Two final runs passed at 54.77/62.11/116.70 ms and 55.22/93.01/199.87 ms; a third
retained run failed at 55.11/198.72/283.01 ms while the independent root/drive facets also spiked to
79.93/122.30 ms p95. The collector must retain that failure. The lower stable baseline does not
close the representative-hardware gate, and an operator must not discard failures by rerunning
until only a pass is shown.

The first instrumented development-host run on 2026-08-20 also failed and remains retained. Its
group p50/p75/p90/p95/p99/max distribution was
52.68/54.76/94.74/140.76/243.87/728.16 ms. Selected-root, drive, review-plan, and review-group p95
were 63.42/69.71/68.81/5.19 ms, and private growth was 880,640 bytes. All 500 query intervals and
101 process snapshots survived the failing assertion. Three coarse initial host samples overlapped
the profile and showed an unrelated backup process using roughly 54-66 MB/s and up to 60% of one
logical processor; one sample also recorded a processor queue of 10. This is evidence of concurrent
development-host pressure, not proof that every tail came from that process and not acceptance.
The heavier initial formatted-counter sampler was then replaced by the persistent lower-overhead
sampler described above; a separate read-only probe verified its CPU/I/O ranking. Do not rerun the
profile merely to replace this retained failure with a pass.

The first designated-representative-host attempt on 2026-08-24 is retained at
`artifacts/windows-representative-query/20260824-161340-948`. The collector stopped during its
Release Infrastructure prerequisite build because the execution sandbox denied access to
`C:\Users\gary\AppData\Local\Microsoft SDKs`. The schema-v2 evidence and build log are valid
diagnostics, but this is not a valid measurement: it contains 0 of 500 query intervals, no host
JSONL, no process snapshots, and no p95 metrics. It neither passes nor numerically fails the gate.
Do not overwrite or rerun it merely for a pass. The smallest separately authorized experiment is
one unchanged collector invocation in a new evidence directory from a desktop execution context
that can read the installed Windows SDK, after reconfirming that the designated machine is under
normal load.

The separately authorized desktop-context run on the same designated representative Windows 11
x64 machine under normal load passed and is retained at
`artifacts/windows-representative-query/20260824-162843-832`. It contains all 500 ordered query
intervals, 101 process snapshots, and 12 valid host-context samples with none invalid or unavailable.
The unchanged 100 ms p95 target passed for group/summary at 62.57 ms, selected-root facet at
32.03 ms, drive facet at 32.07 ms, review plan at 17.00 ms, and review groups at 4.69 ms. Group p50
and p99 were 54.29 and 65.70 ms; retained private growth was 929,792 bytes. The Release build,
31 deterministic Infrastructure contracts, and the exact ignored Rust profile all passed while
`productionEnabled:false` and `milestone11Complete:false` remained recorded. This accepts only
`WPM8-representative-query-performance`; it is `locally_exhausted` and must not be rerun merely for
another pass. It does not close representative large-plan operation performance or any provider,
mutation, physical-accessibility, outcome, or later gate.

A qualifying large-plan operation run must record at least preflight-completion-to-prepare time,
operator confirmation reading time, confirmation submission age, per-batch fresh-admission time,
capability inspection time, durable-begin-to-report time, batch/result paging p50/p95/max, retained
private-memory growth, cancellation latency at each boundary, and the effect of exact-folder and
hard-link batches. Record the plan's logical paths, Shell entries, unique physical items, exact
folders, bytes, affected groups/locations, batch count, and filesystem/provider mix without listing
unrelated user paths.

The five-minute preflight freshness, 60-second confirmation, 30-second admission, and 32-entry file
batch bounds remain provisional. Automated tests prove expiry and enforcement, not usability. A
final decision must cite representative measurements and operator comprehension. Tightening is
compatible; loosening requires a separately reviewed, versioned freshness-policy change.

`FOFX_ADDUNDORECORD` remains omitted. The deterministic flag test asserts the exact current flags
and absence of `0x20000000`. Decide it only after comparing supported Windows builds with real
positive, partial, cancelled, ambiguous, and provider outcomes. Windows Undo must never be described
as app-owned restore or used to erase durable recovery evidence.

## Physical accessibility procedure

On an interactive Windows 11 x64 desktop, separately run Narrator and NVDA through final
confirmation disclosure, immediate/between-batch/current-Shell cancellation wording, success,
partial failure, and recovery-required summaries. Repeat under a Windows high-contrast theme and
move the app between physical 100%, 150%, and 200% monitors while confirmation, progress, result,
and location/detail pages are open. Record focus, announcement order/coalescing, clipping, popup
placement, and keyboard-only access. Automated WPF tests cannot be used to mark these physical rows
passed.

The first separately authorized physical-reader bootstrap is retained at
`artifacts/windows-narrator-nvda/20260824-171812-501`. The physical prerequisites were confirmed and
Narrator started, but the control helper returned `failed to activate captured window` before the
first workflow key; a fresh enumeration returned one Super Duper window and the one permitted retry
failed identically. No valid listening observation or product defect was produced, and NVDA was not
started. The gate remains blocked. The smallest separately authorized experiment is one new-
directory attempt where the operator manually foregrounds the isolated app before the agent binds
without `activate_window`; no other accessibility or acceptance campaign may substitute for it.

The read-only WPF reconstruction now exposes state-specific cancellation wording and an assertive
`recovery_required` warning that forbids retry, directs the operator to inspect every unknown source
and Recycle Bin item, and states that this build cannot resolve or replay the operation. Automated
Core and STA coverage lock that contract while `CanSubmit` remains false. It also exposes a
selectable, path-free recovery summary with the stable operation key, evidence record, run,
preflight, review revision, policy version, immutable preflight/intent signatures, lifecycle times,
cancellation-request state, outcome counts, and stored error code for diagnostic handoff. The
reconstructed result list is automatically filtered to the operation's stored `unknown` results so
the operator does not have to page through known outcomes before physical inspection. Page changes
report and politely announce the exact item range and stored unknown total, allowing a bounded review
to account for every result. Initial reconstruction announces the first committed range, and
previous-page navigation repeats the committed range announcement; stale or cancelled page
responses remain silent. A failed forward or cache-evicted backward page fetch retains the last
committed range and navigation history, reports an assertive page error, and can be retried without
skipping the requested page. A successful exact retry commits the same cursor-history transition as
the original forward or backward request, so both navigation directions remain repeatable after the
retry, including when an older page was evicted from the bounded cache. A failed initial fetch
preserves the durable operation and recovery summaries and exposes an explicitly read-only retry of
that exact detail request. If operation-summary reconstruction itself fails before a durable
operation is returned, the same affordance retries the exact selected-run read and cannot replace a
newer run context. A late failed operation-summary retry is likewise rejected with its stale run
generation and cannot replace the newer committed operation/page context, publish feedback, expose
another retry, or change navigation. A completed reconstruction read with no stored operation
intent announces that empty result through the polite success channel and does not expose retry,
including when an exact retry resolves an earlier operation-summary load failure. That successful
empty retry clears the prior assertive error payload without publishing a second assertive
notification. A successfully committed page retry does the same. Retrying detail loading never
retries or resolves the operation. A late failed page retry is rejected with its stale run
generation and cannot replace the newer committed page, publish an error or result announcement,
expose another retry, or mutate the newer page's cursor history. Each row exposes
its durable operation/preflight/batch identifiers,
source snapshot context, result time, stable code, numeric Shell HRESULT, and recorded recycled-item-
presence value for correlation. This reads durable evidence only; it does not inspect the filesystem,
resolve an unknown item, or replace any Narrator/NVDA, high-contrast, or physical multi-monitor DPI
observation above.

The read-only automation surface routes reconstruction and paging failures through a dedicated
assertive `ActionAborted`/`ImportantMostRecent` notification channel. Successful committed
reconstruction and page changes remain separate repeatable `ActionCompleted` notifications. This
automated contract includes a loaded-STA regression that raises both channels through UI Automation
peers and verifies their distinct payload, event kind, and activity identifier. It does not close
the physical accessibility rows above.

## Review boundary

Attach the JSON, Markdown, logs, TRX, host description, and operator notes to a separate review.
Production wiring may be considered only when every required row is supported by qualifying
evidence, the constants and `FOFX_ADDUNDORECORD` have explicit reviewed decisions, ambiguous
recovery has an accepted accessible product workflow, and independent Milestone 8 gates are
dispositioned. Until then,
keep the real executor unwired and do not claim Milestone 11 complete.

`WPM11-recovery-workflow` accepted Option A by explicit user review on 2026-08-23. Its
[decision package](windows-post-mvp-ux-plan.md#wpm11-recovery-workflow-decision-package-2026-08-23)
requires append-only per-item operator observations while leaving original `unknown`, `ambiguous`,
and `recovery_required` evidence immutable. Automatic live-state inference, replay, and overwriting
the original Shell outcome remain prohibited actions. `WPM11-recovery-review-persistence` is accepted:
schema v11 supplies bounded append-only observation/supersession persistence, derived review state,
restart reconstruction, worker methods, and non-UI client contracts without live inspection or
source-evidence mutation. `WPM11-recovery-review-ui` is accepted: the bounded WPF checklist
reconstructs all three states, pages every immutable unknown item and append-only history record,
records all five observations, shows superseded observations and correction reasons, copies stored
evidence/path, opens the Recycle Bin without querying it, and navigates to a fresh scan without
replay. Exact read and idempotent mutation retry, stale/cancelled/late rejection, keyboard/focus,
automation names, and separate success/error announcements are covered by the named verifier.
`WPM11-ambiguous-start` is accepted on the development host from
`artifacts/windows-ambiguous-start/20260823-144048-588`; its exact verifier retained two immutable
unknowns, one ambiguous batch, one recovery-required operation, two recovery rows, three append-only
observations, and the explicit supersession reason while proving the original source rows unchanged
and all retry/replay/resubmit/inference/restore/delete/copy-forward flags false. The earlier safe
prepare failure remains at `artifacts/windows-ambiguous-start/20260823-143955-657`. This evidence
does not accept provider, residual-TOCTOU, performance, physical-accessibility, or production-wiring
gates.
