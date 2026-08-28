# SOP9c Single-Drive Reference/Repeat Protocol V2

This protocol corrects the demonstrated V1 campaign-runner defect for
`SOP9c-single-drive-reference-repeat`. It does not authorize an invocation. The operator authorized
this design-only slice on 2026-08-28; exactly one physical invocation requires a separate explicit
operator approval after this protocol, its runner, and its verifier are committed and reviewed.

That separate approval was later granted and is now consumed. The sole V2 invocation retained its
manifest and append-only journal, passed the fixed physical preflight, created scoped state, and
completed both pinned Release builds. The consuming campaign host then ended before `worker_started`,
an arm, or native evidence finalization. The surfaced guarded admission failed closed because the V2
evidence root already existed. A post-exit audit found no worker and no scoped V2 state. The recovered
path-free [incident record](evidence/scan-large-drive-single-drive-v2-invalid-20260828.json) pins the
four raw files, zero scan/result/measurement truth, cleanup observations, and consumed authority
without attributing an unproved product, runner, watchdog, or cleanup cause.

V2 is invalid and cannot be rerun or overwritten. A successor protocol/identity requires a separately
reviewed causal defect and explicit operator design and execution authority. None is granted here;
SOP9c remains blocked and SOP9d/SOP9e remain dependency-blocked.

Nothing in this protocol authorizes deletion, Recycle Bin execution, an SOP2f rerun, SOP9d, parked
release validation, a substitute root, or modification/removal of retained V1 evidence or diagnostic
state.

## Fixed identity and unchanged campaign contract

| Identity | Root | Physical mapping | Arm order | Required terminal |
|---|---|---|---|---|
| `sop9c-single-drive-reference-repeat-v2` | E: | healthy NTFS 14 TB HDD, physical disk 1 | `revalidate_content`, `reuse_verified` | both `completed` |

The write-once evidence path is
`artifacts/windows-sop9-large-drive/sop9c-single-drive-reference-repeat-v2`. The isolated state path
is `H:\super-duper-sop9-state\sop9c-single-drive-reference-repeat-v2`. Both must be absent before
admission. The runner must revalidate E:'s identity, mapping, filesystem, media, health, and capacity
plus at least 50 GiB of H: free space before creating state. No other root or state parent may be
substituted.

V2 starts with a new empty product database, status database, and bounded hash cache. It does not
resume V1's incomplete product database or reuse V1's cache. Its first arm therefore remains the
forced reference and its second arm alone evaluates verified reuse against the V2 store.

All V1 correctness, measurement, evidence-retention, SOP2 residual-risk, and safe-cleanup rules in
[`scan-large-drive-acceptance-protocol-v1.md`](scan-large-drive-acceptance-protocol-v1.md) remain
unchanged. In particular, every setup, arm, unavailable-counter, finalization, worker-stop, and
cleanup outcome is retained; there is no favorable retry or outlier removal; raw user paths do not
enter committed evidence; and the strict SOP2 `<1%` overhead gate remains unevaluated.

## Demonstrated causal defect

V1 read one pending worker stdout line with a fixed 180-second deadline in every phase. The forced
E: arm accepted a `persisting` progress frame after 244,141.692 ms discovery and 20,496,093.733 ms
hashing. Persistence continued to update the product WAL, but no additional protocol frame arrived
within 180,033.3426 ms. The runner treated protocol quietness as worker failure and force-stopped the
owned process before a terminal event or result snapshot. This was a runner deadline defect, not a
completed-arm result and not evidence that persistence had stalled.

V2 changes only this causal runner behavior. It does not change the worker, product persistence,
scan inputs, cache policies, device scheduler, read path, result snapshot, evidence schema,
correctness thresholds, or cleanup authority.

## Bounded V2 frame watchdog

Outside an accepted `persisting` progress phase, the V1 180-second protocol-frame deadline remains
unchanged. Once V2 accepts `persisting` for the active run:

1. The runner retains exactly one outstanding `ReadLineAsync` operation; polling never starts a
   second competing stdout read.
2. Every five seconds it checks that the owned worker has not exited and observes only metadata for
   the campaign-owned `product.db` and `product.db-wal`: existence, byte length, and last-write
   ticks. It does not read product rows or user files. Status-database heartbeat activity is
   deliberately excluded so observer writes cannot mask stalled product persistence.
3. Any state fingerprint change advances the last-activity time. The append-only journal records
   watchdog admission and at most one activity summary per ten minutes, not every probe.
4. Fifteen consecutive minutes without a state fingerprint change fails the arm with the explicit
   `state_activity_idle_bound` reason. This is five times V1's invalid fixed deadline and distinguishes
   active persistence from an owned worker that is alive but no longer producing durable activity.
5. Persistence has an absolute 24-hour phase bound even while activity continues. This is more than
   four times the retained forced arm's 5.69-hour hashing duration while remaining finite. Reaching
   it fails with the explicit `absolute_phase_bound` reason.
6. A worker terminal frame closes the watchdog and journals its duration and observed activity-change
   count before normal snapshot and arm validation continue.

The absolute bound is evaluated before activity renewal, so continuous writes cannot make the
campaign unbounded. Probe errors become a stable `unavailable` fingerprint; they do not fabricate
activity and eventually reach the idle bound unless metadata observation recovers and changes.

## Pre-execution review and separate authority

`Verify-WindowsLargeDriveSingleDriveV2Protocol.ps1` must pass before any invocation approval. It
parses the runner/helper, executes only the pure fake-time watchdog boundary test and source-only
campaign description, proves the one-pending-read contract and fixed bounds, pins the immutable V1
incident summary, checks the V2 protocol/plan/handoff state, and preserves every production deletion
lock. It performs no build, physical preflight, worker start, E: scan, evidence reservation, or H:
campaign-state creation/removal.

After the design commit, an operator could separately authorize exactly one command for the fixed V2
identity. The execution command had to include the fail-closed `-RunPhysicalCampaign` switch; naming
the identity alone could not reserve evidence or start the worker. That approval consumed the
identity whether setup, build, preflight, an arm, finalization, or cleanup succeeded or failed. It is
now consumed by the invalid outcome above. Do not run that command again.
