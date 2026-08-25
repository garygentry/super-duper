# Scan Progress Contract v1

This contract is the platform-neutral cumulative truth for `SOP2-progress-reporting`. It is
separate from metrics contract v2: SOP2a defined and tested the projection without migrating status
history, SOP2b connected one serialized producer to live and durable counter truth, and SOP2c added
the bounded worker transport projection.

## Version and units

- `progressContractVersion` is exactly `1`; other versions fail closed.
- `metricsContractVersion` identifies the embedded `ScanCounters` meaning and is exactly the
  current metrics contract version.
- File and byte counters are unsigned, monotonic, cumulative values for one run.
- Funnel bytes are logical file sizes. Partial/full throughput bytes are actual physical bytes read.
  They are never subtracted from one another or relabelled.
- File rates use thousandths of a logical file per second. Physical byte rates use bytes per second.
  `logicalBytesPerSecondMillis`, used only for ETA, is thousandths of a resolved logical byte per
  second. Every rate includes its measured window in nanoseconds.
- Missing rates, cache denominators, active-device mappings, remaining work, and ETA are explicit
  unavailable states rather than zero.

## Funnel and remaining work

The six successive outcomes are discovered, metadata-resolved, partial-screened,
selected-full-hash, full-hash-satisfied, and finalized-duplicate file/logical-byte quantities.
`hash-pipeline candidates` is the known denominator/context for those outcomes, not a seventh
successive stage.
Supplemental live logical counters provide meanings that metrics v2 does not contain:

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

The reducer publishes run-cumulative and recent partial/full read rates. The provisional constants
are fixed until representative SOP9 evidence justifies a reviewed change:

| Meaning | Value |
|---|---:|
| Rate-point minimum interval | 100 ms |
| Recent physical-read window | 30 s |
| Retained rate points | 304 maximum |
| ETA warm-up span | 10 s |
| ETA stability intervals | two consecutive intervals of at least 5 s |
| Minimum slow/fast logical-resolution rate ratio | 75% |

Dense observations replace the current 100 ms rate bucket; history is also time- and count-bounded.
Integer projection uses checked or saturating `u128` intermediates and never divides by zero.

Cache hit rate is count based:

`hits / (hits + misses + errors)`

It is unavailable before a lookup outcome. Requests, stores, and bytes are not part of that
denominator.

ETA divides remaining logical candidate bytes only by stable logical candidate-resolution bytes per
second. It is `work_not_yet_known`, `window_warming`, `no_recent_progress`, `unstable_rate`, or
`not_applicable` until the corresponding condition is satisfied; zero remaining with finalized
results is `complete`.

Active-device state is one non-secret device key, two through 64 unique device keys, or an
explicit unavailable reason (`no_active_io`, `mapping_unavailable`, or `ambiguous`). The current
producer reports `mapping_unavailable` because it has no trustworthy non-secret root/work-to-device
association; a later measured device-aware scheduling package may add one.

## Transition rules and deferred integration

Each observation validates the embedded metrics invariants, supplemental funnel relationships,
phase time, phase ordering, version, bounded device state, and every cumulative transition before
the reducer changes state. Rejected observations do not consume a revision or rate point.
Candidate totals and completion knowledge cannot regress; known candidate totals cannot change.
Recent physical rates and ETA history are partitioned by live phase while cumulative physical rates
remain run-wide. Metrics v2's unused `full_hashing` phase is reserved and rejected in progress v1;
the current producer reports its interleaved hash activity as candidate screening.

SOP2b populates cumulative observations from the same bounded hash deltas used for terminal status
truth. It publishes at 256 file outcomes or 8 MiB of full-content reads and reconciles cancellation,
warnings, cache outcomes, failures, and completed/failed/cancelled terminal counters without adding
per-file status rows. SOP2c reduces those observations in the worker and emits latest-wins frames at
most once per 100 ms with decimal-string byte quantities and strict terminal ordering. SOP2d makes
the paired Windows client fail closed on incomplete or semantically invalid typed snapshots, then
passes valid frames through one generation-scoped latest-only 100 ms Core application gate. Core
revalidates run, transport/source order, cumulative non-regression, and lifecycle stickiness before
projecting explicit units, windows, and unavailable explanations. The WPF surface, warning
drilldown, singleton savings, scheduling, read-path tuning, device-aware scheduling, and cache-policy
UI remain in SOP2e and later named gates.
