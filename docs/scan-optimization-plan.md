# Large-Drive Scan Optimization and Observability Plan

## Status

Active implementation plan. The current pipeline audit and all six telemetry-foundation packages
are complete. This is the scheduled roadmap stream while the Windows post-MVP release-validation
checklist is parked. Before the product is declared feature complete, the release-validation stream
must resume at `WPM8-high-contrast` and follow its closure ledger to completion.

Current execution checkpoint:

- current gate: `SOP7-hash-read-path`;
- next boundary: advance only to `SOP7e-partial-prefix-reuse`;
- last accepted work package: `SOP7d-buffer-read-ahead`;
- prior D: stress run: stopped by the operator after the read-only baseline observation;
- canonical new-session prompt: [`scan-optimization-kickoff-prompt.md`](scan-optimization-kickoff-prompt.md).

This plan exists because representative use includes several roughly 10 TB drives. A full baseline
can take days, so optimization work must be driven by durable measurements, preserve exact-duplicate
correctness, and avoid requiring repeated uninstrumented whole-drive runs.

## Resumable execution protocol

This document is both the plan and the idempotent checkpoint. A session resumes from the first
`ready` work package in dependency order; it does not reconstruct or restart completed work from
conversation history.

### Progress policy

- Complete as many dependency-ready work packages as can be implemented and verified coherently in
  the session. A session is not limited to one gate or one commit.
- Keep each commit bounded to one work package or one inseparable coherent package group. After a
  commit, advance immediately to the next ready package when context and prerequisites remain sound.
- Pause only for a real external/user-decision blocker, a safety/authority boundary, failed required
  verification that cannot be repaired locally, or approaching context degradation that risks an
  incomplete audit, unsafe edit, or unreliable handoff.
- Before pausing for context risk, finish or revert the current coherent edit, run proportional
  verification, update this checkpoint and the session handoff, commit completed work, and leave a
  clean worktree. Context compaction alone is not a reason to restart completed audits.
- Prefer small deterministic fixtures and focused tests while iterating. Run the relevant full
  matrix at a gate acceptance boundary or when shared behavior changes enough to justify it.

### Idempotence and anti-spin rules

- At startup, compare `HEAD`, the worktree, this checkpoint, and cited evidence. If they agree,
  continue at the named next package. Never redo an accepted package merely because a new agent is
  unfamiliar with it.
- A package may move forward only as `open -> ready -> in_progress -> accepted` or to a documented
  `blocked` state. Record the accepting commit and verification in its ledger row before choosing
  more work.
- Audit a package once. Reopen it only for a reproduced regression, a failed acceptance check, a
  dependency contract change, or an explicit reviewed scope change. Do not generate progressively
  narrower findings after its completion criteria pass.
- After two attempts fail for the same reason without new evidence, record the blocker and continue
  with another dependency-ready package in this active stream if one exists. Do not substitute work
  from the parked release-validation stream.
- Schema migrations, command replays, sampling flushes, recovery, retention, and queries must be
  safe to repeat after process interruption. Exact replay returns the existing outcome; partial
  durable state is reconciled or rejected explicitly rather than duplicated.
- When code and checkpoint disagree, code and Git evidence win. Repair the checkpoint before new
  implementation; do not silently assume either side completed.

### Package definition requirement

Before implementation enters a gate, split it into the smallest finite set of dependency-ordered
packages that together satisfy the gate. Each package needs one bounded outcome and objective
completion check. Do not keep adding packages after all published gate criteria pass; additional
ideas return to the roadmap as separately reviewed follow-ons.

## Current evidence and algorithm audit

The 2026-08-25 read-only observation of an uninterrupted D: stress run is a diagnostic baseline, not
an accepted benchmark or a terminal run result:

- D: exposes about 10.63 TB usable space on a Seagate ST14000NM0018 SATA disk.
- Discovery reported 1,333,105 files in roughly four to five minutes.
- At about 16.5 hours, the UI reported 1,027,844 files hashed and was still advancing slowly.
- Point-in-time host sampling showed the physical disk busy with a queue near 8, roughly 21–25 MB/s,
  about 230–260 reads/s, and roughly 30 ms/read while the worker used little CPU. This is consistent
  with a seek/latency-bound rotational workload, not a CPU hash ceiling.
- Worker memory was stable during observation and the hash cache reported no write stalls or
  corruption. Two observed warnings were access-denied discovery warnings. No process, file,
  database, configuration, or active-run state was changed.

The source audit also found a concrete optimization gap:

- Discovery groups every first physical non-empty file by exact byte length in
  `scanner/walk.rs`. Exact size grouping is therefore already present and remains a correct first
  duplicate filter.
- `hasher/xxhash.rs` nevertheless sends every size bucket, including a bucket containing one file,
  through the 1 KiB partial-hash pass. Only the later full-content pass checks for collisions.
  Singleton-size files are therefore opened and partially read even though they cannot be duplicates.
- The existing `files_hashed` counter increments after a successful partial hash. It does not mean
  that the full contents of that many files were hashed, and progress is emitted only after a size
  bucket completes. The current label and update granularity cannot explain actual bytes read,
  collision work, cache benefit, or a long-running bucket.
- Size buckets and the files inside each bucket both use broad Rayon parallelism. On one rotational
  disk this can interleave unrelated paths, increase queue depth and seeks, and underuse sequential
  bandwidth. The live evidence shows physical I/O is currently the binding resource, but scheduling
  and unnecessary reads still provide meaningful software headroom.
- Repeat-run full hashes are already cached in a global RocksDB store across scans and process
  sessions when canonical path, byte size, and nanosecond modified time all match. There is no UI
  policy control. Every admitted file still repeats the 1 KiB partial read; files that never reached
  a full-hash collision have no reusable full hash; renames miss because path is part of the key;
  eviction is unbounded; and same-size content changes with a preserved timestamp are outside the
  current signature's invalidation power. `SOP8` must make this behavior explicit and measurable
  rather than treating file name/date alone as proof that content is unchanged.

## Goals and non-goals

Goals:

- make every phase, byte read, candidate reduction, warning, cache outcome, and device bottleneck
  observable during and after a run;
- remove I/O that cannot affect duplicate correctness;
- schedule remaining I/O according to physical-device characteristics and measured results;
- make progress accurate enough to explain long scans and support an honest bounded-confidence ETA;
- retain cumulative run and host/device performance history locally for comparison and debugging;
- validate changes on representative large rotational drives without discarding failed evidence.

Non-goals:

- changing the hash or exact-duplicate correctness contract merely to improve speed;
- reading excluded cloud placeholders, relaxing hard-link handling, or weakening cancellation;
- putting database, filesystem, or performance-counter work on the WPF dispatcher;
- materializing per-file telemetry or warning history in WPF;
- interrupting or mutating an active operator run to collect evidence;
- enabling production Recycle Bin execution or changing any release-validation safety lock.

## Metrics contract

`SOP1-telemetry-foundation` must define versioned meanings and units before any optimization is
accepted. At minimum, each run records cumulative phase totals and bounded time-series samples for:

### Candidate funnel

- files and logical bytes discovered;
- zero-byte files excluded and hard-link aliases de-duplicated;
- exact-size bucket count;
- singleton-size bucket/file/byte classification plus the subset actually resolved without content
  I/O;
- actual hash-pipeline candidate bucket/file/byte count and separate multi-file duplicate-candidate
  bucket/file/byte count;
- partial hashes attempted, succeeded, failed, and bytes read;
- partial-collision bucket/file/byte count;
- full-hash requests, completed files/bytes, failures, cache hits, cache misses, cache errors, and
  cache stores;
- confirmed duplicate sets, logical copies, physical items, and recoverable logical bytes.

### Time, resource, and device cost

- software/app/worker/schema/metrics versions plus an input-policy signature sufficient to compare
  like runs without persisting raw root paths in telemetry;
- monotonic start/end and active duration for discovery, candidate screening, full hashing, cache,
  persistence, directory analysis, and terminal finalization;
