# Large-Drive Scan Optimization and Observability Plan

## Status

Active implementation plan. The current pipeline audit is complete; product implementation has not
started. This is the scheduled roadmap stream while the Windows post-MVP release-validation
checklist is parked. Before the product is declared feature complete, the release-validation stream
must resume at `WPM8-high-contrast` and follow its closure ledger to completion.

This plan exists because representative use includes several roughly 10 TB drives. A full baseline
can take days, so optimization work must be driven by durable measurements, preserve exact-duplicate
correctness, and avoid requiring repeated uninstrumented whole-drive runs.

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
- singleton-size bucket/file/byte count resolved without content I/O;
- multi-file candidate bucket/file/byte count;
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
| `SOP1-telemetry-foundation` | `local_code` | `ready` | `SOP0-current-pipeline-audit` | Implement the versioned metrics contract, worker-owned status database, bounded sampler/retention, and fixed summary/time-series queries. | Migration/recovery/bounds tests pass; simulated counters reconcile; unavailable samples are explicit; instrumented fixture overhead stays below 1% wall time and 1% CPU or the retained evidence explains and reviews a stricter measured budget. |
| `SOP2-progress-reporting` | `local_code` | `open` | `SOP1-telemetry-foundation` | Publish the candidate funnel, byte progress, rates, cache outcomes, and bounded-confidence ETA with bucket-independent coalescing. | Deterministic tests prove monotonic counters, no semantic mixing, cancellation/stale rejection, and at most ten Core/WPF updates per second. |
| `SOP3-current-warning-log` | `local_code` | `open` | `SOP1-telemetry-foundation` | Make active warnings visible through bounded structured paging while preserving completed-run aggregates and diagnostic logs. | Every current warning count is drillable or represented by a truthful bounded aggregate; restart/terminal handoff and cache bounds pass. |
| `SOP4-performance-tab` | `local_code` | `open` | `SOP1-telemetry-foundation`, `SOP2-progress-reporting`, `SOP3-current-warning-log` | Add the bounded live/history Performance tab with drive information and run comparison. | Core/WPF never bind full samples; keyboard, automation, unavailable-state, high-contrast, focus, restart, and representative-history tests pass. |
| `SOP5-skip-singleton-size-buckets` | `local_code` | `open` | `SOP1-telemetry-foundation`, `SOP2-progress-reporting` | Resolve exact-size singleton buckets without opening file content and make the saved I/O visible. | Correctness fixtures are unchanged; an injected read seam proves zero partial/full opens for singleton buckets; counters reconcile files/bytes as metadata-resolved rather than hashed. |
| `SOP6-device-aware-scheduler` | `local_code` | `open` | `SOP5-skip-singleton-size-buckets` | Replace nested global read parallelism with bounded per-physical-device queues: conservative concurrency on rotational media and measured concurrency on SSDs, while allowing separate devices to progress independently. | Deterministic scheduling/cancellation tests pass; retained 1/N-reader comparisons on representative devices show the selected policy and no correctness or memory regression. |
| `SOP7-hash-read-path` | `local_code` | `open` | `SOP6-device-aware-scheduler` | Benchmark path locality, bucket ordering, buffer/read-ahead size, and reuse of the partial prefix during full hashing. Admit only individually measured changes. | Each retained A/B run records bytes, IOPS, queue, latency, throughput, CPU, memory, and wall time; changes that do not improve the declared workload are rejected or scoped. |
| `SOP8-repeat-run-cache` | `local_code` | `open` | `SOP7-hash-read-path` | Reduce repeat-run reads with explicit identity/size/time invalidation, bounded eviction, and rename/hard-link semantics. | Warm-run fixtures prove exact invalidation and correctness, cache bounds, corruption fallback, and measured read/wall-time savings. |
| `SOP9-large-drive-acceptance` | `operator_evidence` | `open` | `SOP4-performance-tab` through `SOP8-repeat-run-cache` | Retain representative single- and multi-drive Release runs, including failures, and select defaults from evidence. | Duplicate results match reference fixtures; singleton reads are zero; telemetry/warning accounting is complete; memory and UI bounds hold; before/after device and wall-time evidence is retained without retry-only acceptance. |

## Work selection and evidence rules

- Advance one gate, or one explicitly named coherent gate group with one verifier, per implementation
  slice. The immediate implementation gate is `SOP1-telemetry-foundation`.
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
