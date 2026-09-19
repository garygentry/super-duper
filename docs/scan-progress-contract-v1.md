# Scan progress contract v1

This contract defines the platform-neutral cumulative progress snapshot that the worker attaches as
`progress` to every `run.progress` event (see
[Run events and ordering](worker-protocol-v1.md#run-events-and-ordering)). It is separate from the
embedded metrics contract, which defines the `counters` object; the metrics contract is currently
v3 and can advance without changing progress contract v1.

Sources: the reducer and snapshot types are in `crates/super-duper-core/src/telemetry/progress.rs`;
the wire projection that turns byte quantities into decimal strings is in
`crates/super-duper-worker/src/progress_projection.rs`. The Windows client validates the payload in
`apps/windows/src/SuperDuper.Windows.Infrastructure/WorkerRunProgressParser.cs` and
`apps/windows/src/SuperDuper.Windows.Core/Workers/WorkerProgressContract.cs`.

## Version and units

- `progressContractVersion` is exactly `1`; other versions fail closed.
- `metricsContractVersion` identifies the embedded counter meaning and is exactly the current
  metrics contract version, `3`. The producer rejects other values, and the Windows client accepts
  only `3`.
- File and byte counters are unsigned, monotonic, cumulative values for one run.
- Funnel bytes are logical file sizes. Partial/full throughput bytes are actual physical bytes read.
  They are never subtracted from one another or relabelled.
- File rates use thousandths of a logical file per second. Physical byte rates use bytes per second.
  The ETA field `logical_bytes_per_second_millis` is thousandths of a resolved logical byte per
  second. Every rate includes its measured window in nanoseconds.
- Missing rates, cache denominators, active-device mappings, remaining work, and ETA are explicit
  unavailable states rather than zero.

## Funnel and remaining work

The six successive outcomes are discovered, metadata-resolved, partial-screened,
selected-full-hash, full-hash-satisfied, and finalized-duplicate file/logical-byte quantities.
`hash-pipeline candidates` is the known denominator/context for those outcomes, not a seventh
successive stage.
Supplemental live logical counters (the `logical` object) provide meanings that the metrics
counters do not contain:

- partial-screened files and logical bytes;
- full-hash requested logical bytes;
- full-hash satisfied files/logical bytes, whether read or served from cache;
- failed full-hash files/logical bytes, including failures before a content read can start;
- hash-pipeline resolved files/logical bytes; and
- confirmed duplicate logical bytes.

Partial screening may advance before a size bucket is fully classified. Hash-pipeline resolution is
therefore allowed to lag screening, but it cannot exceed work classified as resolved by non-full-
hash screening plus completed/failed full-hash outcomes. This prevents a later collision from
making a published cumulative value regress.

Once candidate totals are known, remaining work is
`candidate logical bytes - hash-pipeline resolved logical bytes`, labelled with the
`hash_pipeline` stage. The current interleaved per-bucket algorithm does not have a truthful global
full-hash phase or denominator, so v1 does not manufacture a separate full-hash ETA.

## Rates, cache, ETA, and devices

The reducer publishes run-cumulative and recent partial/full read rates with these constants:

| Meaning | Value |
|---|---:|
| Rate-point minimum interval | 100 ms |
| Recent physical-read window | 30 s |
| Retained rate points | 304 maximum |
| ETA warm-up span | 10 s |
| ETA stability intervals | two consecutive intervals of at least 5 s |
| Minimum slow/fast logical-resolution rate ratio | 75% |

Dense observations replace the current 100 ms rate bucket; history is also time- and count-bounded.
Integer projection uses checked or saturating `u128` intermediates and never divides by zero. A rate
with no elapsed time is `unavailable` with reason `no_elapsed_time`.

Partial read rates count partial-screened files and partial-hash physical bytes read. Full read
rates count completed plus failed full-content reads and full-hash physical bytes read.

Cache hit rate is count based and combines the partial-hash and full-hash caches:

`(partial hits + full hits) / (partial hits + misses + errors + full hits + misses + errors)`

`cacheHitRateBasisPoints` reports it as an integer from 0 to 10,000 (basis points, rounded down).
It is `null` before any lookup outcome. Requests, stores, and bytes are not part of the
denominator.

ETA divides remaining logical candidate bytes only by stable logical candidate-resolution bytes per
second, and only in the `candidate_screening` phase. It is unavailable with reason
`work_not_yet_known`, `window_warming`, `no_recent_progress`, `unstable_rate`, or `not_applicable`
until the corresponding condition is satisfied; zero remaining with finalized results is
`complete`.

Active-device state is one non-secret device key, two through 64 unique device keys, or an
explicit unavailable reason (`no_active_io`, `mapping_unavailable`, or `ambiguous`). Keys contain 1
to 256 bytes and are not blank. The current producer reports `mapping_unavailable` because it has no
trustworthy non-secret root/work-to-device association.

## Transition rules

Each observation validates the embedded metrics invariants, supplemental funnel relationships,
phase time, phase ordering, version, bounded device state, and every cumulative transition before
the reducer changes state. Rejected observations do not consume a revision or rate point.
Candidate totals and completion knowledge cannot regress; known candidate totals cannot change.
Recent physical rates and ETA history are partitioned by live phase while cumulative physical rates
remain run-wide. The metrics contract's `overall` phase is not a live progress phase, and its
unused `full_hashing` phase is reserved and rejected in progress v1; the current producer reports
its interleaved hash activity as `candidate_screening`.

## Producer and transport

The scan engine populates cumulative observations from the same bounded hash deltas used for
terminal status truth. It publishes at 256 file outcomes or 8 MiB of full-content reads
(`crates/super-duper-core/src/hasher/xxhash.rs`) and reconciles cancellation, warnings, cache
outcomes, failures, and completed/failed/cancelled terminal counters without adding per-file status
rows. The worker reduces those observations and emits latest-wins frames at most once per 100 ms,
with decimal-string byte quantities and strict terminal ordering: no `run.progress` event follows a
terminal lifecycle event.

## Progress payload fields

These are the fields of the `progress` object as the worker serializes it and the Windows client
parser requires them. Field names are case-sensitive. Most names are camelCase; the fields inside
the tagged `activeDevices` and `eta` variants are snake_case, as emitted. "String" means a
canonical unsigned base-10 decimal string; "number" means a JSON number.

### Top level

| Field | JSON kind | Meaning |
|---|---|---|
| `progressContractVersion` | number | Always `1` |
| `metricsContractVersion` | number | Always `3` |
| `revision` | number | Source observation order; positive; coalescing can skip values |
| `monotonicNanos` | number | Monotonic time of the observation |
| `phase` | string | `discovering`, `candidate_screening`, `persisting`, `analyzing_folders`, or `finalizing` |
| `phaseElapsedNanos` | number | Time spent in the current phase; not greater than `monotonicNanos` |
| `counters` | object | The 47 metrics counters; see below |
| `logical` | object | Supplemental logical counters; see below |
| `funnel` | object | Seven stage quantities; see below |
| `partialReadRates` | object | `cumulative` and `recent` rate values |
| `fullReadRates` | object | `cumulative` and `recent` rate values |
| `cacheHitRateBasisPoints` | number or `null` | Combined cache hit rate, 0–10,000 |
| `warningCount` | number | Equals `counters.warnings` |
| `activeDevices` | object | Tagged device state; see below |
| `remainingKnownWork` | object or `null` | `null` until candidate totals are known |
| `eta` | object | Tagged ETA state; see below |

The legacy top-level `phase` of the event maps from `progress.phase`: `candidate_screening` is
`hashing`, and every other value is unchanged.

### `counters`

Number fields: `discoveredFiles`, `zeroByteFiles`, `hardLinkAliasFiles`, `sizeBuckets`,
`singletonSizeBuckets`, `singletonSizeFiles`, `candidateSizeBuckets`, `candidateFiles`,
`duplicateCandidateSizeBuckets`, `duplicateCandidateFiles`, `metadataResolvedFiles`,
`partialHashesAttempted`, `partialHashesSucceeded`, `partialHashesFailed`, `partialHashCacheHits`,
`partialHashCacheMisses`, `partialHashCacheErrors`, `partialHashCacheStores`,
`partialCollisionBuckets`, `partialCollisionFiles`, `fullHashRequests`, `fullHashCacheHits`,
`fullHashCacheMisses`, `fullHashCacheErrors`, `fullHashCacheStores`,
`fullHashContentReadsStarted`, `fullHashContentReadsCompleted`, `fullHashContentReadsFailed`,
`confirmedDuplicateGroups`, `confirmedLogicalCopies`, `confirmedPhysicalItems`, `warnings`,
`cancelChecks`, `cancelledWorkItems`, `telemetrySamplesLost`, `telemetryFlushErrors`, and
`unavailableCounters`.

String (byte) fields: `discoveredBytes`, `hardLinkAliasBytes`, `singletonSizeBytes`,
`candidateBytes`, `duplicateCandidateBytes`, `metadataResolvedBytes`, `partialHashBytesRead`,
`partialCollisionBytes`, `fullHashBytesRead`, and `recoverableBytes`.

### `logical`

| Field | JSON kind |
|---|---|
| `partialScreenedFiles` | number |
| `partialScreenedBytes` | string |
| `fullHashRequestBytes` | string |
| `fullHashSatisfiedFiles` | number |
| `fullHashSatisfiedBytes` | string |
| `fullHashFailedFiles` | number |
| `fullHashFailedBytes` | string |
| `hashPipelineResolvedFiles` | number |
| `hashPipelineResolvedBytes` | string |
| `confirmedLogicalBytes` | string |

### `funnel`

Each stage is `{ "files": <number>, "logicalBytes": <string> }`.

| Stage key | Files from | Logical bytes from |
|---|---|---|
| `discovered` | `counters.discoveredFiles` | `counters.discoveredBytes` |
| `metadataResolved` | `counters.metadataResolvedFiles` | `counters.metadataResolvedBytes` |
| `hashPipelineCandidates` | `counters.candidateFiles` | `counters.candidateBytes` |
| `partialScreened` | `logical.partialScreenedFiles` | `logical.partialScreenedBytes` |
| `selectedForFullHash` | `counters.fullHashRequests` | `logical.fullHashRequestBytes` |
| `fullHashSatisfied` | `logical.fullHashSatisfiedFiles` | `logical.fullHashSatisfiedBytes` |
| `finalizedDuplicates` | `counters.confirmedLogicalCopies` | `logical.confirmedLogicalBytes` |

### Rate values

`partialReadRates` and `fullReadRates` each contain `cumulative` and `recent`. Each is tagged by
`state`:

- `available`: `rate` is `{ "filesPerSecondMillis": <number>, "physicalBytesPerSecond": <string>,
  "windowNanos": <number> }`.
- `unavailable`: `reason` is `no_elapsed_time`.

### `activeDevices`

Tagged by `state`:

- `unavailable`: `reason` is `no_active_io`, `mapping_unavailable`, or `ambiguous`.
- `one`: `device_key` is a string.
- `multiple`: `device_keys` is an array of 2–64 unique strings.

### `remainingKnownWork`

`null`, or `{ "stage": "hash_pipeline", "files": <number>, "logicalBytes": <string> }`.

### `eta`

Tagged by `state`:

- `available`: `stage` (`hash_pipeline`), `remaining_logical_bytes` (string),
  `logical_bytes_per_second_millis` (string), `estimated_seconds` (number), and `window_nanos`
  (number).
- `unavailable`: `reason` is `work_not_yet_known`, `window_warming`, `no_recent_progress`,
  `unstable_rate`, or `not_applicable`.
- `complete`: no other fields.

## Windows client behavior

The Windows client fails closed on an incomplete or semantically invalid snapshot: a missing field,
a wrong JSON kind, a non-canonical decimal string, an unsupported contract version, an unknown
tagged state or reason, or a broken counter or funnel invariant makes the event a protocol error.
It also requires the legacy top-level fields to agree with the snapshot: `bytesDiscovered` equals
`counters.discoveredBytes`, `filesDiscovered` equals `discoveredFiles - zeroByteFiles`,
`filesHashed` equals `partialHashesSucceeded`, and `warningCount` equals `progress.warningCount`.
It ignores unknown additive fields.

Valid frames pass through one generation-scoped, latest-only 100 ms Core application gate. Core
revalidates run, transport/source order, cumulative non-regression, and lifecycle stickiness before
projecting explicit units, windows, and unavailable explanations. The scan view renders the six
successive outcomes while keeping hash-pipeline candidates as denominator context, preserves the
last accepted funnel across lifecycle changes, and overrides stale active-I/O/remaining/ETA claims
after cancelling or terminal state. Its path-free UI Automation summary announces the first
accepted snapshot and phase/status changes immediately, then at most once per five seconds, with
one monotonic cross-run version and latest-only processing.