- current and cumulative files/s and MiB/s for partial and full reads;
- worker CPU time, private/working-set/peak memory, and process read/write operation and byte totals;
- system CPU, available/committed memory, and per-target physical-disk read throughput, IOPS,
  average latency, active time, and queue depth at a bounded sampling interval;
- logical-volume identity, filesystem, capacity/free bytes at run start, physical-device mapping,
  bus/media type, and non-secret model/friendly name when the platform exposes them. Hardware serial
  numbers are not persisted by default.

### Reliability

- warning totals by stable phase/category/code, first and last occurrence, and bounded examples;
- cancellation checks, cancelled work, inaccessible/changing files, cache fallbacks, and recovery
  state;
- telemetry sample loss, unavailable counters, status-database flush errors, and observer overhead.

Counters are monotonic for one run. Gauges such as queue depth remain timestamped samples. Every
displayed rate states its window; every cumulative value states its unit. Unknown or unavailable
values remain explicit rather than becoming zero.

## Local status database

The implemented schema, location, retention, WAL/checkpoint, and bounded-query contract is recorded
in [`scan-status-database.md`](scan-status-database.md).

Use a separate worker-owned local SQLite status database rather than adding high-rate samples to
the immutable product-results database. The implementation design must preserve these boundaries:

- the Rust worker is the only writer and owns schema migration, recovery, retention, and queries;
- a status run may reference the product run ID but has no cross-database foreign key and cannot
  become duplicate-result, review, preflight, or operation truth;
- durable run/phase/device summaries are small fixed-width rows; time-series samples are bounded and
  indexed by run, device, and monotonic sequence;
- no per-file performance row is stored, and raw paths never appear in performance samples;
- warning truth remains the existing product-owned structured warning aggregates; the status
  database stores only performance-oriented warning counters and health state;
- samples are buffered and committed in bounded batches no more frequently than necessary. The
  implementation gate must set and prove a retention policy, crash reconciliation, WAL/checkpoint
  policy, and an observer-overhead budget before acceptance;
- deleting status history cannot delete or reinterpret product results. Missing status history is
  shown as unavailable, not as a failed or empty scan.

## Progress, warnings, and Performance tab

### Progress semantics

Replace the ambiguous single `files hashed` concept with an explicit funnel:

1. discovered;
2. resolved by metadata/size without content I/O;
3. partial-screened;
4. selected for full hashing;
5. full-content hashed or satisfied from cache;
6. finalized into exact duplicate results.

Show file and byte totals, phase elapsed time, recent/cumulative throughput, cache hit rate, warning
count, active device, and remaining known candidate bytes. ETA is displayed only when the remaining
work is measurable and the rate window is stable; otherwise the UI explains why it is unavailable.
Worker events remain coalesced to at most ten UI updates per second and must not wait for a large
size bucket to finish before visible progress advances.

### Warning log

Keep the accepted completed-run warning aggregates and bounded paging. Add a current-run warning
entry point from progress/status that opens a bounded, virtualized log of structured phase/category/
code/count/examples, then carries the same run into completed history. Make diagnostic application
logs separately discoverable for developer/recovery detail. Do not bind every occurrence, expose
unbounded path lists, or conflate the diagnostic log with durable warning truth.

### Performance tab

Add a keyboard-accessible, virtualized Performance tab backed only by bounded worker queries over
the status database. It includes:

- current run health, phase duration, candidate funnel, cache effectiveness, throughput, CPU,
  memory, and warning summary;
- target volume and physical-drive information plus current/peak queue, latency, IOPS, throughput,
  and active time;
- completed-run phase and device summaries;
- comparison with selected prior runs on the same volume/device and software build, with differing
  inputs/builds called out rather than silently compared;
- bounded charts or tables only where they clarify a trend. No complete sample history is loaded
  into Core or WPF.

System brushes, native keyboard controls, focus restoration, high-contrast behavior, clear
unavailable-counter states, and coalesced UI Automation announcements are acceptance requirements.

## Trackable gates

| Gate ID | Disposition | State | Dependencies | Bounded outcome | Completion check |
|---|---|---|---|---|---|
| `SOP0-current-pipeline-audit` | `local_audit` | `accepted` | None | Record the current algorithm and read-only large-drive evidence without interrupting the run. | This document cites the singleton read, ambiguous progress semantics, nested parallelism, and provisional device evidence without claiming a terminal benchmark. |
| `SOP1-telemetry-foundation` | `local_code` | `accepted` | `SOP0-current-pipeline-audit` | Implement the versioned metrics contract, worker-owned status database, bounded sampler/retention, and fixed summary/time-series queries. | Migration/recovery/bounds tests pass; simulated counters reconcile; unavailable samples are explicit; instrumented fixture overhead stays below 1% wall time and 1% CPU or the retained evidence explains and reviews a stricter measured budget. |
| `SOP2-progress-reporting` | `local_code` | `accepted_with_operator_waiver` | `SOP1-telemetry-foundation` | Publish the candidate funnel, byte progress, rates, cache outcomes, and bounded-confidence ETA with bucket-independent coalescing. | SOP2a-SOP2e functional packages and the fixed-cost short leg pass. SOP2f's representative-duration leg is `waived_by_operator_unmeasured`: neither retained representative attempt produced an aggregate, so the strict <1% wall/CPU gate was not evaluated and must not be described as passed. The unresolved overhead risk is carried to SOP9. |
| `SOP3-current-warning-log` | `local_code` | `accepted` | `SOP1-telemetry-foundation` | Make active warnings visible through bounded structured paging while preserving completed-run aggregates and diagnostic logs. | Every current warning count is drillable or represented by a truthful bounded aggregate; restart/terminal handoff and cache bounds pass. |
| `SOP4-performance-tab` | `local_code` | `accepted` | `SOP1-telemetry-foundation`, `SOP2-progress-reporting`, `SOP3-current-warning-log` | Add the bounded live/history Performance tab with drive information and run comparison. | Core/WPF never bind full samples; keyboard, automation, unavailable-state, high-contrast, focus, restart, and representative-history tests pass. |
| `SOP5-skip-singleton-size-buckets` | `local_code` | `accepted` | `SOP1-telemetry-foundation`, `SOP2-progress-reporting` | Resolve exact-size singleton buckets without opening file content and make the saved I/O visible. | Correctness fixtures are unchanged; an injected read seam proves zero partial/full opens for singleton buckets; counters reconcile files/bytes as metadata-resolved rather than hashed. |
| `SOP6-device-aware-scheduler` | `local_code` | `accepted` | `SOP5-skip-singleton-size-buckets` | Replace nested global read parallelism with bounded per-physical-device queues: conservative concurrency on rotational media and measured concurrency on SSDs, while allowing separate devices to progress independently. | Deterministic scheduling/cancellation tests pass; retained 1/N-reader comparisons on representative devices show the selected policy and no correctness or memory regression. |
| `SOP7-hash-read-path` | `local_code` | `open` | `SOP6-device-aware-scheduler` | Benchmark path locality, bucket ordering, buffer/read-ahead size, and reuse of the partial prefix during full hashing. Admit only individually measured changes. | Each retained A/B run records bytes, IOPS, queue, latency, throughput, CPU, memory, and wall time; changes that do not improve the declared workload are rejected or scoped. |
| `SOP8-repeat-run-cache` | `local_code` | `open` | `SOP2-progress-reporting`, `SOP7-hash-read-path` | Turn the existing always-on canonical-path/size/time full-hash cache into an explicit repeat-scan policy. Evaluate a session UI choice between signature-qualified reuse and forced content revalidation; define stable identity, rename, hard-link, timestamp-resolution, partial/full-hash, cross-session, and bounded-eviction semantics without using name/date alone as correctness proof. | Warm same-session and cross-session fixtures prove the selected default and UI policy, exact invalidation/correctness, partial/full read accounting, rename/hard-link behavior, cache bounds, corruption fallback, and measured read/wall-time savings. |
| `SOP9-large-drive-acceptance` | `operator_evidence` | `open` | `SOP4-performance-tab` through `SOP8-repeat-run-cache` | Retain representative single- and multi-drive Release runs, including failures, select defaults from evidence, and observe the unresolved SOP2 representative-overhead risk during useful real-drive acceptance work rather than another standalone synthetic campaign. | Duplicate results match reference fixtures; singleton reads are zero; telemetry/warning accounting is complete; memory and UI bounds hold; before/after device and wall-time evidence is retained without retry-only acceptance. Any SOP2 observer-overhead observation is reported as measured at SOP9 scale or remains explicit residual risk; the waived strict <1% SOP2 gate is not retroactively declared passed. |

