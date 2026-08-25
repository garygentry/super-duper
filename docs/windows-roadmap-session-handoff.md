# Roadmap Session Handoff

Use this document as the kickoff context for each new coding session. At the end of every completed
slice, update the checkpoint, verification totals, immediate next step, and decision log here before
committing. Every session that changes the worktree must commit all of its completed, in-scope work,
including this handoff update, before handing back to the user. Keep this document concise; the
linked plans remain authoritative.

## Session objective

Advance one named gate or one explicitly named coherent gate group from the currently scheduled
roadmap stream. The large-drive scan optimization and observability stream is active. The Windows
post-MVP closure ledger is parked as a release-validation checklist and must resume before final
feature-complete. Verify in proportion to risk, update the selected stream's authority and this
handoff, and commit the bounded slice as one focused commit before ending the session. Do not leave
completed session work uncommitted or substitute work from the parked stream.

## Current checkpoint

- Branch: `wpf-poc`
- Latest completed slice: accept the versioned platform-neutral SOP2a progress contract and reducer
  with deterministic rate/ETA/invariant evidence (this session's commit)
- Worktree after that commit: clean
- Active stream: large-drive scan optimization and observability
- Active plan: `docs/scan-optimization-plan.md`
- Current gate: `SOP2-progress-reporting`
- Next work package: `SOP2b-incremental-pipeline-publication`
- Reusable new-session prompt: `docs/scan-optimization-kickoff-prompt.md`
- Prior D: stress run: stopped by the operator; no live scan must be preserved for this work
- Parked stream: Windows post-MVP release validation; resume at `WPM8-high-contrast` before final
  feature-complete
- MVP Milestones 0-6: implemented and code complete
- Milestone 7 required fail-closed cloud safety: accepted; both unavailable opt-in policies are
  reviewed deferred follow-ons
- Milestone 8 read-only criteria and representative query performance: accepted; three physical
  accessibility gates remain open
- Milestone 9: all four criteria are accepted; bounded folder relationships, responsive
  single-folder reveal, current-page parent-grouped selection, and no-double-schedule evidence are
  complete
- Milestone 10 review/rule criteria: all accepted; no uncovered acceptance criterion remains
- Milestone 11: non-deleting preflight, durable operation contract, separately gated native executor,
  and acceptance evidence tooling are implemented; production execution remains disabled and the
  milestone is not complete
- Milestone 12: bounded external deletion/modification invalidation, durable watcher-overflow
  dirty-root reconciliation, and bounded watcher-event coalescing are accepted; in-app deletion
  outcomes depend on Milestone 11 outcomes
- Milestone 13: bounded warning drilldown, retained 100,000-aggregate memory evidence, and the first
  stable-target warning action are accepted; outcome audit remains blocked
- Milestone 14: planned; required scope and the operator-accepted/production-enabled completion
  contract are accepted; four reviewed follow-ons are deferred

The current implementation slice accepts `SOP2a-progress-contract-reducer`. Progress contract v1
keeps nine missing logical-work meanings separate from durable metrics v2, preserves physical-I/O
units for displayed partial/full rates, and computes ETA only from stable same-unit logical
candidate resolution. Rate history uses 100 ms buckets, a 30-second/304-point bound, and two
positive five-second intervals within a ten-second warm-up; delayed/unequal intervals and sub-one-
byte-per-second progress remain measurable. Cache hits never count as physical reads, phase changes
partition recent/ETA history, the unused synthetic full-hash phase is rejected, active-device state
matches the accepted 64-device ceiling, and every invalid transition is atomic. No pipeline,
status-schema, worker, .NET/WPF, profile, physical campaign, or parked-stream work changed.

The preceding documentation slice starts `SOP2-progress-reporting` with six finite packages. The
read-only audit found that live telemetry is copied only at phase completion, hash progress waits
for a whole exact-size bucket, several logical-byte meanings are absent, worker phase events bypass
the normal 100 ms throttle, and delayed higher-sequence Core events can revive running state after
cancelling or terminal state. `SOP2a` therefore establishes the versioned cumulative contract and
pure fake-clock projection semantics before any pipeline, worker, Core, or WPF integration. No
product code, profile, physical campaign, or parked-stream work changed.

The preceding implementation slice accepts `SOP1f-foundation-acceptance` and therefore
`SOP1-telemetry-foundation`. Audit of the retained +4.68% wall/+1.98% CPU failure found that each of
ten phase flushes performed 43 separate counter reads plus 43 separate upserts. Each flush now uses
one committed-counter read and one atomic multi-row upsert, and unchanged counters retain the
sequence at which their values last changed. Exact replay, monotonic regression rejection, all 43
fixed summaries, the single writer, five-second heartbeat, bounded retention, and terminal behavior
remain intact. The first and only post-change comparable Release profile in
`docs/evidence/scan-telemetry-overhead-20260825-counter-batching.json` passed the unchanged threshold
at -2.10% wall/-22.76% CPU. Those negative deltas are retained as noise/no detected positive
overhead, not claimed acceleration; the original failed evidence remains unchanged.

The same audit confirms that repeat-run full hashes already persist globally across scans/process
sessions for canonical-path/size/nanosecond-time matches. Partial reads always repeat, renames miss,
eviction is unbounded, and there is no UI policy. The operator's proposed reuse control is viable
and is now explicit `SOP8-repeat-run-cache` scope, including forced revalidation, stronger
identity/time invalidation, partial/full cache accounting, and same/cross-session measurement. No
SOP8 or UI implementation was pulled ahead of its dependencies.

The preceding implementation slice accepts `SOP1e-host-device-sampler`. A platform-neutral cadence
controller accepts the writer's sequence/phase, caps sample count, suppresses early samples, and
reports delayed intervals under a fake clock without owning a thread or database connection. The
Windows implementation maps volume roots to physical disk numbers, captures filesystem/capacity/free
space plus process/system CPU, memory, and I/O, and derives disk read throughput, IOPS, latency,
active time, and queue depth from cumulative native counters. Hardware serials are not queried or
persisted; blocked counters remain explicit unavailable values.

The preceding implementation slice accepts `SOP1d-bounded-queries-retention`. Status history now has
strict descending run cursors (maximum 100), ascending host/device sample cursors (maximum 500),
fixed counter/phase/device summaries, and an atomic 64-device ceiling. Default repeatable retention
keeps 50 terminal runs and 100,000 samples per run, never deletes active runs, removes terminal replay
payloads, and runs at startup and terminal completion. WAL auto-checkpoints at 1,000 pages and
retention/history deletion use a passive checkpoint with explicit unavailable frame counts. The
separate product database remains unchanged when terminal status history is deleted. The next
package is `SOP1e-host-device-sampler`.

The preceding implementation slice accepts `SOP1c-run-lifecycle`. Worker scans now default a separate
`scan_status.db` beside the product database, with an environment/options override. Metrics contract
v2 distinguishes the actual hash pipeline from disjoint multi-file duplicate candidates so the
current singleton-read baseline and later saved I/O are both measurable. Discovery, partial/full
hash, cache, duplicate, byte, warning, and reliability counters flush only at phase boundaries;
completed, cancelled, and failed product runs receive matching best-effort status terminal state.
A normal deterministic scan writes ten cumulative flushes and no per-file telemetry rows. The next
package is `SOP1d-bounded-queries-retention`.

The preceding implementation slice accepts `SOP1b-status-store`. Status schema v2 adds an exact
sequence/payload replay ledger. One immediate transaction now owns run start, monotonic cumulative
counter snapshots, phase state, device descriptors, host/device samples, and terminal state.
Conflicting replay, counter/phase regression, invalid sequence/timestamp, unknown device, numeric
overflow, and non-writable run state fail without a partial write. Startup reconciles abandoned
running/cancelling runs and phases to interrupted while preserving committed samples. The next
package is `SOP1c-run-lifecycle`.

The preceding implementation slice accepts `SOP1a-contract-schema`. Rust now owns metrics-contract v1
counter/gauge types and invariant validation plus a separate schema-v1 status database with fixed
run/phase/counter/device/host-sample/device-sample tables. Empty create and reopen are idempotent;
unversioned non-empty and newer databases fail without modification. No scan lifecycle write,
product database, protocol, worker, Core, WPF, or execution-lock behavior changed. The next package
is `SOP1b-status-store`.

The preceding plan-control follow-up makes that stream idempotent across agent sessions: it adds a
finite work-package ledger, audit-once and two-identical-failure anti-spin rules, multiple-package
session progress, explicit context-risk handoff conditions, and a state-independent reusable kickoff
prompt. The operator stopped the prior D: stress run, so implementation may now proceed without an
active-run preservation constraint. The next package is `SOP1a-contract-schema`.

The preceding documentation-only slice records a second durable roadmap stream for large-drive scan
optimization and observability. A read-only source and live-run audit confirms that exact-size
grouping exists but singleton buckets still receive a 1 KiB partial read, the existing
`files_hashed` counter counts partial-hash success rather than full-content hashing, and nested
parallel reads can create a seek-heavy rotational-disk workload. The new plan orders durable
metrics/status storage, progress, warnings, and the Performance tab before singleton, scheduling,
read-path, cache, and representative large-drive acceptance gates. No product code or active-run
state changed.

The Windows post-MVP closure ledger is now parked, not cancelled or deferred. Its current states,
evidence, completion contract, and production locks remain unchanged. When it resumes before final
feature-complete, its next gate is still `WPM8-high-contrast`.

The preceding documentation-only slice records the operator instruction “Mark Narrator as skipped by
operator and proceed from there.” The retained bundle
`artifacts/windows-narrator-nvda/20260824-171812-501` now records Narrator as
`skipped_by_operator` and NVDA as `not_run`. This does not accept the combined gate or waive the
reviewed Narrator requirement. `WPM8-narrator-nvda` remains blocked, while the operator has
authorized proceeding only to the independent `WPM8-high-contrast` gate.

The preceding evidence-only slice retained `artifacts/windows-narrator-nvda/20260824-171812-501` and
left `WPM8-narrator-nvda` blocked. The operator confirmed an interactive Windows 11 x64 desktop,
physical listening availability, and both readers. An isolated copy of the existing immutable-
result database was prepared and Narrator started. Before any keyboard-workflow input, the control
helper returned `failed to activate captured window`; fresh enumeration again returned exactly one
Super Duper window and the one permitted activation retry failed identically. No valid physical
observation or product defect was produced, and NVDA was not started. No product code, smoke,
provider, mutation, high-contrast, DPI, outcome, performance, or later campaign ran.

The preceding evidence-only slice accepted `WPM8-representative-query-performance` from the explicitly
confirmed designated Windows 11 x64 machine under normal load. The retained Release bundle is
`artifacts/windows-representative-query/20260824-162843-832`: all 500 ordered intervals, 101 process
snapshots, and 12 valid host samples are present with none invalid/unavailable. The unchanged 100 ms
p95 target passed at 62.57 ms group/summary, 32.03 ms selected-root facet, 32.07 ms drive facet,
17.00 ms review plan, and 4.69 ms review groups; group p50/p99 were 54.29/65.70 ms and private growth
was 929,792 bytes. The Release build, 31 deterministic Infrastructure contracts, and exact Rust
profile passed while `productionEnabled:false` and `milestone11Complete:false` remained recorded.
The earlier pre-measurement sandbox failure remains retained at
`artifacts/windows-representative-query/20260824-161340-948`. No product code, mutation/provider
switch, retry-only acceptance, or excluded campaign occurred; the gate is `locally_exhausted`.

The preceding slice accepts `WPM13-action-navigation` for exactly one existing family:
`scan/hash_recoverable_warning`. Its server-owned `runId` resolves through `run.get` to the same
completed immutable duplicate-file set before navigation. Core/Shell cancel explicit requests,
reject changed run/page context, and keep one missing target actionable without changing schema-v14
accounting/examples, cursors, five-page/25-row bounds, restart reconstruction, or terminal history.
WPF provides one aggregate-scoped automation ID, Alt+O, explicit cancellation, assertive error
feedback, exact-action focus recovery, and group-grid focus on success.

`Verify-WindowsActionNavigation.ps1`, the full Debug/Release Rust and Windows matrices, and real
non-mutating Debug/Release smoke prove the gate and every production lock. One initial verifier
bundle is retained as failed because its Cargo filter matched zero tests; two pre-acceptance Debug
smoke failures record the reproduced virtualized-column and fixture-family defects. The final gate
is `locally_exhausted`; provider/physical-accessibility, mutation, general Activity/outcomes, broad
performance, and later-gate campaigns remain untouched.

## Immediate next step

Implement `SOP2b-incremental-pipeline-publication`: connect one serialized cumulative producer to
the accepted progress snapshot and live telemetry observations, with bounded file/read publication
inside a single large size bucket. Add deterministic gated read/cache seams for intermediate
progress, terminal reconciliation, failure and cancellation behavior, while retaining current
singleton reads and bucket scheduling. Do not pull worker protocol, Core/WPF, device scheduling,
read-path tuning, warning UI, Performance tab, or `SOP8` cache UI ahead of their dependencies.

The parked release-validation resume point remains `WPM8-high-contrast`. Do not run that physical
campaign or substitute another closure-ledger gate unless the operator reschedules the stream.

## Required startup audit

Before editing:

1. Run `git status --short`, inspect recent history, and inspect the complete latest commit diff.
2. Read `AGENTS.md` and these authoritative documents:
   - `ROADMAP.md`
   - `docs/windows-mvp-plan.md`
   - `docs/windows-post-mvp-ux-plan.md`
   - `docs/windows-recycle-bin-acceptance.md`
   - `docs/windows-roadmap-closure-ledger.md`
   - `docs/scan-optimization-plan.md`
   - `docs/scan-optimization-kickoff-prompt.md`
3. Confirm that the checkpoint above still matches `HEAD` and the worktree.
4. Confirm which roadmap stream is scheduled, identify its exact authorized gate ID, and confirm
   dependencies. Do not select from the parked stream. During a bootstrap only, inventory gates
   instead of selecting implementation.
5. Inspect code and tests only for the authorized gate, then state the evidence that permits edits,
   the verifier that closes it, and which gates remain outside the slice.

If newer commits exist, treat Git and the authoritative plans as truth, then update this document
before committing the next slice.

## Non-negotiable boundaries

- Production Recycle Bin execution remains disabled.
- Keep `RecycleOperationViewModel.CanSubmit` false.
- Keep production injection on `DisabledRecycleOperationCapabilityExecutor`.
- Keep every worker response `executorEnabled:false`.
- Do not add a **Move to Recycle Bin now** action.
- Do not inspect the live filesystem to resolve ambiguous outcomes.
- Do not replay or resolve recovery-required operations without an explicitly reviewed product
  workflow.
- Do not weaken acceptance or performance thresholds.
- Do not rerun performance profiles merely until green; retain passing and failing evidence.
- Do not claim physical hardware, provider, Narrator/NVDA, high-contrast, multi-monitor DPI,
  recovery-resolution, or representative-performance acceptance without qualifying evidence.
- Keep physical/operator/provider/performance gates distinct from locally implementable work.
- Do not edit product code without a named `local_code` gate and a reproducible defect, failing
  test, or specific uncovered acceptance criterion.
- Do not add a test-only slice unless it closes a named criterion or protects a concrete defect
  discovered while advancing that criterion.
- Audit a surface once per gate. If no local gap is found, record `locally_exhausted` and do not
  repeat the audit until new evidence, code, or a reviewed decision changes the boundary.
- After two consecutive attempts with the same blocker and no new evidence, stop the goal and
  preserve the diagnostic result instead of trying adjacent speculative changes.
- Missing hardware, provider fixtures, operator access, or product decisions are stopping
  conditions, not authorization to substitute unrelated work.
- Do not search unrelated TODOs, refactor opportunistically, or pull a later milestone forward to
  keep a goal active.
- Keep `super-duper-core` UI-agnostic.
- Keep WPF views in the executable, application contracts/view models in Core, and process/native
  concerns in Infrastructure.
- Do not alter unrelated formatting or unrelated user changes.
- Keep performance/status persistence worker-owned and separate from immutable product-result,
  review, preflight, and operation truth.
- Never interrupt or mutate an active operator scan for diagnostics unless the operator explicitly
  authorizes that action.

## Open evidence and decision gates

These remain open unless a later committed slice cites qualifying evidence or an explicit reviewed
decision:

- `SOP2` through `SOP9` in `docs/scan-optimization-plan.md`
- Physical Narrator and NVDA acceptance
- Windows high-contrast acceptance
- Physical 100/150/200% multi-monitor DPI acceptance
- Real-provider no-hydration and provider-outcome campaigns
- Controlled access-denied, disconnect, capacity, and path-swap outcomes
- Representative large-plan operation performance
- Final freshness, confirmation, admission, and batch constants
- `FOFX_ADDUNDORECORD`
- Residual Shell TOCTOU
- Required Milestone 14 keyboard/accessibility, state, query-instrumentation, Release-scale, and
  end-to-end cloud-safety gates

Missing evidence is `open` or `not_run`, never a pass. Milestone 11 remains incomplete while its
required gates are open.

## Latest verification baseline

Accepted `SOP2a` passes 7/7 focused progress-contract tests and the bounded-history Core library
test. The full Rust workspace passes 162 tests with 5 ignored and 0 failed. Strict Core library and
focused-test Clippy pass with warnings denied after only the three documented pre-existing library
allowances. Coverage includes version/golden serialization, atomic regression rejection, exact
logical-versus-physical units, bounded recent/run-cumulative rates, phase partitioning, cache
denominator, regular/delayed/unequal/sub-one-byte ETA, 64-device bounds, and unavailable/complete
states. No .NET, WPF, product/status migration, physical large-drive, profile, provider, mutation,
or release-validation campaign ran.

The SOP2 ledger slice is documentation-only. Three independent read-only audits agreed on the six
package boundaries and found the same live-counter, bucket-cadence, protocol-throttle, and stale-
lifecycle gaps. Link/checkpoint inspection and the final diff passed; no product tests were rerun
because no product code changed. The worktree started clean at `0a3c1c1`.

Accepted `SOP1f` passes 13/13 focused telemetry tests and the full Rust workspace with 154 passed, 5
ignored, and 0 failed, including the accelerated no-progress heartbeat fixture and batched/unchanged
counter-write contract. Strict Core Clippy passes with warnings denied after the three documented
unchanged-file allowances; worker Clippy passes after additionally allowing its five pre-existing
diagnostics. The separately invoked optimized Release profile passed the unchanged 100/100-basis-
point assertion at -210 wall/-2,276 CPU basis points. The negative deltas are not attributed as a
speedup. The earlier +468/+198 failure remains retained. No .NET, WPF, physical large-drive,
provider, mutation, or release-validation campaign ran. `SOP1` is accepted.

`SOP1e-host-device-sampler` passed 12/12 Core library tests, including deterministic fake cadence,
missed-interval, maximum-cardinality contracts and a real read-only Windows volume probe on the
designated machine. The Windows probe mapped the test volume to a `physical:*` key, returned
capacity/free-space and process CPU/working-set gauges, and retained no model/serial value. Strict
Core library Clippy passes with warnings denied after the three documented pre-existing allowances.
No worker protocol, WPF, .NET, active scan, external process, provider, or performance campaign ran.

`SOP1d-bounded-queries-retention` passed 12/12 telemetry tests and 12/12 end-to-end scan tests; the
full Rust workspace test graph compiles. Coverage includes stable cursor order, page-limit rejection,
fixed summaries, atomic 64-device rejection, default retention constants, sample/run trimming,
active-run preservation, exact repeat after reopen, terminal-history deletion, WAL policy, and
product-database isolation. Strict Core library Clippy passes with warnings denied after the three
documented pre-existing allowances. The storage contract is documented in
`docs/scan-status-database.md`; no worker protocol, WPF, .NET, physical, or provider campaign changed.

`SOP1c-run-lifecycle` passed the full Rust workspace: 146 passed, 4 ignored, 0 failed across Core,
worker, FFI, and CLI targets (including 12 end-to-end scan tests, 9 telemetry tests, and 17 worker
tests). Completed/cancelled/failed fixtures prove matching terminal status; the completed fixture
also proves ten bounded phase flushes and reconciled candidate/hash/cache/duplicate counters.
Core/worker library Clippy passed with warnings denied after allowing only documented pre-existing
lint classes. No .NET, WPF, physical-drive, provider, or performance campaign was required.

`SOP1b-status-store` passed 9/9 focused telemetry tests and strict Clippy with only the three
documented pre-existing unchanged-file lint classes allowed. Coverage includes exact start/flush/
terminal replay, conflicting replay, atomic counter/device/sample writes, explicit null gauges,
counter and phase regression, sequence/timestamp guards, schema-v1 migration plus injected rollback,
restart interruption reconciliation, and product-database isolation. No scan lifecycle, worker,
protocol, WPF, .NET, physical, provider, or performance campaign changed.

`SOP1a-contract-schema` passed 4/4 focused telemetry tests and 9/9 Core library tests. Strict Clippy
for the telemetry target passed after allowing only `needless_return`, `let_and_return`, and
`needless_question_mark`, which are the documented pre-existing warnings in unchanged scanner and
storage files. The initial strict run retained those five unchanged-file diagnostics. No .NET,
worker, product-database migration, scan, WPF, physical, provider, or performance campaign was
required for this contract-only package.

The preceding planning slice was documentation-only. Source inspection confirms exact-size grouping
at `scanner/walk.rs` and the unconditional singleton partial-hash pass plus ambiguous bucket-level
progress in `hasher/xxhash.rs`. The D: run was observed read-only; no process, product/runtime
database, cache, file, configuration, drive, or active-run state was changed. Verification was
limited to documentation links, gate consistency, production-lock preservation, Markdown/diff
hygiene, and a clean committed worktree.

The operator-directed follow-up changes only the retained WPM8 screen-reader disposition and
authoritative documentation: Narrator is `skipped_by_operator`, NVDA is `not_run`, the combined gate
remains blocked, and `WPM8-high-contrast` is the next independent authorized gate. No product code,
assistive-technology run, theme change, DPI work, provider/mutation/outcome/performance campaign, or
later gate occurred.

The blocked WPM8 physical-reader bundle
`artifacts/windows-narrator-nvda/20260824-171812-501` retains `manifest.json`,
`operator-observations.md`, the isolated database copy, and hash-cache directory. Narrator startup
succeeded, but both the initial and one fresh-window activation attempt failed with
`failed to activate captured window` before any workflow key. Therefore there are no valid
Narrator/NVDA observations, no product result, and no acceptance. Only proportional documentation,
production-lock, and diff checks followed; every excluded campaign remained skipped.

The accepted WPM8 bundle `artifacts/windows-representative-query/20260824-162843-832` retains
`acceptance-evidence.json`, `acceptance-report.md`, the Release build and 31-test deterministic
contract logs/TRX, all 500 query intervals, 101 process snapshots, and 12 valid host-context samples
with none invalid or unavailable. The exact Release Rust profile passed in 13.45 seconds with p95
values of 62.57/32.03/32.07/17.00/4.69 ms for group/summary, selected-root facet, drive facet,
review plan, and review groups; private growth was 929,792 bytes. Production remained disabled.
Only the proportional production-lock and documentation/diff checks followed; all full, provider,
physical-accessibility, mutation, outcome, broad-performance, and later-gate campaigns were skipped.

The WPM8 attempt retained `acceptance-evidence.json`, `acceptance-report.md`, and
`infrastructure-build.log` at `artifacts/windows-representative-query/20260824-161340-948`. The
Release Infrastructure build exited 1 after 874.08 ms because Windows SDK discovery could not read
`C:\Users\gary\AppData\Local\Microsoft SDKs`. The collector recorded `productionEnabled:false` and
`milestone11Complete:false`; it did not reach deterministic contracts or the profile and therefore
retained 0/500 intervals, 0/101 process snapshots, no host JSONL, and no p95 metrics. All full,
provider, physical-accessibility, mutation, outcome, broad-performance, and later-gate campaigns
were deliberately skipped.

The action-navigation slice passed `Verify-WindowsActionNavigation.ps1`; its final passing bundle is
`artifacts/windows-action-navigation/20260824-185741-521`, while failed zero-match verifier evidence
remains at `artifacts/windows-action-navigation/20260824-183642-262`. Focused evidence passed 1 worker protocol
test, 12 Release Core navigation/Shell tests, and 1 loaded-STA automation/dispatcher/focus test. Full
Debug/Release Rust passed 51 storage tests with 4 explicit performance ignores, 32 FFI tests, and
16 worker tests. Each serialized Debug/Release solution passed 113 Core, 69 Infrastructure, and 3
loaded-STA tests with 5 provider/physical Shell tests skipped. Real Debug/Release non-mutating smoke
passed stable hash-warning navigation, Alt+O, exact completed-run targeting, group-grid focus,
unchanged warning history/bounds, and all prior behavior; disposable fixtures ended in
`a083f70cdab544e1810bc93e4fb54af3` (Debug) and `421cae07aaa7492988d9988feedcd1f1`
(Release).

The latest event-coalescing slice was verified as follows:

- `Verify-WindowsEventCoalescing.ps1` passed 1 focused bounded-hint storage/history test, 1 worker
  hint/overflow/restart protocol test, 3 deterministic Infrastructure burst/rate/fallback tests, 6
  Core coalescing/overflow/cancellation/stale-context tests, and 1 loaded-STA dispatcher/automation/
  focus test, plus XAML and PowerShell parsing, diff hygiene, and every production lock;
- the full Debug Rust workspace passed 85 Core/scan/storage tests, 32 FFI tests, and 15 worker
  tests; the same 3 explicit operator performance profiles remained ignored; optimized Release
  passed 1 live-hint/history test, 2 dirty-root/migration tests, and 1 worker protocol test;
- each full Debug/Release solution matrix passed 107 Core, 69 Infrastructure, and 3 loaded-STA tests,
  with the same 5 explicitly gated provider/physical Shell tests skipped in each;
- real Debug and Release non-mutating worker/WPF smoke passed a deterministic 1,000-event worker
  aggregate, a real disposable non-result-file watcher burst with unchanged scan fixtures, bounded
  visible pending status, durable overflow/restart/reconciliation, external validation, immutable history,
  dispatcher/focus behavior, and unchanged production locks;
- targeted Rust/.NET formatting, PowerShell/XAML parsing, `git diff --check`, and the production-lock
  audit passed;
- provider, physical-accessibility, Recycle Bin/Shell-mutation, broad performance, Activity, in-app
  outcomes, and later-milestone campaigns were deliberately skipped.

Use proportional verification for the next slice. Run focused tests while iterating, then the
relevant full matrix before commit when shared Core/WPF/Infrastructure behavior changes. Run Rust
tests only when Rust behavior changes. Do not run physical or performance campaigns unless the slice
and prerequisites genuinely call for them.

## Completion loop

For each session:

1. Audit current state and preserve unrelated work.
2. Confirm the scheduled stream and select one ready named gate or coherent gate group from its
   authoritative plan.
3. Confirm its dependencies, evidence for edits, completion check, and explicit non-goals.
4. Implement only that gate. If a prerequisite is missing or the gate is locally exhausted, update
   its authoritative plan and stop without manufacturing a code slice.
5. Add regression coverage required by the named criterion; do not enumerate unrelated edge cases.
6. Update the selected stream's gate state and relevant authoritative documentation without
   overstating acceptance. Update the parked ledger only when its scheduling or its own gates change.
7. Run proportional verification and `git diff --check`.
8. Review the final diff against every boundary above.
9. Commit one focused commit.
10. Update this handoff's checkpoint, immediate next gate, verification baseline, and decision log
    as part of that commit.
11. Confirm `HEAD` contains every completed in-scope change from the session and the worktree is
    clean. A session with completed changes is not finished until its commit succeeds.
12. Report the commit, gate disposition, verification, skipped gates, blocker or next ready gate,
    and any user decision required.

## Handoff decision log

| Date | Commit | Completed slice | Next boundary |
|---|---|---|---|
| 2026-08-25 | this session | Accept `SOP2a-progress-contract-reducer` with versioned supplemental logical-work truth, bounded physical rate windows, same-unit stable ETA, atomic transitions, truthful cache/device/unavailable states, and no durable-schema or UI integration. | Implement `SOP2b-incremental-pipeline-publication` with gated mid-bucket/read evidence and exact terminal/cancellation/failure reconciliation. |
| 2026-08-25 | this session | Define the six-package `SOP2-progress-reporting` ledger from a cross-layer audit: typed cumulative truth, incremental bucket-independent publication, deterministic worker projection/coalescing, defensive Core application, accessible WPF surface, and integrated acceptance. | Implement `SOP2a-progress-contract-reducer`; publish exact meanings and fake-clock projection constants before touching the pipeline or UI. |
| 2026-08-25 | this session | Accept `SOP1f-foundation-acceptance` and `SOP1` after replacing 43 per-counter reads/upserts per flush with one read/atomic multi-row upsert, preserving exact replay and fixed summaries, and retaining the single post-change passing profile beside the original failure. Audit and adopt the operator's repeat-run cache proposal into `SOP8` planning without UI implementation. | Define the finite `SOP2-progress-reporting` package ledger before code; keep SOP5/SOP8/UI and the parked release stream behind their dependencies. |
| 2026-08-25 | this session | Integrate one serialized five-second heartbeat/status writer and prove sampling during a phase with no progress callbacks; retain the first SOP1f Release profile as failed at +4.68% wall/+1.98% CPU on a 1.25-second baseline. `SOP1f` remains in progress. | Do not retry unchanged. Reduce a concrete measured cost or obtain explicit review of an absolute/representative-duration observer budget before accepting `SOP1`. |
| 2026-08-25 | this session | Accept `SOP1e-host-device-sampler` with deterministic cadence/loss/cardinality contracts and read-only Windows process/system/volume/physical-disk probes that omit serials and preserve unavailable gauges. | Advance `SOP1f-foundation-acceptance`; integrate through the single writer and measure observer overhead before accepting `SOP1`. |
| 2026-08-25 | this session | Accept `SOP1d-bounded-queries-retention` with fixed run/phase/counter/device summaries, bounded cursors, a 64-device ceiling, default 50-run/100,000-sample retention, passive WAL checkpoints, and isolated terminal-history deletion. | Advance `SOP1e-host-device-sampler`; protocol and WPF exposure remain out of scope until the storage/sampler foundation accepts. |
| 2026-08-25 | this session | Accept `SOP1c-run-lifecycle` with metrics-contract v2, configurable worker status path, platform-neutral candidate/hash/cache counters, ten bounded phase flushes, and matching completed/cancelled/failed terminal state. | Advance `SOP1d-bounded-queries-retention`; status remains best-effort observability, separate from product truth, with no per-file rows. |
| 2026-08-25 | this session | Accept `SOP1b-status-store` with schema-v2 replay ledger, atomic monotonic run/phase/counter/device/sample writes, exact replay/conflict handling, explicit unavailable gauges, transactional migration, and startup interruption reconciliation. | Advance `SOP1c-run-lifecycle`; status storage is not product truth and no per-file telemetry rows are allowed. |
| 2026-08-25 | this session | Accept `SOP1a-contract-schema` with metrics-contract v1 Rust types/invariants and a separate schema-v1 status database that creates/reopens idempotently and rejects unknown/newer state without modification. | Advance `SOP1b-status-store`; do not integrate scan lifecycle writes until atomic replay/monotonic/recovery store semantics accept. |
| 2026-08-25 | this session | Make the active scan plan idempotent with a finite SOP1 package ledger, multi-package session progress, audit-once/two-failure anti-spin rules, context-risk handoff requirements, and a reusable state-independent kickoff prompt. | Begin `SOP1a-contract-schema`, accept it only with schema/metric invariant tests, then continue dependency-ready SOP1 packages. |
| 2026-08-25 | this session | Create the trackable large-drive scan optimization/observability stream, confirm singleton size buckets are still partially read, specify cumulative status metrics/progress/warning/Performance-tab gates, and park the preserved Windows release checklist. | Advance only `SOP1-telemetry-foundation`; resume release validation at `WPM8-high-contrast` before final feature-complete. |
| 2026-08-22 | `ec8da88` | Announce only committed Recycle Bin result pages; cover backward repeatability and stale/cancelled silence. | Re-audit for the next locally implementable Milestone 11 read-only/recovery gap; do not cross an evidence or recovery-design gate. |
| 2026-08-22 | `96a49af` | Preserve the committed Recycle Bin recovery page and cursor history across a failed forward fetch; allow exact retry. | Re-audit for another local read-only contract gap; stop at physical/provider/performance or recovery-resolution gates. |
| 2026-08-22 | `a476af3` | Cover failed backward recovery paging after the bounded cache evicts an older page; preserve the committed page and exact retry. | Re-audit for another local read-only contract gap; stop at physical/provider/performance or recovery-resolution gates. |
| 2026-08-22 | `a022588` | Clear stale Recycle Bin UI Automation announcement text when the selected run context changes without publishing an empty notification. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `35ee281` | Separate assertive Recycle Bin operation/page failures from polite committed-result notifications. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `45f6a09` | Exercise the separate Recycle Bin success/error channels through loaded WPF automation peers. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `d971119` | Announce the exact first committed unknown-result range during Recycle Bin reconstruction. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `306972e` | Retry the exact failed read-only Recycle Bin detail request, including the initial recovery page, without retrying the operation. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `d0b0362` | Commit the matching forward or backward cursor-history transition after a successful exact Recycle Bin detail retry. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `bd7efa6` | Retry a failed read-only Recycle Bin operation-summary reconstruction request for the exact selected completed run without allowing a stale retry to replace newer context. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `bd9e983` | Clear the resolved assertive Recycle Bin page-error payload after a successful exact retry without publishing another error notification. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `14649ea` | Prove a late failed exact page retry cannot replace newer run context, publish stale feedback, expose another retry, or mutate cursor history. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `37894b9` | Prove a late failed exact operation-summary retry cannot replace newer operation/page context, publish stale feedback, expose another retry, or change navigation. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `ce54d1a` | Announce a successful read-only operation reconstruction that finds no stored operation intent, without exposing retry or changing the execution boundary. | Re-audit for another local read-only contract gap; stop at physical accessibility, provider, performance, or recovery-resolution gates. |
| 2026-08-22 | `c34eebb` | Cover an exact read-only operation-summary retry that resolves an earlier load failure to no stored operation intent. | Replace further open-ended gap discovery with a finite roadmap closure audit. |
| 2026-08-23 | `8ff0b60` | Add the closure-ledger scaffold and tighten roadmap execution, edit authority, anti-spin, and stop rules. | Populate and accept every remaining Milestone 7-14 gate without product-code changes, then authorize only the first dependency-ready gate. |
| 2026-08-23 | `775417a` | Populate and accept the Milestone 7-14 closure inventory, reconcile statuses, and expose the critical path and independent lanes. | Review WPM7-opt-in-policy-scope + WPM14-required-scope + WPM14-completion-contract; do not substitute another ready gate. |
| 2026-08-23 | `d10ce7d` | Accept the required/deferred Milestone 7/14 scope and operator-accepted plus production-enabled completion contract without authorizing Recycle Bin production wiring. | Advance only WPM11-recovery-workflow to an explicit product/safety decision boundary; do not substitute another ready gate. |
| 2026-08-23 | `a2f3b6c` | Prepare the complete WPM11 ambiguous-recovery decision package without selecting or implementing a model. | Keep WPM11-recovery-workflow as the only authorized gate until the user chooses Option A, Option B with the explicit per-item waiver, or a named revision. |
| 2026-08-23 | `6d0182a` | Accept WPM11 recovery Option A and create its exact persistence/protocol, accessible WPF, and controlled process-loss dependency chain without product-code changes. | Advance only WPM11-recovery-review-persistence; keep WPF, campaigns, live inference, replay, and production wiring out of scope. |
| 2026-08-23 | `98d5558` | Implement and accept WPM11 recovery-review persistence/protocol with schema-v11 append-only observations, derived state, supersession, bounded paging, restart reconstruction, and matching non-UI client contracts while preserving every production lock. | Advance only WPM11-recovery-review-ui; keep automatic inspection/inference, replay, campaigns, Milestone 12 mutation, and production wiring out of scope. |
| 2026-08-23 | `d7b62d7` | Implement and accept the bounded accessible WPM11 recovery-review UI with exact safe retries, explicit append-only correction, approved copy/navigation, focus/automation/announcements, and every production lock preserved. | Advance only WPM11-ambiguous-start; do not substitute another evidence, performance, provider, mutation, or production gate. |
| 2026-08-23 | `16f6996` | Run and accept WPM11 ambiguous-start with disposable durable-start process loss, restart reconstruction, real WPF Option A observations/supersession, exact immutable-evidence verification, and retained passing/failing bundles. | Advance only WPM9-folder-relationships; preserve all execution locks and do not substitute another campaign or later milestone. |
| 2026-08-23 | `29b4256` | Implement and accept bounded side-by-side WPM9 folder relationship cards from immutable paged data with common/differing path context, per-copy/recoverable metrics, stable automation, keyboard/focus behavior, and unchanged physical de-duplication. | Advance only WPM9-explorer-responsiveness; keep parent grouping, thumbnails, review mutation, deletion, later milestones, and production wiring separate. |
| 2026-08-23 | `e578fd3` | Implement and accept responsive single-folder Explorer reveal over one immutable-page member with background native work, bounded actionable state, stale-context rejection, stable automation, Alt+E/double-click access, focus restoration, and real Debug/Release success/failure smoke. | Advance only WPM9-parent-grouping; keep open-all spawning, thumbnails, review mutation, deletion, later milestones, and production wiring separate. |
| 2026-08-23 | `5f79de8` | Implement and accept bounded current-page parent-grouped Explorer selection with deterministic one-call-per-parent background work, aggregate success/actionable partial failure, cancellation/stale-page rejection, Alt+G, stable automation, focus restoration, and real Debug/Release smoke. | Advance only WPM12-external-invalidation; exclude watchers, overflow/coalescing, Activity, deletion, and production wiring. |
| 2026-08-23 | this session | Implement and accept the schema-v12 bounded selection/visible-page external validation overlay with deletion/modification invalidation, sticky prior intent, exclusion-before-access, immutable history, restart, cancellation/stale-context rejection, stable keyboard/automation/focus state, and real Debug/Release smoke. | Advance only WPM12-watcher-overflow; exclude event coalescing, Activity, mutation, provider/physical/performance campaigns, later gates, and production wiring. |
| 2026-08-23 | this session | Implement and accept schema-v13 durable watcher-overflow dirty roots with bounded server-owned reconciliation, visible no-silent-trust WPF state, restart/cancellation/stale-generation protection, immutable-history preservation, one response-level UI update, keyboard/automation/focus support, and real Debug/Release smoke. | Advance only WPM12-event-coalescing; exclude Activity, deletion outcomes/mutation, provider/physical/performance campaigns, later gates, and production wiring. |
| 2026-08-24 | this session | Implement and accept one global 100 ms/200-path watcher-event coalescer with deterministic at-most-ten-UI-updates-per-second bounds, one read-only worker event/cache/binding/dispatcher update per batch, durable overflow fallback, stale-run rejection, accessible pending state, and real Debug/Release non-mutating burst smoke. | Advance only WPM13-warning-drilldown; preserve every accepted live-state/history/production boundary and exclude later Activity categories, mutation, and external campaigns. |
| 2026-08-24 | this session | Implement and accept schema-v14 bounded run-warning aggregates/examples, opaque server paging, fixed Core cache, restart reconstruction, immutable terminal history, cancellation/stale rejection, and accessible Run-history drilldown/focus. | Advance only WPM13-bounded-memory; exclude new Activity categories, navigation, outcomes, mutation, and external campaigns. |
| 2026-08-24 | this session | Implement and accept indexed stable warning sorting/keyset paging, sort-bound cursors, one retained 100,000-aggregate Release proof, fixed five-page Core caching, 25-row virtualized WPF binding, cancellation/stale rejection, dispatcher responsiveness, and keyboard/focus restoration. | Advance only WPM13-action-navigation; preserve schema-v14 accounting/examples, immutable history, every production lock, and all excluded campaigns. |
| 2026-08-24 | this session | Implement and accept the single existing completed-run hash-warning action with stable run-ID resolution, exact immutable duplicate-set navigation, cancellation/stale-context rejection, actionable missing-target feedback, Alt+O/automation, and focus restoration without changing bounded warning history or production locks. | Advance only WPM8-representative-query-performance on the designated representative Windows 11 x64 machine; otherwise preserve that blocker and stop. |
| 2026-08-24 | this session | Retain the single designated-host WPM8 query attempt at `artifacts/windows-representative-query/20260824-161340-948`; the Release prerequisite build failed on sandbox-denied Windows SDK access before any query measurement. | Keep WPM8-representative-query-performance blocked; separately authorize one unchanged new-directory collector invocation from a desktop context with installed-Windows-SDK access after reconfirming normal load. Do not substitute another gate. |
| 2026-08-24 | this session | Accept the designated-host WPM8 Release profile at `artifacts/windows-representative-query/20260824-162843-832` with all 500 intervals, 101 process snapshots, 12 valid host samples, 929,792 bytes private growth, and five p95 values below the unchanged 100 ms target. | Record WPM8-representative-query-performance `locally_exhausted`; preserve both bundles and every production lock. Advance only WPM8-narrator-nvda, contingent on its physical operator prerequisites. |
| 2026-08-24 | this session | Retain the blocked WPM8 physical-reader bootstrap at `artifacts/windows-narrator-nvda/20260824-171812-501`; Narrator started, but both app-activation attempts failed before any workflow key, so no valid listening pass or product observation occurred and NVDA was not started. | Keep WPM8-narrator-nvda blocked. Require separate authorization for exactly one new-directory manual-foreground bootstrap; do not substitute another gate. |
| 2026-08-24 | this session | Record the operator instruction to mark Narrator skipped; retain `skipped_by_operator` for Narrator and `not_run` for NVDA without accepting or waiving WPM8-narrator-nvda. | Advance only WPM8-high-contrast on its physical operator prerequisites; preserve the screen-reader blocker and stop before multi-monitor DPI. |
