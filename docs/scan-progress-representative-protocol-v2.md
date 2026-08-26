# SOP2f Representative Protocol V2

## Disposition and authority

`SOP2f-representative-v2` is a proposed, separately versioned replacement for the representative
leg of `SOP2f-progress-acceptance`. This design package does not authorize a campaign and does not
accept SOP2f or SOP2. The retained short result and the retained v1 pre-measurement failure remain
immutable. Exactly one v2 campaign may run only after the operator explicitly approves this
protocol and authorizes its campaign command.

The machine-readable declaration is
[`scan-progress-representative-protocol-v2.json`](scan-progress-representative-protocol-v2.json).
The executable harness is
[`Measure-WindowsScanProgressRepresentativeOverhead.ps1`](../scripts/Measure-WindowsScanProgressRepresentativeOverhead.ps1).

## Why v1 was infeasible

The v1 two-hour watchdog began before three clean Release builds, fixture creation, validation, and
conditioning. The builds alone took 23 minutes 13 seconds. The harness then performed an initial
full-content SHA-256 pass over 600,008 files and was also designed to repeat that same full-content
pass before every one of 12 arms. The retained attempt completed only the initial pass before the
watchdog check failed; no warmup or measured arm began. The interval from the harness commit time
to retained failure was at most 7,709 seconds, which gives a conservative retained upper bound for
the completed setup but no representative arm duration.

V1 therefore scheduled 13 conditioning passes over 4,605,870,080 logical bytes and 600,008 opens,
even though every complete scan already reads every range used by the next scan. That redundant
work, not the approved fixture or threshold, caused the protocol infeasibility.

## Exact v2 protocol

- Revisions remain control `0a3c1c1` (`0a3c1c13005c0409f6a3095e397d02dd11704a62`) and treatment
  `f803cbd` (`f803cbde5e72aa35d77aa509c5f6b86d5f9798f0`). Any compatibility defect stops at operator
  review; the harness must not substitute a revision.
- The fixture remains generator v1, SplitMix64 seed `0x534F503246524550`: 600,000 normal allocated
  4,096-byte unique-content files plus four normal allocated identical pairs of 268,435,456,
  268,500,992, 268,566,528, and 268,632,064 bytes. Totals remain 600,008 files and
  4,605,870,080 logical bytes. No fixture resizing or tuning is permitted.
- Creation is followed by exact count, length, allocation/reparse, manifest, enumeration, pair
  equality, pair distinction, and full-content digest validation. One revision-neutral full-content
  SHA-256 pass in manifest order both records the immutable digest and conditions the filesystem
  cache before the first warmup. A recursive filesystem mutation guard runs from before that pass
  through the final arm; any content/size/name/attribute/security event or watcher error/overflow
  invalidates the campaign.
- There is no separate per-arm conditioning pass. Every arm scans the complete unchanged fixture,
  including the 1 KiB partial baseline for all 600,008 files and full reads of all eight large
  duplicate members. A completed arm therefore rewarms exactly the scan-relevant byte ranges before
  the next arm. This preserves warm-filesystem-cache comparability without inserting a workload
  that is neither control nor treatment between arms.
- Every arm starts a new worker and a new state directory containing fresh product, status, and hash
  state. Logging remains off. Worker startup, session creation, state creation, result/status
  reconciliation, and cleanup remain outside the measured start-to-terminal interval.
- Warmups are control then treatment. Measured order is `C,T / T,C / C,T / T,C / C,T`, five runs
  per revision. This retains the approved counterbalance: every pair contains one run per revision,
  with three control-first pairs and two treatment-first pairs.
- Every arm must complete in 60 through 600 seconds, emit progress, finish successfully, reproduce
  the exact four duplicate groups and members, and reconcile terminal product truth with every
  deterministic durable counter. Treatment must exercise typed, mid-bucket, and mid-read progress;
  control must not emit typed progress.
- All five measured runs for each revision feed sum-of-all-runs wall and worker-process CPU ratios.
  Wall passes only when `treatmentWall * 100 < controlWall * 101`; CPU passes only when
  `treatmentCpu * 100 < controlCpu * 101`. Reported basis points are rounded only for display and
  never determine the strict comparison. Negative deltas mean no detected positive overhead, not
  acceleration.
- The retained short leg remains separately required at no more than 100,000,000 ns positive wall
  overhead and 125,000,000 ns positive worker-CPU overhead. V2 does not rerun it.

## Feasibility preflight and bounds

The no-state preflight validates the exact full revisions, retained evidence hashes, absent v2
write-once evidence, fixture arithmetic, conditioning count, order/count balance, threshold math,
campaign arithmetic, current free space, and absence of product/profile processes. It creates no
fixture, build, state database, hash cache, or evidence file.

The retained attempt bounds completed setup by 7,709 seconds and records 1,393 seconds of builds.
V2 allows 9,000 seconds for all setup. With 12 arms each conservatively reserved at the complete
600-second qualification maximum and 900 seconds reserved for final reconciliation and cleanup,
the derived envelope is 17,100 seconds (4 hours 45 minutes) inside a hard 18,000-second (5-hour)
watchdog. Expected per-arm duration is therefore deliberately budgeted as 1-10 minutes; no narrower
representative estimate is claimed before a valid arm exists. The campaign requires 30 GiB free
before setup, 20 GiB after builds and fixture creation, and 15 GiB before every arm.

Run the no-state preflight with:

```powershell
pwsh -NoProfile -File ./scripts/Measure-WindowsScanProgressRepresentativeOverhead.ps1 `
  -Protocol SOP2f-representative-v2 -PreflightOnly
```

## Evidence, invalidity, and cleanup

The one future authorized invocation creates exactly
`docs/evidence/scan-progress-representative-overhead-sop2f-v2.json` with `CreateNew` semantics. A
pass, threshold failure, qualification failure, setup failure after campaign admission, or partial
campaign all retain at that same path. The evidence records protocol ID, repository HEAD, harness
SHA-256, revisions,
fixture digests, setup timing, conditioning semantics, host/free-space facts, every attempted arm,
all available reconciliations, aggregates only when complete, threshold booleans, cleanup outcome,
and the exact disposition/failure. Once the path exists, the harness refuses every later attempt.

Any revision mismatch, retained-evidence mismatch, product process, free-space failure, setup or arm
timeout, fixture/result/counter mismatch, missing progress seam, process/cleanup failure, incomplete
arm set, or evidence-write failure invalidates the campaign. No retry, outlier exclusion, fixture
change, ordering/cache change, threshold change, adjacent profile, or manual evidence repair is
authorized. Safe cleanup is limited to the validated GUID-named campaign root under the resolved
temporary directory; reparse points are rejected and worker/probe absence is verified afterward.

After explicit operator approval, the single campaign command requiring authority to build the two
archived Release revisions, create/delete the temporary fixture and isolated state, launch the
workers/probe, and create the write-once repository evidence is:

```powershell
pwsh -NoProfile -File ./scripts/Measure-WindowsScanProgressRepresentativeOverhead.ps1 `
  -Protocol SOP2f-representative-v2 -RunCampaign
```

Protocol approval authorizes exactly that one invocation. It does not authorize another v1 run,
another v2 attempt, product changes, SOP3/SOP5/SOP8 work, or the parked release-validation stream.