## Telemetry-foundation work-package ledger

`SOP1-telemetry-foundation` is complete only when every package below is accepted. Package boundaries
may share a commit only when separating them would leave an unusable or unverifiable intermediate
state.

| Package ID | State | Dependencies | Bounded outcome | Completion check | Evidence/commit |
|---|---|---|---|---|---|
| `SOP1a-contract-schema` | `accepted` | SOP0 | Add versioned Rust metric types, exact counter/gauge semantics, and the separate status-database schema/migration contract without scan integration. | Schema creation/reopen/newer-version rejection and metric invariant tests pass; no product database or WPF contract changes. | This session: 4 focused telemetry tests, 9 Core library tests, and strict Clippy with only the three documented pre-existing lint classes allowed. |
| `SOP1b-status-store` | `accepted` | SOP1a | Implement the worker-owned status store with atomic run/phase/device/sample writes, interruption reconciliation, and explicit unavailable values. | Focused store tests prove exact replay, monotonic counter rejection, crash-safe reopen, and no cross-database mutation. | This session: 9 focused telemetry tests and strict Clippy with only the three documented pre-existing lint classes allowed. |
| `SOP1c-run-lifecycle` | `accepted` | SOP1b | Connect scan lifecycle and platform-neutral candidate/hash/cache counters to the status store with bounded buffered flushes. | Deterministic scan fixtures reconcile terminal counters and interrupted/cancelled runs without per-file telemetry rows. | This commit: metrics contract v2 separates actual hash-pipeline work from duplicate-candidate classification; completed/cancelled/failed fixtures reconcile; one normal scan writes ten phase-boundary flushes; full Rust workspace and focused strict Clippy pass with documented pre-existing lint allowances. |
| `SOP1d-bounded-queries-retention` | `accepted` | SOP1b | Add fixed run/phase/device summaries, bounded sample paging, retention, checkpoint policy, and status-history deletion isolation. | Query/cursor/bounds tests and retention/reopen tests pass; product results remain unchanged when status history is absent or removed. | This commit: 12 telemetry tests and 12 end-to-end scan tests pass; the full workspace compiles; strict Core Clippy passes with only documented pre-existing allowances. Run pages are capped at 100, sample pages at 500, devices at 64, and repeatable retention/history deletion preserves active and product state. |
| `SOP1e-host-device-sampler` | `accepted` | SOP1b | Add a platform seam and Windows implementation for bounded process/system/volume/physical-device samples, with explicit unavailable-counter health. | Fake-clock/platform tests pass; Windows probes identify mapped target devices without serial persistence or WPF-thread work. | This commit: fake cadence/cardinality/loss tests and a real read-only Windows local-volume probe pass; the volume maps to a `physical:*` device, host gauges are available, serials are absent, and strict Core Clippy passes. |
| `SOP1f-foundation-acceptance` | `accepted` | SOP1c, SOP1d, SOP1e | Integrate the packages, document retention/schema/recovery, and retain observer-overhead evidence. | Focused and full relevant matrices pass; instrumentation adds less than 1% wall time and 1% CPU on the declared fixture, or a reviewed measured budget is recorded; gate `SOP1` becomes accepted. | The retained [first profile](evidence/scan-telemetry-overhead-20260825.json) failed at +4.68% wall/+1.98% CPU. Audit found 43 reads plus 43 upserts per flush; one read plus one atomic multi-row upsert now preserves all summaries/replay while skipping unchanged writes. The single post-change [comparable Release profile](evidence/scan-telemetry-overhead-20260825-counter-batching.json) passed at -2.10% wall/-22.76% CPU; negative values are treated as noise/no detected positive overhead, not acceleration. Focused 13-test telemetry, full 154-pass/5-ignore workspace, and strict Core/worker Clippy verification pass. |

## Progress-reporting work-package ledger

`SOP2-progress-reporting` is complete only when every package below is accepted. The Rust producer
owns cumulative truth, the worker derives bounded-window projections and coalesces transport, and
Core validates/projects the accepted snapshots without inventing a third counter source. Package
boundaries may share a commit only when separating them would leave an unusable or unverifiable
intermediate state. The versioned meanings, units, provisional windows, and unavailable states are
defined in [`scan-progress-contract-v1.md`](scan-progress-contract-v1.md).

