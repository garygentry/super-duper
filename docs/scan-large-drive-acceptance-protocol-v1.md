# SOP9 Large-Drive Acceptance Protocol V1

This protocol governs `SOP9a` through `SOP9e` in
[`scan-optimization-plan.md`](scan-optimization-plan.md). It does not authorize any deletion,
Recycle Bin execution, SOP2f rerun, parked release-validation campaign, or root outside the fixed
SOP9 package ledger.

## Fixed campaign identities

| Identity | Roots | Arm order | Required terminal |
|---|---|---|---|
| `sop9b-representative-cancellation-v1` | D: / physical HDD 0 | `revalidate_content`, cancel after first accepted hashing read progress | `cancelled` |
| `sop9c-single-drive-reference-repeat-v1` | E: / physical HDD 1 | `revalidate_content`, `reuse_verified` | both `completed` |
| `sop9d-multi-drive-reference-repeat-v1` | D: + E: / physical HDDs 0 + 1 | `revalidate_content`, `reuse_verified` | both `completed` |

V1 disposition is now fixed. SOP9b is accepted. The sole SOP9c V1 identity was consumed on
2026-08-27/28 and is retained as invalid: its forced arm entered result persistence, then the
runner's 180-second protocol-frame deadline fired before a terminal frame, force-stopped the worker,
preserved diagnostic state, and never started `reuse_verified`. V1 cannot be rerun or overwritten.
SOP9d remains unconsumed but blocked by its SOP9c dependency. A corrected protocol version and new
single-drive identity require the plan/handoff update and explicit operator authority described
below; neither is authorized by this document.

The evidence path is `artifacts/windows-sop9-large-drive/<identity>`. The isolated state path is
`H:\super-duper-sop9-state\<identity>`. Both must be absent before the attempt. Drive letters,
physical mappings, NTFS, media type, health, capacity, and at least 50 GiB of H: free space are
revalidated before evidence reservation. A mismatch blocks the package; it is not permission to
substitute another root.

## Write-once and failure retention

The runner creates `manifest.json` and `attempt.jsonl` before building or starting the worker.
Creation consumes the campaign identity. Existing evidence or state refuses admission. The journal
is append-only and flushed after every retained entry. Build, setup, protocol, counter-unavailable,
run, cancellation, snapshot, worker-stop, state-cleanup, and evidence-finalization failures remain
in that directory. No attempt, arm, tail, unavailable value, cancellation, or cleanup failure may
be discarded or replaced by a favorable retry.

`acceptance-evidence.json` is created with create-new semantics after cleanup is attempted. If it
cannot be created, the manifest/journal and diagnostic state remain the evidence. A new protocol
version requires a demonstrated causal defect, an explicit plan/handoff update, and operator
authority. It never overwrites V1.

## Correctness and bounded evidence

Each arm owns an isolated immutable product run and a matching status run. A query-only Release
helper streams stable file/folder rows in deterministic order into SHA-256 without emitting raw
paths. It reports the immutable run policy plus hashed parameter/root identities, product
aggregates, exact warning accounting, all fixed overall counters, retained terminal commit sequence
and database/process-write volume, and distributions over the already retained maximum
100,000 host/device samples and 64 devices. The committed summary contains root/device hashes and
aggregates, not user paths.

Completed forced/reuse arms must have identical file and folder result digests and aggregates.
Counters must prove singleton files are metadata-resolved rather than hash candidates, hard-link
aliases are excluded from recoverable copies, actual read/cache outcomes reconcile, warnings are
fully accounted, and host/device unavailable values remain explicit. The cancellation arm must
publish no progress after terminal and must never expose partial results as completed.

## SOP2 residual-risk statement

SOP9 records progress-frame counts/serialized bytes/rate, retained status commit sequence plus
database/process-write volume, worker CPU/memory/process I/O, device distributions, and sampling
health during useful representative runs. Terminal retention deliberately removes replay payload
rows, so their post-terminal count and bytes remain recorded as zero rather than being misreported
as absent observation. These are observer-cost proxies at SOP9 scale. There is no observer-off counterfactual in this
protocol, so they cannot causally isolate progress overhead. `strictGateEvaluated` and
`strictGatePassed` remain false. SOP2's waived strict `<1%` wall/CPU gate stays unevaluated; any
unattributed observer cost remains explicit residual risk.

## Cleanup

Only the worker process started by the runner may be stopped. A state path is recursively removed
only after its normalized absolute path is proven to be a strict child of the fixed campaign state
parent and is not a reparse point. Valid terminal snapshots permit cleanup; a pre-snapshot failure
preserves diagnostic state. Every cleanup outcome is retained. Evidence directories are never
removed by the physical runner.
