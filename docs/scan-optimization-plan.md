# Large-Drive Scan Optimization and Observability Plan

## Status

Active implementation plan. The current pipeline audit and the first five telemetry-foundation
packages are complete. This is the scheduled roadmap stream while the Windows post-MVP release-validation
checklist is parked. Before the product is declared feature complete, the release-validation stream
must resume at `WPM8-high-contrast` and follow its closure ledger to completion.

Current execution checkpoint:

- current gate: `SOP1-telemetry-foundation`;
- next work package: `SOP1f-foundation-acceptance`;
- last accepted work package: `SOP1e-host-device-sampler`;
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
| `SOP1-telemetry-foundation` | `local_code` | `in_progress` | `SOP0-current-pipeline-audit` | Implement the versioned metrics contract, worker-owned status database, bounded sampler/retention, and fixed summary/time-series queries. | Migration/recovery/bounds tests pass; simulated counters reconcile; unavailable samples are explicit; instrumented fixture overhead stays below 1% wall time and 1% CPU or the retained evidence explains and reviews a stricter measured budget. |
| `SOP2-progress-reporting` | `local_code` | `open` | `SOP1-telemetry-foundation` | Publish the candidate funnel, byte progress, rates, cache outcomes, and bounded-confidence ETA with bucket-independent coalescing. | Deterministic tests prove monotonic counters, no semantic mixing, cancellation/stale rejection, and at most ten Core/WPF updates per second. |
| `SOP3-current-warning-log` | `local_code` | `open` | `SOP1-telemetry-foundation` | Make active warnings visible through bounded structured paging while preserving completed-run aggregates and diagnostic logs. | Every current warning count is drillable or represented by a truthful bounded aggregate; restart/terminal handoff and cache bounds pass. |
| `SOP4-performance-tab` | `local_code` | `open` | `SOP1-telemetry-foundation`, `SOP2-progress-reporting`, `SOP3-current-warning-log` | Add the bounded live/history Performance tab with drive information and run comparison. | Core/WPF never bind full samples; keyboard, automation, unavailable-state, high-contrast, focus, restart, and representative-history tests pass. |
| `SOP5-skip-singleton-size-buckets` | `local_code` | `open` | `SOP1-telemetry-foundation`, `SOP2-progress-reporting` | Resolve exact-size singleton buckets without opening file content and make the saved I/O visible. | Correctness fixtures are unchanged; an injected read seam proves zero partial/full opens for singleton buckets; counters reconcile files/bytes as metadata-resolved rather than hashed. |
| `SOP6-device-aware-scheduler` | `local_code` | `open` | `SOP5-skip-singleton-size-buckets` | Replace nested global read parallelism with bounded per-physical-device queues: conservative concurrency on rotational media and measured concurrency on SSDs, while allowing separate devices to progress independently. | Deterministic scheduling/cancellation tests pass; retained 1/N-reader comparisons on representative devices show the selected policy and no correctness or memory regression. |
| `SOP7-hash-read-path` | `local_code` | `open` | `SOP6-device-aware-scheduler` | Benchmark path locality, bucket ordering, buffer/read-ahead size, and reuse of the partial prefix during full hashing. Admit only individually measured changes. | Each retained A/B run records bytes, IOPS, queue, latency, throughput, CPU, memory, and wall time; changes that do not improve the declared workload are rejected or scoped. |
| `SOP8-repeat-run-cache` | `local_code` | `open` | `SOP7-hash-read-path` | Reduce repeat-run reads with explicit identity/size/time invalidation, bounded eviction, and rename/hard-link semantics. | Warm-run fixtures prove exact invalidation and correctness, cache bounds, corruption fallback, and measured read/wall-time savings. |
| `SOP9-large-drive-acceptance` | `operator_evidence` | `open` | `SOP4-performance-tab` through `SOP8-repeat-run-cache` | Retain representative single- and multi-drive Release runs, including failures, and select defaults from evidence. | Duplicate results match reference fixtures; singleton reads are zero; telemetry/warning accounting is complete; memory and UI bounds hold; before/after device and wall-time evidence is retained without retry-only acceptance. |

## Current gate work-package ledger

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
| `SOP1f-foundation-acceptance` | `in_progress` | SOP1c, SOP1d, SOP1e | Integrate the packages, document retention/schema/recovery, and retain observer-overhead evidence. | Focused and full relevant matrices pass; instrumentation adds less than 1% wall time and 1% CPU on the declared fixture, or a reviewed measured budget is recorded; gate `SOP1` becomes accepted. | Single-writer five-second heartbeat integration and no-progress sampling fixture pass. [`scan-telemetry-overhead-20260825.json`](evidence/scan-telemetry-overhead-20260825.json) failed the 1% threshold at +4.68% wall/+1.98% CPU on a 1.25-second baseline (absolute +58.7 ms/+31.25 ms); operator budget review or measured overhead reduction remains. |

## Work selection and evidence rules

- Advance through dependency-ready work packages until a real stop condition in the resumable
  execution protocol applies. The immediate package is `SOP1f-foundation-acceptance`.
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