| Package ID | State | Dependencies | Bounded outcome | Completion check | Evidence/commit |
|---|---|---|---|---|---|
| `SOP2a-progress-contract-reducer` | `accepted` | SOP1 | Define one versioned, platform-neutral cumulative live snapshot and pure reducer for the six-stage file/byte funnel, partial/full actual-I/O counters, cache outcomes, warning count, a bounded active-device state (one, multiple, or unavailable), stage-qualified remaining known bytes, window-labelled recent/cumulative rates, and ETA value or closed unavailable reason. Add missing logical-byte meanings instead of deriving them from physical reads or recoverable bytes; rates retain physical-I/O units, and unavailable remains distinct from zero. | Fake-clock and invariant tests prove monotonic updates, overflow/divide-by-zero handling, exact units, semantic separation, phase/window reset, cache denominator, stable versus warming/unstable/zero-rate/unknown-work ETA, and serialization compatibility. The contract publishes its exact rate-window, stability, and coalescing constants before integration. | Progress contract v1 remains separate from durable metrics v2 and defines ten supplemental logical counters (the SOP2b accounting review added general failed-full-hash files), bounded one/multiple/unavailable active-device state, 100 ms rate buckets, a 30-second/304-point physical-rate window, and ETA after two stable five-second logical-resolution intervals. Eight focused integration tests plus one bounded-history library test pass. |
| `SOP2b-incremental-pipeline-publication` | `accepted` | SOP2a | Populate the snapshot from the same observations that reconcile terminal telemetry, including discovery, partial/full reads, cache outcomes, warnings, cancellation checks, and failures. Publish bounded progress inside a large size bucket and long full-file read without writing per-file status rows or changing scan results. Keep the current singleton content read and `metadata-resolved = 0` baseline until SOP5; derive partial/full activity windows from timestamped deltas without claiming the currently unused full-hash phase or restructuring bucket scheduling. | A deterministic gated-read fixture advances before one large same-size bucket finishes; cache hit/miss/error and failed-read fixtures reconcile logical satisfied bytes against actual I/O; terminal live and durable counters agree; cancellation stops further reads/publication; cloud and hard-link behavior remain unchanged. | This commit: one serialized engine sink consumes cumulative hasher deltas batched at 256 file outcomes or 8 MiB, while the heartbeat/status writer retains durable ownership. Deterministic injected-I/O tests prove mid-bucket/mid-read publication, cache/read/failure accounting, and cancellation silence; completed/failed/cancelled engine tests compare every final live metrics-v2 counter with status storage. Full Rust verification passes 169 tests with 5 ignored; strict Core/focused Clippy passes with only documented pre-existing allowances. No status-schema, worker protocol, .NET/WPF, scheduling, singleton, cache-policy, profile, or parked-stream change. |
| `SOP2c-worker-progress-projection` | `accepted` | SOP2a, SOP2b | Replace the legacy worker progress state with the typed snapshot projection, fake-clock rate/ETA calculation, optional non-secret device mapping, and a latest-value coalescer. Keep protocol-v1 additive compatibility while deprecating ambiguous `filesHashed` from display use. Assign sequence after coalescing, make cancelling sticky, discard pending progress at terminal, and emit no post-terminal progress. | Deterministic phase-churn and 1,000-update tests prove strictly increasing sequences, latest-wins delivery, phase changes within the next legal slot, at least 100 ms between ordinary frames and no more than ten frames in every half-open one-second interval. Protocol documentation defines decimal byte fields, unavailable states, terminal ordering, and the exact bound. | This commit: the worker reduces typed observations and uses one timer-owned latest-value emitter with post-coalescing sequence numbers, sticky cancellation, pending discard/join before terminal events, and a hard 100 ms interval. Protocol v1 retains its legacy fields, deprecates partial-success `filesHashed`, preserves zero-byte discovery compatibility, and adds the complete typed snapshot with every byte/rate quantity encoded as a decimal string. Device state remains truthfully `mapping_unavailable`; no association is invented. A narrow SOP2b follow-up publishes typed discovery truth at the existing 256-file callback. Twenty-two focused worker tests, the discovery fixture, strict focused Clippy, and the full Rust workspace (175 passed, 5 ignored) pass. No Core/.NET/WPF, schema, profile, cache-policy, or parked-stream change. |
| `SOP2d-core-progress-projection` | `accepted` | SOP2c | Extend Infrastructure/Core contracts and add a defensive latest-only application gate so a burst or delayed dispatcher work cannot exceed the UI bound or revive stale state. Reject wrong-run, duplicate/out-of-order, counter-regressing, running-after-cancelling, and post-terminal progress. Project explicit units/window labels and ETA explanations. | JSON contract and fake-scheduler tests feed 1,000 frames and prove at most ten accepted Core/WPF updates per second, latest state preservation, lifecycle/disposal cancellation of pending work, terminal/cancelling stickiness, stale silence, and exact funnel/rate/cache/ETA formatting. | This commit: Infrastructure parses the complete typed snapshot with exact field kinds/casing, canonical decimal-u64 strings, closed tagged states, additive unknown-field compatibility, and Core semantic/invariant validation. One generation-scoped latest-only gate owns at most one delayed dispatcher closure, applies at most once per 100 ms, and invalidates pending work across cancellation, terminal, run reset, unexpected exit, and disposal. Core revalidates run/transport/source order, lifecycle stickiness, and cumulative non-regression before projecting the seven funnel stages plus explicit phase/rate/cache/device/remaining-work/ETA text. Deterministic 1,000-frame and delayed-dispatch tests pass; the full Windows solution passes 129 Core, 74 Infrastructure, and 3 loaded-STA smoke tests with 5 operator-only Infrastructure tests skipped. The paired worker builds successfully. No XAML/WPF surface, Rust behavior, schema, performance profile, cache policy, or parked-stream change. |
| `SOP2e-accessible-progress-surface` | `accepted` | SOP2d | Replace the ambiguous indeterminate `Files hashed` card with the six-stage file/byte funnel, phase elapsed, recent/cumulative partial/full throughput, cache effectiveness, warnings, bounded one/multiple/unavailable active-device state, remaining known bytes, and ETA or truthful unavailable explanation. Retain cancellation and use stable automation IDs, system brushes, keyboard reachability, and coalesced UI Automation announcements. | Core and loaded-STA WPF tests prove bindings, bounded collections, keyboard/focus behavior, system-brush/high-contrast compatibility, automation names, cancelling/terminal states, ETA-unavailable text, and announcements driven only by accepted coalesced snapshots. No warning drilldown, Performance tab, cache-policy control, or optimization-gate UI is added. | This commit: the progress view removes legacy `Files hashed` and renders exactly the six contract outcomes with file/logical-byte values; hash-pipeline candidates remain separate denominator context. A vertically scrolling, narrow-width wrapping surface exposes phase elapsed, four window-labelled physical-read rates, cache, warnings, bounded device text, remaining work, ETA/unavailable reasons, current activity, and cancellation with stable automation IDs and system brushes. Terminal lifecycle overrides prevent stale active-I/O, remaining-work, or ETA claims while preserving the last accepted funnel; selected runs without a live snapshot receive an explicit unavailable explanation. UI Automation announces only accepted snapshots: first/phase/status changes immediately, otherwise at most once per five seconds, using one monotonic cross-run version and `MostRecent` processing. Core rejection tests and the existing 1,000-frame Shell test prove silence outside the accepted application path. Loaded-STA tests prove six-item bounds, narrow wrapping, system-theme brushes, exact names/bindings, Alt+C focus behavior, and latest-only queued notification delivery. The full Windows solution passes 137 Core, 74 Infrastructure, and 3 loaded-STA smoke tests with 5 operator-only Infrastructure tests skipped. No warning drilldown, physical high-contrast campaign, Performance tab, cache policy, Rust/worker/schema/profile, or parked-stream change. |
| `SOP2f-progress-acceptance` | `waived_by_operator_unmeasured` | SOP2b, SOP2c, SOP2d, SOP2e | Integrate and accept the gate with one focused cross-layer verifier and proportionate full matrices. | Functional verification must pass. The retained fixed-cost short leg must pass its 100,000,000 ns wall and 125,000,000 ns worker-CPU caps. The operator waived the separate representative-duration leg unmeasured; its strict <100 bp wall/CPU condition remains unevaluated, not passed. | Functional acceptance and the retained short leg pass. V1 retained [pre-measurement failure](evidence/scan-progress-representative-premeasurement-20260825.json) before any arm. The sole authorized [`SOP2f-representative-v2`](scan-progress-representative-protocol-v2.md) attempt completed setup and conditioning, but control warmup 0 did not complete inside its bound and the empty-aggregate defect prevented native invalid-evidence serialization. The recovered write-once [invalid-campaign incident](evidence/scan-progress-representative-overhead-sop2f-v2.json) retains zero completed/measured arms, null aggregates, an unevaluated <100 bp wall/CPU gate, cleanup facts, and consumed authority. No v1/v2 rerun or SOP2f-v3 is authorized. SOP2 is `accepted_with_operator_waiver`; the unresolved representative-overhead risk is assigned to SOP9 real-drive acceptance. |

## Current-warning-log work-package ledger

`SOP3-current-warning-log` is complete only when every package below is accepted. Existing
schema-v14 completed-run aggregates, at-most-three examples, opaque paging, five-page Core cache,
25-row WPF page, and diagnostic application log remain the foundation; SOP3 adds active-run truth
without turning occurrences or paths into an unbounded second history.

| Package ID | State | Dependencies | Bounded outcome | Completion check | Evidence/commit |
|---|---|---|---|---|---|
| `SOP3a-live-warning-accounting` | `accepted` | SOP1, SOP2 | When an accepted live warning count advances before specific phase details are available, atomically persist the exact gap as one stable structured fallback aggregate with one diagnostic-log example. Replace that fallback with specific aggregates at phase completion; keep active, cancelling, interrupted, and terminal counts exactly accounted without per-occurrence rows. | Storage and worker tests prove monotonic exact accounting, one bounded fallback row, specific-aggregate replacement, warning-triggered persistence before publication, and restart/terminal preservation. | This commit: worker progress forces the atomic product-database update before admitting a higher warning frame and remains silent if accounting fails. One stable `active_unclassified_recoverable_warning` row carries only the exact unclassified gap and one diagnostic-log example; specific phase replacement remains authoritative. Full storage tests pass 52/52 with 4 retained operator profiles ignored; full worker library tests pass 24/24. |
| `SOP3b-active-warning-page-snapshot` | `accepted` | SOP3a | Make active warning paging mutation-safe with a server-owned snapshot/revision contract, stale-cursor rejection, explicit active/terminal state, and separate diagnostic-log location metadata. | Worker/API tests prove bounded pages cannot silently mix revisions, stale cursors fail closed, restart reconstructs the latest durable snapshot, and completed-run protocol compatibility remains additive. | This commit: [schema v15](storage-schema-v15.md) adds one durable per-run warning revision. Aggregate/lifecycle mutations advance it, each page reads rows/counts/state/revision from one SQLite snapshot, and cursors bind run/sort/revision/status. Active mutation and terminal handoff reject stale cursors; restart reconstructs the exact interrupted snapshot. The additive response reports active/terminal/pending state and client-configured bounded diagnostic-log location as supplemental—not durable warning—metadata. Full storage passes 53 with 4 retained profiles ignored; all 25 worker library tests and the focused typed WorkerClient lifecycle test pass. |
| `SOP3c-bounded-current-warning-view-model` | `accepted` | SOP3b | Add a reusable Core current/completed warning drilldown with generation-scoped cancellation, revision-bound five-page cache, 25-row binding, exact live/accounted status, and terminal handoff that cannot revive stale active pages. | Deterministic Core tests prove cache/page bounds, active refresh, cancellation/stale rejection, restart reconstruction, and one-way terminal handoff while preserving immutable completed history. | This commit: Core consumes the SOP3b revision/state/status and separate diagnostic-log metadata in one reusable drilldown. Active and pending first pages bypass cache, every cursor page remains bound to the accepted durable identity, five LRU pages and 25 visible rows are hard limits, generation changes cancel and reject late responses, exact accounted totals are projected explicitly, and terminal identity is a one-way immutable latch. Completed Run history delegates paging to the same component without a WPF change. Four focused deterministic tests plus the full Windows solution pass. |
| `SOP3d-accessible-warning-entry-and-acceptance` | `accepted` | SOP3c | Add the Progress warning entry point, reuse the bounded virtualized warning surface through terminal history, and make the separate diagnostic application log discoverable with system-brush, keyboard, focus, and coalesced announcement behavior. | Focused cross-layer and loaded-STA tests prove every displayed current count is accounted, accessible entry/focus/automation, diagnostic-log separation, terminal/restart handoff, cache/UI bounds, and no SOP4 Performance tab. | This commit: the Progress warnings card exposes one Alt+W entry with an exact count-aware automation name. Shell routes that run to the existing History drilldown, whose accepted 25-row/five-page bounds, revision validation, terminal latch, and warning-grid focus remain the only warning surface. A separate system-brush diagnostic application-log block binds only the page's supplemental metadata and states that it is not durable warning truth. `Verify-WindowsAccessibleWarningEntry.ps1` passes 7 focused Core tests plus 1 loaded-STA WPF test; the full Debug Windows solution passes 143 Core, 74 Infrastructure, and 3 smoke tests with the same 5 operator-only skips. No Rust, worker, schema, SOP4, campaign, or parked-stream work ran. |

## Performance-tab work-package ledger

`SOP4-performance-tab` is complete only when all three finite packages below are accepted. They are
one coherent protocol/Core/WPF group because no intermediate package alone exposes a usable surface.

| Package ID | State | Dependencies | Bounded outcome | Completion check | Evidence/commit |
|---|---|---|---|---|---|
| `SOP4a-fixed-performance-queries` | `accepted` | SOP1, SOP2, SOP3 | Add query-only status-run history and one-run current/peak summaries without a schema change or raw sample transfer. | Store and worker tests prove a 25-run page, fixed 43-counter/six-phase/one-host summaries, at most 64 device rows, explicit nulls, restart continuity, and `executorEnabled:false`. | This commit: `performance.run.page` and `performance.snapshot.get` use a query-only SQLite reader. SQL selects the newest gauge and computes retained peaks; the protocol returns no time-series arrays. Focused status and worker restart tests plus the typed real-worker client test pass. |
| `SOP4b-bounded-live-history-projection` | `accepted` | SOP4a | Project current health, funnel/cache/throughput/CPU/memory/warnings, phases, drive descriptors/current/peak gauges, newest retained runs, and a selected comparison in Core. | Deterministic tests prove 25/6/64 collection ceilings, live refresh from accepted progress/lifecycle, stale cancellation, truthful unavailable text, exact device/input/build comparison labels, restart reconstruction, and refusal of unsafe/unbounded responses. | This commit: Core never receives raw samples, refreshes at most once per 50 accepted transport sequences plus lifecycle/manual refresh, and compares two bounded snapshots. A fresh view model reconstructs the same retained comparison after restart. |
| `SOP4c-accessible-performance-acceptance` | `accepted` | SOP4b | Add the virtualized keyboard-accessible Performance tab and close SOP4 with representative bounded-history evidence. | Loaded-STA and full regression matrices prove native keyboard focus, stable automation, system brushes/high-contrast compatibility, latest-only announcements, explicit unavailable values, 25 history rows, 64 device rows, restart continuity, and unchanged production locks. | `Verify-WindowsPerformanceTab.ps1` passes focused Rust store/worker, Core, typed-client, and loaded-STA tests plus parsing, diff, and production-lock checks. Full Debug/Release Rust each pass 181 tests with 5 ignored profiles; full Debug/Release Windows each pass 145 Core, 74 Infrastructure, and 3 smoke tests with the same 5 operator-only skips. No representative/physical campaign, SOP5+, or parked release work ran. |

## Singleton-size-bucket work-package ledger

`SOP5-skip-singleton-size-buckets` is complete only when both finite packages below are accepted.
They form one coherent algorithm-and-acceptance group because a content-I/O short circuit without
its injected zero-open proof is not a reviewable intermediate state.

| Package ID | State | Dependencies | Bounded outcome | Completion check | Evidence/commit |
|---|---|---|---|---|---|
| `SOP5a-singleton-short-circuit-accounting` | `accepted` | SOP1, SOP2 | Resolve each exact-size singleton from discovery metadata before the partial/full hash read seam, admit only multi-file size buckets to the hash pipeline, and project the saved logical work through the existing metadata-resolved/candidate counters while physical read counters remain actual I/O. | Deterministic producer and end-to-end fixtures prove singleton files/bytes exactly equal metadata-resolved work, hash candidates exactly equal multi-file size-bucket work, terminal candidate/resolved and live/durable counters reconcile, and duplicate results, hard-link semantics, cancellation, errors, cache behavior, deterministic output, and bounded memory remain unchanged. | This commit: discovery classifies exact-size singletons as metadata-resolved before hashing, and the hash pipeline's legacy denominator plus typed candidate totals contain only multi-file size buckets. The end-to-end fixture reconciles 2 files/41 bytes as singleton and metadata-resolved, 4 files/8,228 bytes as candidates and hash-pipeline-resolved, and only 2,084 physical partial-read bytes; every live/durable terminal counter matches. |
| `SOP5b-zero-open-correctness-acceptance` | `accepted` | SOP5a | Close SOP5 with an injected read-seam fixture and proportionate focused plus full Rust/Windows regression evidence. | The seam observes zero singleton partial/full opens while non-singleton buckets retain their existing partial/full paths and duplicate results; Debug/Release Rust and Windows matrices pass, production locks remain disabled, and no campaign, SOP6+, or parked validation work runs. | The injected mixed-bucket seam observes 0 singleton opens, 4 non-singleton partial opens, 2 full-content opens, and the unchanged duplicate group. The full Core target passes 124 tests with 5 ignored; full Debug/Release Rust each pass 182 with 5 ignored. Debug/Release Windows builds report 0 warnings/errors and each test matrix passes 145 Core, 74 Infrastructure, and 3 smoke tests with the same 5 operator-only skips. Static lock checks and `git diff --check` pass. No campaign, SOP6+, or parked validation ran. |

## Device-aware scheduler work-package ledger

`SOP6-device-aware-scheduler` is complete only when all three finite packages below are accepted.
The selected reader counts must come from the retained SOP6 comparison rather than from a later
read-path experiment. Additional locality, buffering, read-ahead, or prefix-reuse ideas remain SOP7.

| Package ID | State | Dependencies | Bounded outcome | Completion check | Evidence/commit |
|---|---|---|---|---|---|
| `SOP6a-device-policy-contract` | `accepted` | SOP5 | Add a platform-neutral device scheduling contract and a Windows non-content-opening mapping from a candidate path to one exact physical disk plus seek-penalty media class. Use one conservative shared fallback queue when mapping is unavailable, ambiguous, remote, or spans devices. | Deterministic fake-mapper tests prove stable grouping, one-reader rotational/unknown fallback policy, bounded solid-state policy, same-device serialization at its ceiling, and independent progress across distinct devices. Windows mapping tests cover drive aliases, one physical extent, ambiguous extents, unavailable queries, and seek-penalty classification without opening candidate content. | Windows maps a drive volume through its single physical extent and the documented seek-penalty property; drive aliases share the cached result. Remote, failed, multi-extent, or conflicting observations fall back conservatively. Fixed policy tests select one rotational/unknown reader, four SSD readers, a global logical-parallelism ceiling, same-device bounds, and independent-device progress. |
| `SOP6b-bounded-hash-scheduler` | `accepted` | SOP6a | Replace the nested global Rayon bucket/group/file reads with one bounded per-device scheduler for both partial and full reads while preserving the partial-before-full collision contract, cache behavior, progress batching, and duplicate correctness. | Injected scheduling/I/O tests prove no device exceeds its selected reader ceiling, separate devices overlap, queued work observes cancellation before content open, terminal counters reconcile, output is deterministic, and total scheduler queues/tasks remain bounded by already-admitted candidate work. | One fair global dispatcher now owns per-device queues and removes every nested Rayon read from hashing. Partial work completes before collision-derived full work; only admitted candidate paths are queued; cancellation clears pending work before the content seam and flushes exact progress. The injected two-HDD pipeline holds each device at one read while overlapping both and reconciles 8 partial attempts, 8 full requests/completions, 8 resolved files, and the exact duplicate result. |
| `SOP6c-reader-comparison-acceptance` | `accepted` | SOP6b | Retain one predeclared 1-reader versus selected-N comparison for each available representative rotational and solid-state device, choose or reject each media policy from all retained samples, and close SOP6 with proportionate full regression and production-lock evidence. | Evidence records fixture/signature, media/mapping proof, order, wall time, throughput, CPU, memory, reads/bytes, device IOPS/queue/latency/active time, cancellation, correctness, and every sample including failures. Debug/Release Rust and Windows matrices pass with no memory/correctness regression, production locks remain disabled, and SOP7+, unrelated physical campaigns, and parked validation do not run. | Write-once direct/unbuffered 1 GiB arms in order 1/N/N/1 retain identical checksums and exact process I/O. On the 14 TB rotational disk, two readers were 4.2593% slower with 4.0830% lower throughput and about double latency, selecting one. On the SSD, four readers cut wall time 48.9399% and raised throughput 85.4809%, selecting four. `Verify-WindowsDeviceScheduler.ps1`, strict Core Clippy with four documented pre-existing lint allowances, full Debug/Release Rust (192 passed, 6 intentional profiles ignored), and full Debug/Release Windows (145 Core, 74 Infrastructure, 3 smoke; 5 operator-only skips) pass with zero build warnings/errors and unchanged production locks. |

## Hash-read-path work-package ledger

`SOP7-hash-read-path` is complete only when all six finite packages below are accepted. Every
experiment changes one read-path factor at a time against the accepted SOP6 reader policy. Each
write-once A/B comparison uses fixed alternating order, retains every sample including failures,
and declares its workload before execution. A factor may be accepted as a product change, scoped
to a media/workload class, or rejected; it must not be rerun merely to obtain a favorable result.

| Package ID | State | Dependencies | Bounded outcome | Completion check | Evidence/commit |
|---|---|---|---|---|---|
| `SOP7a-read-experiment-contract` | `accepted` | SOP6 | Add platform-neutral read-strategy/order seams and one deterministic, write-once profiling harness that records fixture/input signature, software build, media/mapping proof, fixed arm order, wall/throughput/CPU/memory/process-I/O/device metrics, exact checksums, cancellation, and cleanup without changing production defaults. | Deterministic tests prove one-factor isolation, stable task/result identity, exact byte/checksum reconciliation, bounded fixture/task memory, write-once output, and fail-closed invalid evidence. The harness supports both representative rotational and solid-state roots but runs no full-drive campaign. | A test-only platform-neutral harness fixes control/treatment/treatment/control order, isolates all five factors, preserves SOP6's selected one/four/one device reader ceilings, uses four distinct deterministic arm fixtures, and records the required host/device/process/checksum/cleanup fields. Five focused tests pass with one physical profile ignored; strict Core Clippy passes with only the four documented pre-existing lint allowances. Production hash defaults and the accepted SOP6 source/evidence hashes are unchanged. |
| `SOP7b-path-locality` | `accepted` | SOP7a | Compare accepted scheduler insertion order with deterministic canonical parent/path locality inside each physical-device queue while keeping bucket membership, bucket order, reader counts, buffer, read-ahead, and prefix behavior fixed. | Retained alternating-order evidence selects, rejects, or media-scopes locality from all samples. Any admitted order preserves deterministic duplicate results, cancellation-before-open, bounded queues, and exact counters. | The product change is rejected. Fixed 1 GiB-per-arm C/T/T/C evidence retained every sample and identical checksums/bytes. The SSD cached workload regressed 11.5698% wall and 10.4500% throughput; explicit device unavailability prevents a physical-SSD claim. On the rotational direct-read workload, idealized parent/allocation locality improved wall only 1.2075% and throughput 1.2204%, with all device counters available. That small synthetic result does not justify changing buffered production order. No sample was discarded/rerun, both fixtures were removed, and no production source changed. |
| `SOP7c-bucket-ordering` | `accepted` | SOP7b | Compare the accepted ascending-size bucket order with one predeclared alternative while keeping within-bucket order, reader counts, buffer, read-ahead, and prefix behavior fixed. | Retained alternating-order evidence selects or rejects the alternative for its declared workload; any admitted order preserves deterministic results, progress reconciliation, cancellation, and bounded memory. | Descending exact-size order is selected for all media. Direct 1 GiB-per-arm C/T/T/C evidence retained identical checksums/bytes and complete device metrics. Treatment improved mean wall time 3.4622% and throughput 3.9257% on SSD, and improved wall 8.4299% and throughput 9.1264% on rotational media. A pre-evidence SSD cleanup-handle failure wrote no result; bounded cleanup was added and verified before the sole corrected run. One order regression, the SOP6 verifier, strict Clippy, and full Debug/Release Rust matrices pass (199 passed, 7 intentional profiles ignored). |
| `SOP7d-buffer-read-ahead` | `accepted` | SOP7c | Measure full-content cache-miss buffer sizes and the platform sequential-read hint as separate factors under the accepted scheduler/order policy; do not change partial-hash length or reader counts. | Retained evidence records every candidate arm and selects, rejects, or media-scopes each factor. Any admitted setting preserves exact hashes/bytes, mid-read progress, cancellation latency, error accounting, and bounded per-reader memory. | Direct/write-through fixture preparation produced complete cache-miss device evidence for 16 fixed 1 GiB arms. A 1 MiB buffer improved SSD wall 29.4921%/throughput 42.3401% but regressed HDD wall 5.3654%, so it is SSD-only; rotational/unknown remain 64 KiB. The Windows sequential hint improved wall 13.3373% on SSD and 7.8503% on HDD and is enabled globally. Exact hashes/bytes, 1 MiB cancellation granularity, 4 MiB maximum SSD aggregate buffer, SOP6 scheduling, strict Clippy, and serial Debug/Release Rust matrices pass (201 passed, 7 ignored). The initial parallel Debug matrix retained the documented heartbeat sensitivity at 3 rather than 4 samples; no threshold changed. |
| `SOP7e-partial-prefix-reuse` | `ready` | SOP7d | Compare rereading the accepted 1 KiB partial prefix with a bounded collision-workload design that continues a cache-miss full hash from the exact accepted prefix without trusting it across metadata change, cache hit, error, or cancellation. | The experiment reports both physical bytes and wall/resource cost on a declared collision-heavy workload. Reuse is admitted only if exact hashes and change detection hold, saved bytes reconcile, retained prefix state is explicitly bounded, and the measured workload improves; otherwise it is rejected. | Pending. |
| `SOP7f-read-path-acceptance` | `open` | SOP7b, SOP7c, SOP7d, SOP7e | Integrate only admitted factors, document every rejection/scope, and close SOP7 without altering SOP6 reader counts or SOP8 cache policy. | The named verifier plus full Debug/Release Rust and Windows matrices pass; retained evidence satisfies the gate metric fields, correctness/cancellation/cloud/hard-link/memory regressions and production locks pass, and SOP8+, unrelated campaigns, and parked validation do not run. | Pending. |

## Work selection and evidence rules

- `SOP2f-progress-acceptance` is `waived_by_operator_unmeasured`, and SOP2 is
  `accepted_with_operator_waiver`. Do not run v1 or v2, design SOP2f-v3, or describe the strict <1%
  representative wall/CPU gate as passed. Carry that residual risk to SOP9 and advance only through
  the dependency-ordered roadmap gates; SOP7 is next now that SOP6 is accepted.
- Do not optimize against the current UI counter. Establish file/byte/device metrics first.
- Small synthetic and sampled representative fixtures should tune changes before another full 10 TB
  campaign. A full-drive run is an acceptance artifact, not the inner development loop.
- Preserve passing and failing measurements with software build, input signature, volume/device,
  host context, sampling health, and observer cost.
- Compare medians and tail distributions as well as totals. Do not rerun until a favorable sample
  appears or attribute every tail to hardware without evidence.
- Correctness, cloud exclusion, hard-link, cancellation, bounded-memory, and production-execution
  locks outrank throughput.
- Update this plan, `ROADMAP.md`, and `windows-roadmap-session-handoff.md` whenever a gate changes
  state. The Windows closure ledger remains authoritative only for the parked release-validation
  stream.

## Decision log

| Date | Decision | Rationale |
|---|---|---|
| 2026-08-25 | Create a separate scan-scale and observability stream; park, do not discard, the Windows release-validation checklist. | Multiple 10 TB drives make whole-run time a primary product concern, while the release checklist must still resume before final feature-complete. |
| 2026-08-25 | Treat exact-size singleton short-circuiting as an open measured optimization. | Exact-size grouping exists, but the current partial-hash pass still opens singleton files. |
| 2026-08-25 | Put durable telemetry before algorithm changes and keep it in a separate worker-owned local status database. | Accurate cumulative counters and device evidence are required to explain progress, compare runs, and avoid contaminating immutable product-result truth with sampled operational data. |
| 2026-08-25 | Make sessions multi-package, checkpointed, audit-once, and safe to resume from a state-independent prompt. | Maximize useful progress per session while preventing repeated audits, progressively narrower follow-ups, and ambiguous recovery after context compaction or agent replacement. |
| 2026-08-25 | Advance the metrics contract to v2 during first lifecycle integration and distinguish actual hash-pipeline work from disjoint multi-file duplicate candidates. | The pre-optimization baseline reads singleton buckets; keeping both meanings in one counter would make the later singleton-I/O reduction impossible to measure truthfully. |
| 2026-08-25 | Retain the newest 50 terminal runs and 100,000 samples per run by default, cap run/sample pages at 100/500 and devices at 64, and remove terminal replay payloads during repeatable retention. | Multi-day scans need useful local history, while fixed row/page/device limits and passive checkpoints prevent the observability store from becoming an unbounded second product database. |
| 2026-08-25 | Use read-only native Windows host/volume/physical-disk probes behind a deterministic cadence seam, with no serial persistence and explicit unavailable gauge counts. | Device evidence must be gathered without WPF work, external helper processes, scan interruption, or false zero values when counters are blocked. |
| 2026-08-25 | Retain the first SOP1f Release overhead profile as a failed threshold result and do not retry the unchanged fixture. | The functional integration passes, but the 12,000-file short scan measured +4.68% wall and +1.98% CPU versus the published 1%/1% threshold; accepting an absolute or representative-duration budget requires explicit review. |
| 2026-08-25 | Accept SOP1f after batching status counters and retain both the failed and passing profiles. | The measured observer path performed 860 counter statements across ten phase flushes. One read and one atomic multi-row upsert per flush preserves fixed summaries, exact replay, and regression rejection; the first post-change comparable Release profile passed the unchanged threshold. Its negative deltas are recorded as noise, not claimed speedup. |
| 2026-08-25 | Carry the operator's repeat-scan cache proposal into `SOP8` after auditing current behavior. | Full hashes already persist across sessions for canonical-path/size/nanosecond-time matches, but partial reads repeat, renames miss, eviction is unbounded, no UI policy exists, and name/date alone is insufficient invalidation. The later gate now requires an explicit reuse-versus-revalidate policy and measured same/cross-session evidence. |
| 2026-08-25 | Split SOP2 into six dependency-ordered packages with Rust-owned cumulative truth, worker-owned projection/coalescing, and defensive Core application bounds. | The accepted telemetry contract is richer than live progress but updates only at phase boundaries; hasher callbacks wait for a whole size bucket; logical screened/cache-satisfied/finalized bytes are not all represented; worker phase events can bypass its timer; and Core can apply delayed running progress after cancelling or terminal state. One bounded contract-to-acceptance sequence closes each surface once without pulling SOP3/SOP4/SOP5/SOP8 forward. |
| 2026-08-25 | Keep progress contract v1 separate from durable metrics v2 and base ETA only on explicitly resolved logical candidate bytes. | SOP2a can close its pure reducer boundary without prematurely migrating status history. Physical partial/full rates remain actual I/O; screening may lead bucket classification, cache hits resolve work without reads, and the interleaved pipeline has no truthful global full-hash phase. Stable same-unit logical resolution is the only admitted ETA denominator. |
| 2026-08-25 | Publish hash-pipeline progress through one serialized engine sink at 256 file outcomes or 8 MiB, with supplemental logical failure files separate from content-read failures. | These bounds expose progress inside a large bucket/read without per-file status writes. General failures include paths that fail before a read begins, while durable metrics v2 truthfully keeps `full_hash_content_reads_failed` narrower. Completed, failed, and cancelled terminal snapshots now reconcile every overlapping counter with status storage. |
| 2026-08-25 | Project worker progress through one timer-owned latest-value emitter and keep unmapped device state explicit. | A single emission owner prevents immediate/timer reordering, the 100 ms slot enforces ten frames per half-open second, cancellation is latched before pending emission, and joining the emitter before terminal publication prevents stale progress. The current producer has no trustworthy root/work-to-device association, so `mapping_unavailable` is more accurate than an inferred key. |
| 2026-08-25 | Require the Windows client to validate the complete typed progress contract, then pass accepted frames through one generation-scoped latest-only Core gate. | Exact JSON kinds and canonical decimal strings fail closed against stale or malformed paired workers. One outstanding delayed dispatcher closure plus a 100 ms acceptance interval prevents a second UI burst, while run/lifecycle generations and cumulative baselines prevent stale, regressing, running-after-cancelling, or post-terminal state from reviving. |
| 2026-08-25 | Present six successive funnel outcomes, keep hash-pipeline candidates as denominator context, and announce accepted snapshots on a separate five-second accessibility cadence. | The contract explicitly excludes candidates as a seventh outcome. Fixed six-item presentation avoids ambiguous `Files hashed`, while system brushes, narrow wrapping, stable focus/IDs, terminal overrides, and a monotonic `MostRecent` UIA channel make the accepted truth usable without turning the ten-per-second visual bound into ten spoken updates per second. |
| 2026-08-25 | Measure SOP2 observer overhead once with an isolated cross-revision Release worker A/B rather than rerunning the SOP1 status-database toggle. | Both arms of the old ignored Core test already execute the SOP2 producer and reducer, so SOP2 cost cancels out. The predeclared `0a3c1c1`/`f803cbd` comparison keeps accepted SOP1 telemetry enabled in both arms and uses the unchanged fixture/order/threshold while directly including the new producer, reducers, coalescer, and JSON transport. |
| 2026-08-25 | Retain the sole SOP2 profile as a failed wall result and stop at operator disposition after completing the functional matrix. | The cross-revision Release median measured +4.39% wall (+66.9243 ms) and -3.83% CPU (-109.375 ms, treated as noise) versus the published +1%/+1% ceilings. Only 10 bounded/~30 KB treatment frames were emitted, and no concrete measured optimization is likely to recover 339 wall basis points on this short fixture without another prohibited tune-and-retry loop. |
| 2026-08-25 | Approve a two-part SOP2f overhead budget and one immutable representative-duration campaign. | The short fixture now has fixed caps of 100 ms wall and 125 ms worker CPU, anchored respectively to one published coalescing slot and eight 15.625 ms Windows CPU-accounting quanta; its retained result passes without rerun. Steady-state acceptance still requires strictly less than 1% aggregate wall and CPU over five qualifying runs per revision on the fixed 600,008-file/4,605,870,080-byte fixture. The operator approved these exact parameters and authorized stopping the pre-existing Release app PID 48972; it was verified and stopped before campaign setup. |
| 2026-08-25 | Stop SOP2f at operator decision after the sole representative campaign failed before measurement. | Clean control/treatment/status-probe builds and fixed-fixture creation/validation passed, but the initial mandatory 600,008-file full-content conditioning pass exhausted the two-hour envelope before any arm. No aggregate exists, the measurement path remains absent, cleanup passed, and retry/tuning is prohibited. |
| 2026-08-25 | Predeclare separately versioned `SOP2f-representative-v2` without authorizing or running it. | V1 redundantly scheduled the same 600,008-file full-content conditioning before all 12 arms even though each complete scan rewarms the next arm's scan-relevant ranges. V2 keeps the fixture, revisions, warmups/order/count, qualification, exact reconciliation, fixed short caps, strict aggregate <1% wall/CPU gate, write-once/no-retry rules, and safe cleanup; it removes only those 12 redundant passes and uses a conservative five-hour envelope proven by an executable no-state preflight. SOP2f/SOP2 remain unaccepted at operator decision. |
| 2026-08-25 | Consume the sole authorized `SOP2f-representative-v2` invocation and retain its invalid outcome without retry. | V2 completed builds, exact fixture creation/validation, and one-time conditioning before control warmup 0, removing v1's pre-arm setup bottleneck. The warmup did not complete within 600 seconds; the invalid path then failed on an empty measured-run sum and removed the temp root before it could retain in-memory attempt facts. A recovered write-once incident records zero completed/measured arms, null aggregates, no threshold evaluation, cleanup/process absence, and the consumed authority without fabricated metrics. A new versioned design or SOP2 waiver/rejection now requires explicit operator direction. |
| 2026-08-26 | Record SOP2f as `waived_by_operator_unmeasured` and close SOP2 as `accepted_with_operator_waiver`. | SOP2a-SOP2e are accepted, functional verification and the fixed-cost short leg pass, and both invalid representative attempts remain retained with no measured aggregate. The strict <1% representative wall/CPU gate was not evaluated and is not a pass. Do not design or run SOP2f-v3; observe or retain the residual overhead risk during useful SOP9 real-drive acceptance work. |
| 2026-08-26 | Accept `SOP3a-live-warning-accounting` as the first dependency-ordered SOP3 package. | Existing completed-run warning paging was bounded, but live progress could exceed the last phase-accounted durable total. Higher accepted warning frames now commit an exact single-row fallback aggregate first, fail closed on persistence error, and retain exact accounting across specific phase replacement and restart without per-occurrence rows or a protocol/UI change. |
| 2026-08-26 | Accept `SOP3b-active-warning-page-snapshot` with durable mutation-safe paging. | Schema-v15 warning revisions and one-snapshot reads bind every cursor to exact run/sort/revision/status state, reject active mutation and terminal handoff instead of mixing pages, reconstruct after restart, and expose separately configured diagnostic-log metadata without making logs durable warning truth. |
| 2026-08-26 | Accept `SOP3d-accessible-warning-entry-and-acceptance` and close SOP3. | Progress routes its exact current run/count through the one bounded History drilldown, restores focus to the warning grid, exposes page-owned diagnostic application-log metadata separately from warning truth, and retains latest-only UIA announcements plus system-brush/keyboard automation contracts across terminal and restart. Focused and full Windows verification pass; SOP4 and all campaigns remain untouched. |
| 2026-08-26 | Accept `SOP4-performance-tab` as one three-package protocol/Core/WPF group. | Query-only SQL summaries keep raw samples below the worker boundary; Core hard-caps 25 history, six phase, and 64 device rows; the accessible virtualized tab exposes current/peak drive and run health plus context-qualified comparisons. Focused and full Debug/Release matrices pass with production locks unchanged. No campaign ran; SOP5 is next but not started. |
| 2026-08-26 | Accept `SOP5-skip-singleton-size-buckets` as one two-package algorithm/acceptance group. | Exact-size singletons now resolve from discovery metadata before the injected content-I/O seam; only multi-file buckets enter partial/full hashing. Existing metadata/candidate and physical-read counters expose the saved logical and physical I/O without a schema or protocol change. Mixed-bucket seam, terminal reconciliation, error/cancellation/cache/hard-link, full Debug/Release Rust and Windows, and production-lock checks pass. No representative/physical campaign ran; SOP6 is next but not started. |
| 2026-08-26 | Accept `SOP6-device-aware-scheduler` as one three-package device-contract/scheduler/evidence group. | Windows maps volumes to one physical extent and seek-penalty class without candidate-content access; unavailable or ambiguous mapping shares one conservative fallback. One fair dispatcher replaces nested hash reads with 1 rotational/unknown or 4 SSD readers while separate devices overlap. Retained 1/N/N/1 direct-I/O HDD/SSD evidence selects those defaults with exact checksums/I/O, and full Debug/Release matrices plus production locks pass. SOP7 is next but not started. |
| 2026-08-26 | Split `SOP7-hash-read-path` into six finite one-factor packages before implementation. | The accepted path has four independent variables: per-device path locality, bucket order, full-read buffer/read-ahead behavior, and rereading the partial prefix. One common write-once harness precedes isolated comparisons; only measured improvements may enter the product, while rejected or media-scoped results remain retained. SOP6 reader counts and SOP8 cache policy stay fixed. |
| 2026-08-26 | Accept `SOP7a-read-experiment-contract` without changing production read behavior. | The common test-only harness isolates one factor, uses the retained SOP6 reader ceilings, fixes alternating arm order and distinct deterministic fixtures, records complete required evidence, rejects malformed/write-reused output, and proves checksum/byte/cancellation/memory bounds before any physical comparison. SOP7b path locality is next. |
| 2026-08-26 | Accept `SOP7b-path-locality` with the proposed product change rejected. | The cached SSD workload materially regressed, while direct rotational evidence showed only a 1.21% improvement on an idealized parent/allocation-local fixture. Both write-once results, including explicit unavailable SSD device samples, are retained without retry or discarded samples. Production insertion order remains unchanged; SOP7c is next. |
| 2026-08-26 | Accept `SOP7c-bucket-ordering` and select descending exact-size admission. | Fixed direct-read evidence improved mean wall time on both representative media classes while preserving every checksum, byte, counter, reader ceiling, and cleanup bound. A transient cleanup failure wrote no evidence and was corrected with bounded retry before the sole retained SSD run. Full Debug/Release Rust matrices and the evolved SOP6 verifier pass. |
| 2026-08-26 | Accept `SOP7d-buffer-read-ahead` with media-scoped buffers and the Windows sequential hint. | Direct/write-through fixture creation makes the buffered profiles physical cache-miss comparisons. The 1 MiB buffer is materially faster on SSD but slower on HDD, selecting 1 MiB only for SSD and the conservative 64 KiB for rotational/unknown. The sequential hint improves both media classes. Exact hashes, bytes, cancellation granularity, and aggregate memory bounds pass full serial Debug/Release Rust. |
