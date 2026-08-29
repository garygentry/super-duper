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
- Latest completed slice: review the consumed invalid V2 facts at the operator-decision boundary,
  establish no causal defect, and retain SOP9c blocked without a successor
- Worktree at the start of the causal review: clean at `4866ab5`
- Active stream: large-drive scan optimization and observability
- Active plan: `docs/scan-optimization-plan.md`
- Current gate: `SOP9-large-drive-acceptance` blocked after the consumed invalid V2 attempt for
  `SOP9c-single-drive-reference-repeat`
- Next boundary: retain `SOP9c-single-drive-reference-repeat` as `blocked_invalid_campaign`; only
  new causal evidence may return a separately versioned successor design for explicit operator
  approval, and any physical invocation requires another separate approval; do not rerun V1/V2 or
  start SOP9d
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

`SOP2f-progress-acceptance` is `waived_by_operator_unmeasured`, and SOP2 is
`accepted_with_operator_waiver`. SOP2a through SOP2e are accepted; functional verification and the
fixed-cost short leg pass at +66.9243 ms wall and no detected positive CPU cost against the 100 ms
wall/125 ms worker-CPU caps. The retained v1 pre-measurement failure contains no completed arm. The
sole authorized v2 attempt completed setup and initial conditioning, but control warmup 0 did not
complete inside its bound; its empty-aggregate defect prevented native invalid-evidence
serialization. The recovered write-once incident retains zero completed/measured arms, null
aggregates, cleanup facts, and an unevaluated strict <1% wall/CPU gate. That gate did not pass. Do
not run v1/v2 or design SOP2f-v3. The unresolved representative-overhead risk is assigned to SOP9,
where it may be observed during useful real-drive acceptance work. SOP3 through SOP7 are now
accepted; the next dependency boundary is SOP8.

`SOP3a-live-warning-accounting` is accepted as the first of four finite SOP3 packages. The audit
found that schema-v14 completed-run aggregates/examples, worker paging, the five-page Core cache,
25-row virtualized WPF page, and diagnostic log already exist, but accepted live progress could
advance beyond the last phase-accounted product-database warning total. Worker progress now forces
one atomic update before publishing a higher count. The exact unclassified gap is represented by a
single stable `active_unclassified_recoverable_warning` aggregate with one diagnostic-log example;
specific phase aggregates replace it when available. Monotonic regression fails, persistence
failure suppresses the higher frame, and interruption/restart preserves exact accounting. No
protocol, Core/.NET, WPF, schema, diagnostic log, Performance tab, or later optimization changed.

`SOP3b-active-warning-page-snapshot` is accepted. Schema v15 adds one durable warning revision per
run. Structured aggregate replacement, active fallback change, cancelling, terminal completion, and
startup interruption advance it; warning rows, totals, exact accounted count, lifecycle state, and
revision are read from one SQLite snapshot. Opaque cursors bind the exact run, sort, revision, and
run status, so active mutation or terminal handoff returns `invalid_cursor` rather than mixing
pages. Restart reconstructs the exact interrupted snapshot. The additive worker response exposes
active/terminal/pending state and the client-configured bounded local diagnostic-log path as
supplemental developer/recovery metadata, never durable warning truth. No Core warning contract/
view model, WPF entry point, cache policy, SOP4, later optimization, or campaign changed.

`SOP3c-bounded-current-warning-view-model` is accepted. Core now projects the complete SOP3b page
identity and separate diagnostic-log metadata through one reusable active/completed drilldown.
Generation changes cancel and reject late requests; active and pending first-page refreshes bypass
cache; cursor pages must retain the exact accepted revision/state/status; and run/sort/revision
changes cannot exceed five cached pages or 25 bound rows. Exact durable accounted/live totals are
shown explicitly. Once terminal identity is accepted it cannot change or return to active, so a
late active page cannot replace immutable completed history. Existing Run history delegates its
paging to this component without changing XAML, focus/navigation behavior, or adding the Progress
entry point. No Rust, worker, schema, WPF, SOP4, campaign, or parked-stream work changed.

`SOP3d-accessible-warning-entry-and-acceptance` is accepted and closes SOP3. The Progress warnings
card exposes one count-aware Alt+W command; Shell carries the exact current run into Run history,
opens the existing bounded drilldown, and uses its accepted warning-grid focus request. The same
surface continues through terminal handoff and reconstructs interrupted terminal truth after a new
Shell instance. A system-brush diagnostic application-log block is bound only to the warning page's
supplemental metadata, separate from the shell recovery log, and explicitly says that it is not
durable warning truth. Loaded-STA evidence covers keyboard reachability, stable automation IDs and
names, focus, the 25-row UI bound, and latest-only `MostRecent` announcements. No Rust, worker,
schema, SOP4, campaign, or parked-stream work changed.

`SOP4-performance-tab` is accepted as three finite coherent packages. The worker exposes only a
25-run newest-first page and one fixed snapshot from a query-only status-database connection; SQL
owns latest/peak reduction, and raw host/device samples never cross the process boundary. Core hard-
caps 25 history, six phase, and 64 device rows, refreshes live from accepted progress/lifecycle,
preserves explicit unavailable values, and labels device/volume, input-signature, or software-build
differences before showing a selected retained comparison. The virtualized WPF tab uses native
keyboard controls, stable automation, system brushes, latest-only announcements, and preserves
operator focus during live updates. Restart reconstruction and representative 25/64-row history
are deterministic. Production execution remains disabled. No schema migration, SOP5+, physical or
representative campaign, or parked-stream work ran.

`SOP5-skip-singleton-size-buckets` is accepted as two finite coherent packages. Discovery now
classifies exact-size singleton buckets as metadata-resolved and admits only multi-file buckets to
the existing partial/full hash pipeline; the early return remains after the existing cancellation
check. A mixed injected-I/O fixture records no singleton path at either read seam while preserving
four non-singleton partial opens, two full-content opens, and the expected duplicate group. The
end-to-end fixture reconciles 2 files/41 bytes as singleton and metadata-resolved, 4 files/8,228
bytes as candidates and terminally hash-pipeline-resolved, and only 2,084 physical partial-read
bytes; live and durable terminal counters match exactly. Existing cache, cancellation, candidate
read-failure, hard-link, long-path, deterministic-result, and bounded-memory regressions pass. No
schema, protocol, WPF, scheduler, cache-policy, production lock, campaign, or parked stream changed.

`SOP6-device-aware-scheduler` is accepted as three finite coherent packages. Windows classifies a
candidate's drive volume to exactly one physical extent and the documented seek-penalty media class
without opening candidate content; drive aliases reuse one cached result, while remote, failed,
multi-extent, conflicting, or unavailable mappings share one conservative fallback queue. A fair
global dispatcher replaces every nested Rayon read in partial/full hashing, caps rotational and
unknown devices at one reader and SSDs at four within the host logical-parallelism ceiling, lets
separate devices progress independently, and removes queued work before content open on
cancellation. Retained direct/unbuffered 1 GiB arms in fixed 1/N/N/1 order preserve exact checksums
and process I/O. Two rotational readers were 4.2593% slower with 4.0830% lower throughput and about
double latency, selecting one; four SSD readers reduced wall time 48.9399% and raised throughput
85.4809%, selecting four. No read-path/locality/buffer/prefix change, schema, protocol, WPF,
cache-policy, production lock, SOP7+, unrelated campaign, or parked-stream work changed.

`SOP7a-read-experiment-contract` is accepted as the first of six finite SOP7 packages. One test-only
platform-neutral harness fixes control/treatment/treatment/control order, changes exactly one of
path locality, bucket order, buffer size, sequential read-ahead, or partial-prefix reuse per
comparison, and preserves SOP6's selected one/four/one reader ceilings. Four distinct deterministic
arm fixtures prevent same-path reuse; evidence includes input/build/device identity, complete host/
process/device cost, exact ordered checksums and bytes, cancellation, and cleanup, and fails closed
before write when malformed or when its output already exists. Production hash defaults and the
accepted SOP6 source/evidence hashes are unchanged. No physical profile, SOP7b+, SOP8, Windows UI,
production lock, unrelated campaign, or parked-stream work changed.

`SOP7b-path-locality` is accepted with no product change. Both representative roots retained every
1 GiB arm in fixed control/treatment/treatment/control order with identical checksums, process-read
bytes, and cleanup. The just-created SSD fixtures remained cache-resident, so the recorded device
unavailability is explicit and no physical-SSD claim is made; deterministic parent/path order was
11.5698% slower in wall time and reduced throughput 10.4500% in that cached workload. The corrected
direct/unbuffered rotational fixture made allocation locality match parent paths and recorded all
device counters; treatment improved wall only 1.2075% and throughput 1.2204%. That small idealized
gain does not justify changing buffered production insertion order. No sample was discarded or
rerun, no production source changed, and bucket order, buffer/read-ahead, prefix reuse, SOP8+, and
the parked stream remain untouched.

`SOP7c-bucket-ordering` is accepted and changes production to descending exact-size bucket
admission. All retained SSD/HDD direct-read arms preserve within-bucket order, SOP6 reader ceilings,
64 KiB reads, no read-ahead/prefix reuse, exact checksums, 1,073,856,512 process/physical bytes, and
complete device sampling. Descending order improved SSD mean wall 3.4622% and throughput 3.9257%,
and rotational wall 8.4299% and throughput 9.1264%. The initial SSD invocation completed reads but
failed closed before evidence on a transient cleanup handle; its generated directory was validated
and removed, bounded cleanup retry was added, and only the corrected run was retained. No favorable
result retry or discarded retained sample occurred. A deterministic order test, the evolved SOP6
verifier, strict Clippy, and full Debug/Release Rust matrices pass. Buffer/read-ahead, prefix reuse,
SOP8+, Windows UI, and the parked stream remain untouched.

`SOP7d-buffer-read-ahead` remains accepted after a fixed-factor audit correction. Its immutable v1
evidence remains retained, while separately versioned v2 arms use accepted descending buckets and
the SSD sequential-hint arm uses the already selected 1 MiB buffer. The v2 1 MiB treatment improves
SSD wall 59.7138% and throughput 141.4003%; it is neutral-to-worse on HDD, so rotational/unknown
remain 64 KiB. The v2 sequential hint improves HDD wall 3.0205% and throughput 3.0315%. On SSD its
2.0602% wall mean conflicts with 1.5547% lower throughput and substantial arm drift, so the hint is
disabled for SSD and retained only for rotational/unknown. Full-hash tasks carry the already
accepted scheduler media class; cache semantics, reader counts, and partial length do not change.
Memory remains bounded to 4 MiB across four SSD readers or 64 KiB for one conservative reader, and
cancellation is checked at least once per selected buffer.

`SOP7e-partial-prefix-reuse` is accepted with no product prefix-reuse change. Two retained C/T/T/C
profiles use 2,048 cache-miss files per arm with one collision-heavy shared 1 KiB prefix and distinct
full content. Reuse saves exactly 2,097,152 physical bytes per arm with exact hashes. SSD wall and
throughput regress 21.2033% and 18.8068%. The HDD arm has sharply drifting wall samples and 8.0857%
lower mean throughput, so it does not establish improvement. Experimental state is bounded to 1 KiB
per reader and 4 KiB aggregate; production keeps zero cross-phase prefix state and avoids new
metadata-revalidation, cache-hit, error, and cancellation branches. SOP8+, Windows UI, and the
parked stream remain untouched.

`SOP7f-read-path-acceptance` is accepted and closes SOP7. The named
`Verify-WindowsHashReadPath.ps1` verifier pins all 19 retained SOP7 raw/policy files, every factor
decision, descending bucket order, selected media buffers/read hints, zero production prefix state,
unchanged SOP6 reader ceilings, exact hash/counter/cancellation/memory regressions, and production
deletion locks. Focused strict Core Clippy passes with documented pre-existing allowances. Debug
and Release Rust each pass 202 tests with 7 intentional profiles ignored. Debug and Release Windows
each build with zero warnings/errors and pass 145 Core, 74 Infrastructure, and 3 smoke tests with 5
operator-only Infrastructure skips. SOP8 implementation, unrelated physical campaigns, and the
parked stream did not run.

`SOP9c-single-drive-reference-repeat-v2-protocol` is accepted as a design-only recovery package.
The fixed new identity is `sop9c-single-drive-reference-repeat-v2` on E: with unchanged forced-then-
reuse arms and completed terminals. V2 changes only V1's demonstrated persistence quiet-frame
defect: it owns one pending stdout read, keeps 180 seconds outside persistence, probes only owned
database/WAL metadata every five seconds, fails after 15 minutes without state activity, and keeps
an absolute 24-hour persistence cap with at-most-ten-minute activity journaling. The design verifier
passes pure fake-time and temporary metadata-activity tests plus source/authority/safety checks.
At design acceptance, no build, physical preflight, worker, E: I/O, V2 evidence/state reservation,
V1 diagnostic mutation, SOP9d, or parked validation ran; the identity was then unconsumed. Its later
separately authorized disposition follows.

The sole separately authorized V2 invocation is now consumed and retained invalid. Its write-once
manifest and journal record one reservation, passed fixed E: preflight, scoped state creation, and
both pinned Release builds. The consuming campaign host then ended before `worker_started`, an arm,
or native evidence finalization; the surfaced guarded second admission correctly refused the
existing identity. The post-exit audit found no worker and no scoped V2 state. The path-free recovered
incident records zero scan/result/measurement truth and does not invent a product, runner, watchdog,
or cleanup cause. V1/V2 cannot be rerun; a successor design requires a separately reviewed causal
defect plus explicit operator authority, and SOP9d/SOP9e remain dependency-blocked.

The subsequent operator-boundary causal review inspected the immutable manifest, four-event
journal, both build logs, post-exit process/state observations, and the runner transition from
`build_ready` to worker start. No retained `attempt_failed`, worker-start, arm, terminal, result,
measurement, or native finalization/cleanup record exists. The host interruption cannot establish
why execution ended and is not itself a causal defect. No successor protocol or identity is
justified or authorized; SOP9c remains `blocked_invalid_campaign`.

The SOP8 boundary audit was completed before implementation. The current lazy global
RocksDB value is an unversioned full hash keyed by canonical path/size/nanosecond modified time;
partial reads always repeat, non-colliding files never enter the cache, renames miss, capacity is
unbounded, and decoded hits lack a post-lookup signature check. Discovery already de-duplicates
hard-link aliases by stable physical identity. Five finite packages now order: an owned versioned
1,500,000-entry bounded store; fail-closed physical identity/content-change qualification; partial
plus full pipeline/telemetry integration; one write-once 512 MiB Release policy comparison; and the
selected-default accessible session control plus final verifier. Legacy/corrupt/unsupported/coarse
evidence always falls back to reads. SOP6 reader ceilings, SOP7 read policy, exact results, hard
links, cancellation, memory/telemetry guarantees, retained cache evidence, and deletion locks stay
fixed.

`SOP8a-versioned-bounded-cache-store` is accepted. A new owned/reopenable v2 RocksDB contract
defines closed `reuse_verified`/`revalidate_content` policies with the safe forced-read default,
bounded physical identity/change-token keys, partial plus optional full hashes, exact replay and
conflict rejection, and atomic entry/order/count/sequence writes. It caps live v2 entries at
1,500,000 and prunes the deterministic oldest set to 1,350,000 using at most 1,024 keys per batch.
Open repairs missing order/count/sequence state, removes orphan/malformed indices, preserves legacy
keys without admitting them as hits, and makes corrupt v2 entries ineligible/prunable. Production
still uses the unchanged legacy lookup; no read path, counters, worker/Core/WPF contract, schema,
profile, campaign, or deletion lock changed.

`SOP8b-qualified-content-signature` is accepted without production cache-hit integration. The
platform-neutral seam requires stable physical identity, byte length, positive non-coarse
nanosecond modified time, and a bounded content-change token; any missing/error/coarse/invalid field
is explicitly ineligible, and before/after observations must be identical. Windows gathers all
fields through a metadata-only `FILE_READ_ATTRIBUTES` handle using volume/file index, size,
LastWriteTime, and ChangeTime. The real fixture proves identity survives rename and same-size edit,
while ChangeTime advances for both. Windows rename reuse is deliberately rejected rather than
weakening invalidation; the pipeline will read current content. Upstream hard-link alias de-
duplication remains unchanged.

`SOP8c-partial-full-cache-integration` is accepted with forced revalidation still the provisional
default. Each scan owns the versioned bounded store. The partial stage either performs a qualified
before/after hit or reads the accepted 1 KiB prefix and seeds it; the full stage receives that exact
verified partial signature, so a mutation between stages cannot bind an old prefix hash to new
content. Full reads retain SOP7's media buffer/hint policy and no-prefix-reuse decision. Cache open,
lookup, corrupt-entry, and store failures remain recoverable read fallbacks with bounded warnings;
parallel stores serialize only entry-count/order metadata and do not change SOP6 reader ceilings.
Metrics contract v3 adds partial hit/miss/error/store counters, all 47 fixed counters serialize
through the worker and are strictly parsed/validated by Core, and cache-hit rates combine both
stages without relabeling logical work as physical bytes. Exact duplicate, hard-link, cancellation,
bounded-memory, telemetry, and deletion-lock behavior is unchanged. No SOP8 measurement or UI ran.

`SOP8d-repeat-policy-measurement` is accepted from its sole write-once Release invocation. The
retained 512 MiB local SSD artifact preserves identical 64-group/128-item results across the fixed
forced/reuse/reuse/forced order. Both same-process and reopened-store reuse arms record 128 partial
and 128 full hits, zero process reads, and exact savings of 131,072 partial plus 536,870,912 full
bytes. Bracketing forced arms record the expected 537,001,984 process-read bytes. Forced median/tail
wall time is 120.83595/144.8017 ms versus reuse 12.63885/13.2765 ms, an 89.54% median improvement.
All samples, build/input/hashed-host/media/store/signature/policy identity, CPU/memory/process-I/O,
explicit unavailable device values, cancellation, and successful cleanup are retained; no retry
occurred. `reuse_verified` was selected for SOP8e.

`SOP8e-session-policy-and-acceptance` is accepted and closes SOP8. The selected
`reuse_verified` default and explicit `revalidate_content` alternate now cross additive worker/Core
contracts and are persisted in immutable run JSON; execution reads that snapshot. New omitted start
values default to reuse, while old snapshots missing the field reconstruct as forced reads to retain
their historical truth. Setup exposes one native keyboard-focusable choice with an access key,
automation name/help, system-theme behavior, and concise verification/fallback disclosure. The
selection is captured per start generation; session changes and disposal cancel it, and stale
completions/failures publish nothing. `Verify-WindowsRepeatCache.ps1` pins the retained artifact,
v2 store bounds, signature, exact hits/reads/results, 89.54% selection, SOP6/SOP7 policies,
cache/telemetry/correctness/cancellation/cloud/hard-link/memory regressions, and every production
lock. No product schema migration, SOP9 work, unrelated physical campaign, or parked validation ran.

An earlier implementation slice accepts `SOP2c-worker-progress-projection`. The worker now reduces
the accepted typed observations and projects the complete additive snapshot while retaining the
protocol-v1 legacy fields. One timer-owned latest-value emitter assigns transport sequences after
coalescing, admits at most one frame per 100 ms, latches cancellation, and is closed and joined
before terminal publication. Every byte and byte-rate quantity is a decimal string; legacy
`filesHashed` is explicitly deprecated as partial-hash success, and legacy discovery continues to
exclude zero-byte files while typed counters include them. The producer has no trustworthy device
association and therefore keeps `mapping_unavailable`. A narrow SOP2b follow-up publishes typed
discovery observations at the existing 256-file callback. No Core/.NET/WPF, schema, scheduling,
singleton, cache-policy, profile, physical campaign, or parked-stream work changed.

The preceding implementation slice accepts `SOP2b-incremental-pipeline-publication`. One serialized
engine sink now consumes cumulative hasher deltas without per-file status writes. Partial work is
batched at 256 file outcomes and full-read bytes at 8 MiB, so progress advances inside a large size
bucket and long content read while preserving the existing Rayon scheduling and singleton reads.
Observed cache and streaming-read seams retain actual bytes through failure/cancellation and
separate all failed full-hash requests from the narrower content-read-failure metric. Completed,
failed, and cancelled terminal live counters reconcile exactly with durable metrics v2. Progress
contract v1 now has ten supplemental logical counters; no durable schema migration was introduced.
No worker protocol, .NET/WPF, scheduling, singleton, repeat-cache policy, profile, physical campaign,
or parked-stream work changed.

The preceding implementation slice accepts `SOP2a-progress-contract-reducer`. Progress contract v1
keeps the supplemental logical-work meanings separate from durable metrics v2, preserves physical-I/O
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

Stop before any further physical campaign. The sole authorized
`sop9c-single-drive-reference-repeat-v2` identity is consumed and invalid after a pre-worker campaign-
host interruption. Do not rerun/overwrite V1/V2, delete or mutate V1 diagnostic state or retained V2
evidence, consume SOP9d, or substitute another drive. A successor protocol and identity require a
separately reviewed causal defect; the completed retained-evidence review establishes none. Preserve
the blocker unless new causal evidence is supplied. Only then could the operator separately approve
a versioned successor design, and any later physical invocation would require another distinct
approval. SOP2's waived strict representative overhead gate remains unevaluated and must not be
revived.

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

- `SOP9` in `docs/scan-optimization-plan.md`; SOP9 also owns the unresolved SOP2
  representative-overhead risk because the strict <1% wall/CPU gate was waived unmeasured
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

At clean `4866ab5`, `Verify-WindowsLargeDriveSingleDriveV2Invalid.ps1` passes again. The read-only
causal review also confirms that the immutable four-event journal ends at `build_ready`, the next
runner source transition is worker start, and no retained fact identifies why the campaign host
ended. This establishes no causal product, runner, watchdog, cleanup, or campaign defect and leaves
SOP9c `blocked_invalid_campaign` without successor authority.

The consumed invalid V2 attempt passes `Verify-WindowsLargeDriveSingleDriveV2Invalid.ps1`. The
verifier pins the manifest, four-event append-only journal, and both Release build logs by exact
SHA-256. It proves one reservation; fixed E: identity; forced/reuse order; completed-terminal and
180-second/5-second/15-minute/24-hour watchdog contract; passed preflight; scoped state creation; and
build-ready hashes, followed by no worker, arm, terminal, native acceptance evidence, scan result, or
measurement. It also checks the post-exit absence of the scoped V2 state and worker, the guarded
second admission, consumed/no-retry authority, honestly unevaluated SOP2 risk, SOP6/SOP7/SOP8
policies, and every production lock. The recovered record does not attribute an unproved product,
runner, watchdog, or cleanup cause. SOP9c is `blocked_invalid_campaign`; SOP9d/SOP9e remain blocked.

Accepted `SOP9c-single-drive-reference-repeat-v2-protocol` passes
`Verify-WindowsLargeDriveSingleDriveV2Protocol.ps1`. The verifier parses the runner, pure watchdog
helper, and itself; executes only the fake-time controller and source-only V2 description; and uses
a disposable temporary database/WAL metadata fixture to prove activity renewal. It pins the new
unconsumed E: identity, forced/reuse order, completed terminal requirement, exactly one stdout read
site/pending read, the unchanged 180-second non-persistence deadline, five-second probes, 15-minute
idle bound, 24-hour absolute cap, ten-minute journal bound, and explicit idle/absolute failure
reasons. Naming V2 without the separately authorized `-RunPhysicalCampaign` switch fails before
evidence/state access. The verifier also pins the immutable V1 incident summary, separate invocation
authority, SOP2 residual-risk truth, and every production deletion lock. No build, hardware
preflight, worker,
E: I/O, V2 evidence/state creation, V1 diagnostic-state access or mutation, SOP9d, or parked
validation ran.

The consumed `sop9c-single-drive-reference-repeat-v1` attempt is retained as invalid and passes
`Verify-WindowsLargeDriveSingleDriveInvalid.ps1`. The verifier pins the manifest, append-only
journal, invalid evidence, build logs, worker diagnostic log, and path-free
[`scan-large-drive-single-drive-invalid-20260828.json`](evidence/scan-large-drive-single-drive-invalid-20260828.json)
summary by exact SHA-256. Its one `revalidate_content` arm retained 20,554 progress frames,
583,624,783 partial-read bytes, and 256,449,196,445 full-read bytes before entering `persisting`.
Exactly 180,033.3426 ms after the last frame, V1's fixed deadline declared a protocol timeout while
the product WAL was still advancing; cleanup then waited once for 30,185.196 ms, force-stopped the
worker, recorded exit code -1, skipped state removal, and preserved the owned product/status/cache
state for diagnostics. There is no worker terminal event, completed/measured arm, result digest,
reuse arm, or favorable retry. The verifier also pins the honestly unevaluated SOP2 gate, SOP6
reader ceilings, SOP7 read policy, SOP8 new/legacy policy truth, and every production deletion lock.
V1 remains an immutable `blocked_invalid_campaign` incident; its separately reviewed V2 successor
was later accepted and consumed invalid as described above. SOP9d/SOP9e are dependency-blocked.

Accepted `SOP9b-representative-cancellation` passes
`Verify-WindowsLargeDriveCancellation.ps1`. The sole fixed D: physical identity is consumed; its
manifest, append-only journal, terminal evidence, and path-free committed summary are pinned by
exact SHA-256 and were not retried. The `revalidate_content` arm discovered 2,727,118 logical files
and 11,698,178,946,036 bytes, classified 363,432 singleton files without content reads and 180 hard-
link aliases outside recoverable copies, then accepted 256 partial reads/262,144 bytes before the
runner requested cancellation. Terminal `cancelled` followed 2,408.5771 ms later; no progress
followed terminal and empty result digests prove no partial duplicate result was represented as
completed. Nine product/status warnings, the complete file/byte funnel, one cancelled queued item,
and zero telemetry loss/flush errors reconcile. All 166 host and device samples remain within the
100,000/64 retention caps, five unavailable counter observations remain explicit, and peak observed
working set/private bytes are 3,290,181,632/3,713,601,536. The bundle retains 5,881 progress frames,
18,996,432 serialized bytes, status commit sequence 169, 806,912 status-database bytes, and
54,476,916 process-write bytes as non-causal SOP2-scale proxies; without an observer-off
counterfactual, the strict `<1%` gate remains unevaluated and did not pass. Cleanup stopped the owned
worker without force and removed validated campaign state. The verifier also pins SOP6 reader
ceilings, SOP7 read policy, SOP8 new/legacy policy truth, and every production deletion lock.

Accepted `SOP9a-write-once-campaign-tooling` passes
`Verify-WindowsLargeDriveAcceptanceTooling.ps1`. The verifier revalidates the three absent physical
campaign identities and D:/E:/H: hardware without starting them, then runs one temporary forced/
reuse fixture with identical file/folder digests, 2 singleton metadata resolutions, one hard-link
alias, 4 partial plus 4 full reuse hits, zero warnings, at most 3 observed frames/second, retained
commit/database/process-write proxies, and exact scoped cleanup. A tooling-only injected failure
retains its manifest, append-only failure journal, invalid evidence, and diagnostic state. Eight
historical evidence hashes, legacy forced reconstruction, singleton zero-open, closed worker policy,
query-only helper compilation, strict example Clippy with four unchanged-source allowances,
PowerShell parsing, diff checks, and all production locks pass. Terminal status replay rows remain
zero by the accepted retention policy and are not misreported as missing observation. No
representative run, SOP2f campaign, parked validation, or deletion capability ran.

The SOP9 package-definition slice is documentation-only. The complete startup audit found a clean
`wpf-poc` worktree at `29a392c`, preserved SOP8 as accepted, inspected its closing diff and all
retained scan evidence/tooling, and found no active Super Duper process. The sandboxed storage
inventory retained access-denied results; an approved read-only desktop-context inventory mapped
D: and E: to distinct healthy 14 TB SATA HDDs and H: to a separate SSD with sufficient isolated-
state space. The plan now fixes five dependency-ordered packages with explicit correctness,
measurement, failure-retention, cleanup, and acceptance criteria. Documentation links/checkpoints,
`git diff --check`, and a clean focused commit are the proportional checks. No build, product code,
representative run, SOP2f campaign, parked validation, or deletion capability runs.

Accepted `SOP8e-session-policy-and-acceptance` passes the final
`Verify-WindowsRepeatCache.ps1`. The verifier pins the 14,552-byte evidence artifact at SHA-256
`7dfc8aa6b44dd00f9e307197b4ca82a4b6b2e28888c3f236b9bd8281a9569dc8`, all exact measurement
outcomes, v2 store/signature/immutable-policy contracts, SOP6/SOP7 decisions, telemetry,
correctness, cancellation, cloud, hard-link, bounded-memory regressions, and production locks.
Focused worker/Core/loaded-STA tests prove closed-value rejection, measured default, forced
alternate, exact new/legacy history, generation-scoped stale silence, and accessible disclosure.
Strict Core/worker Clippy passes with the eight documented unchanged lint allowances. Full Debug
and Release Rust each pass 221 tests with 8 intentional operator profiles ignored. Windows Debug
and Release each build with zero warnings/errors and pass 149 Core, 74 Infrastructure, and 3
loaded-STA smoke tests with 5 operator-only Infrastructure skips. The first verifier attempt stopped
before matrices on two stale over-broad SOP6 source pins and was corrected to retain immutable
evidence plus behavioral mapping/scheduler checks. A later Release Windows attempt used a stale
normal worker because `cargo test --release` does not build that executable; the verifier now builds
each Rust workspace explicitly before its paired Windows matrix, and the complete rerun passed.
No product fix or favorable physical retry was involved.

Accepted `SOP8d-repeat-policy-measurement` retains one valid write-once Release artifact at
`docs/evidence/scan-repeat-cache-policy-20260827.json` from build `a8eda4d39421f287158114244143d40b56e595f4`.
The profiler test passed in 2.51 seconds after its clean Release build; 128 fixed files/64 exact
pairs totaled 536,870,912 bytes, same-process and reopened reuse saved every declared read with
identical results, the median wall improvement was 89.54%, cancellation passed, and the fixture/
cache were removed. The evidence is 14,552 bytes, status `valid`, records zero favorable retries,
and selects `reuse_verified`. No Windows UI/session/default or deletion-lock change accompanied the
measurement.

Accepted `SOP8c-partial-full-cache-integration` passes focused production-path tests for exact
forced partial/full stores and physical bytes, same-process and reopened-store qualified hits with
zero physical bytes, exact duplicate-key equality, edits and between-stage mutation rejection,
concurrent count/order integrity, cache/read/error reconciliation, and the existing cancellation,
SOP6 reader, SOP7 read-policy, hard-link, and exact-result regressions. Metrics contract v3 exposes
47 fixed counters; focused defensive parser tests and the real-worker lifecycle pass. The complete
Debug and serial Release Rust matrices each pass 217 tests with 7 intentional profiles ignored;
strict Core/worker Clippy passes with eight explicitly allowed unchanged-file lint classes. The
first concurrent Release attempt observed one instead of two heartbeat samples in the existing
timing test; the clean serial matrix passed it without a product change. The Debug Windows solution
passes 145 Core, 74 Infrastructure, and 3 loaded-STA smoke tests with 5 operator-only skips. The
Windows build is warning-free and `git diff --check` passes. Forced revalidation remains default;
no SOP8d evidence, UI/session contract, product schema, SOP9 campaign, parked validation, or
production deletion lock changed.

Accepted `SOP8b-qualified-content-signature` passes 3 focused injected qualification/window tests,
the real Windows metadata-only identity/rename/edit test, and the full Core library (51 passed, 2
intentional physical profiles ignored). Strict Core Clippy passes with the documented unchanged-
file allowances. Coverage proves unchanged alias acceptance only when all fields match, preserved-
modified-time token rejection, identity-change rejection, and explicit metadata/identity/time/
coarse/token/invalid ineligibility. Windows rename preserves identity but changes the selected
token, so it must miss. `git diff --check` passes; production cache/hash I/O, counters, protocol/UI,
schema, measurement, campaigns, and deletion locks remain unchanged.

Accepted `SOP8a-versioned-bounded-cache-store` passes 7 focused store tests and the full Core
library (47 passed, 2 intentional physical profiles ignored). Strict Core Clippy passes with the
four previously documented allowances plus two unchanged integration-test instances of the newly
reported `needless_borrows_for_generic_args` lint. Coverage proves closed policy parsing and forced
default, create/reopen/replay/conflict, newer-schema rejection without modification, bounded
identity/token/key/value/order encodings, deterministic oldest pruning, corrupt-entry fallback and
pruning, atomic metadata/order recovery, and untouched legacy keys. `git diff --check` passes.
Production hash lookup remains unchanged and no physical/profile/UI/worker/schema/campaign ran.

The SOP8 package-definition slice is documentation-only. The complete startup and latest-commit
audit found a clean `wpf-poc` worktree at `88b1ffa`, with the scan plan/handoff agreeing on SOP8 and
`ROADMAP.md` carrying a stale SOP7 boundary that this slice repairs. Cache source/call-site tests,
run/session contracts, stable hard-link identity, telemetry counters, and every production lock
were inspected. Markdown links, package/gate consistency, `git diff --check`, and a clean commit are
the proportional acceptance checks; no product test, cache mutation, profile, physical campaign,
SOP9, or parked-stream work runs.

Accepted `SOP7f-read-path-acceptance` passes `Verify-WindowsHashReadPath.ps1`, including the nested
SOP6 verifier and every production lock. All 19 retained SOP7 evidence/policy hashes and decisions
are pinned. Debug and Release Rust each pass 202 tests with 7 intentional profiles ignored. Debug
and Release Windows each build with zero warnings/errors and pass 145 Core, 74 Infrastructure, and
3 smoke tests with 5 operator-only Infrastructure skips. Focused strict Core Clippy passes with
documented pre-existing allowances. The accepted product policy is encountered path order,
descending exact-size buckets, 1 MiB/no sequential hint on SSD, 64 KiB/sequential hint on
rotational/unknown, and no partial-prefix reuse.

Accepted `SOP7d-buffer-read-ahead` retains both immutable v1 records and four corrected v2 raw
buffer/sequential SSD/HDD files plus a v2 policy record. All v2 arms use descending buckets, read
exactly 1,073,856,512 bytes with identical hashes, complete device samples, direct/write-through
fixture preparation, and cleanup. The selected policy is 1 MiB/no hint on SSD and 64 KiB/Windows
sequential hint on rotational/unknown. Focused read-path/hash tests and strict Core Clippy pass; the
full acceptance matrix belongs to SOP7f.

Accepted `SOP7e-partial-prefix-reuse` retains two raw SSD/HDD profiles and one policy record. Each
arm has 2,048 collision-heavy cache-miss files; reuse saves exactly 1 KiB per file/2 MiB per arm and
preserves all hashes. SSD regresses and HDD throughput regresses amid strong arm-order drift, so the
product change is rejected and production retains zero prefix state. Eight deterministic harness
tests pass with one physical profile ignored.

Accepted `SOP7c-bucket-ordering` retains
`scan-read-path-bucket-order-{ssd,hdd}-20260826.json` plus its policy record. All eight direct 1 GiB
arms have identical ordered hashes/bytes, fixed C/T/T/C order, available device metrics, no
cancellation, and removed fixtures. Descending order improves SSD wall 3.4622%/throughput 3.9257%
and rotational wall 8.4299%/throughput 9.1264%. One pre-evidence cleanup failure wrote no result;
bounded cleanup and exact generated-fixture removal pass before the retained run. The focused order
and six harness tests, evolved SOP6 verifier, and strict Core Clippy pass. Full Debug and Release
Rust each pass 199 tests with 7 intentional profiles ignored and no failures.

Accepted `SOP7b-path-locality` retains
`scan-read-path-locality-{ssd,hdd}-20260826.json` plus its policy record. All eight 1 GiB arms have
identical ordered hashes, exact physical/process bytes, fixed C/T/T/C order, no cancellation, and
removed fixtures. SSD treatment regressed wall 11.5698% in the explicitly cache-resident workload;
rotational direct-read treatment improved wall only 1.2075% with all disk metrics available. The
product change is rejected, so no production correctness/regression matrix was required. Five
deterministic harness tests and strict Core Clippy with four documented allowances pass; cleanup
finds zero scoped fixtures/processes.

Accepted `SOP7a-read-experiment-contract` passes 5 focused deterministic read-path tests with the
single physical profiler intentionally ignored. Coverage proves exact one-factor isolation for all
five comparisons, stable task identity under bucket/path ordering, identical hashes and exact 1 KiB
prefix byte savings, cancellation before content open, fixed 1 MiB per-reader/1 KiB prefix bounds,
write-once output, malformed-evidence rejection, and use of SOP6's selected device reader ceilings.
Strict Core Clippy passes with the four documented pre-existing lint allowances. No physical
profile or product read default ran or changed; the SOP6 pinned source/evidence hashes remain
unchanged.

Accepted `SOP6-device-aware-scheduler` passes `Verify-WindowsDeviceScheduler.ps1`. Deterministic
tests prove one-reader rotational/unknown queues, four-reader SSD queues, conservative conflict/
mapping fallback, a global host-parallelism ceiling, cross-device independence, and cancellation
before queued content work. Windows fake-probe tests cover drive aliases, one physical extent,
multi-extent/remote/unavailable fallback, and seek-penalty classification. The injected hash
pipeline holds each of two rotational devices at one read while overlapping them and reconciles 8
partial attempts, 8 full requests/completions, 8 resolved files, and the exact duplicate result.
Write-once evidence in `docs/evidence/scan-device-scheduler-{ssd,hdd}-20260826.json` and its policy
record retains four 1 GiB direct/unbuffered arms per device in 1/N/N/1 order, exact checksums and
process read totals, CPU/memory, and complete disk throughput/IOPS/latency/active/queue samples. The
rotational comparison selects one reader; the SSD comparison selects four. Full Debug and Release
Rust each pass 192 tests with 6 intentional profiles ignored. Debug and Release Windows builds each
report 0 warnings/errors and each matrix passes 145 Core, 74 Infrastructure, and 3 smoke tests with
5 operator-only Infrastructure tests skipped. The first sandboxed Debug build was blocked before
the matrix by installed-SDK access; the unchanged desktop-context Debug and Release matrices pass.
Strict Core Clippy passes with four documented pre-existing lint allowances. Production locks and
`git diff --check` pass. No SOP7+, unrelated physical campaign, or parked
release-validation work ran. SOP2 remains `accepted_with_operator_waiver`; its strict <1%
representative wall/CPU gate remains unevaluated, not passed.

Accepted `SOP5-skip-singleton-size-buckets` passes the exact injected read-seam test and the full
Core target. The mixed seam observes 0 singleton partial/full opens, 4 non-singleton partial opens,
2 full-content opens, 4/12,288 candidate files/bytes resolved, and the unchanged duplicate result.
The end-to-end scan records 2/41 singleton and metadata-resolved files/bytes, 4/8,228 candidate and
duplicate-candidate files/bytes, 4 partial attempts, 2,084 actual partial-read bytes, exact terminal
candidate/resolved equality, and every live/durable counter equal. The complete Core target passes
124 tests with 5 intentional profiles ignored. Full Debug and Release Rust each pass 182 tests with
the same 5 ignored; Debug and Release Windows builds each report 0 warnings/errors and each test
matrix passes 145 Core, 74 Infrastructure, and 3 smoke tests with the same 5 operator-only skips.
`RecycleOperationViewModel.CanSubmit` remains false, production still injects
`DisabledRecycleOperationCapabilityExecutor`, worker responses remain `executorEnabled:false`, no
**Move to Recycle Bin now** action exists, and `git diff --check` passes. No representative or
physical campaign, SOP6+, SOP2f-v1/v2/v3 work, or parked release-validation work ran. SOP2 remains
`accepted_with_operator_waiver`; its strict <1% representative wall/CPU gate remains unevaluated,
not passed.

Accepted `SOP4-performance-tab` passes `Verify-WindowsPerformanceTab.ps1`: focused status-store,
worker restart/protocol, Core, typed real-worker client, and loaded-STA tests prove query-only fixed
summaries, 25/6/64 collection bounds, live/lifecycle refresh, explicit unavailable values, device/
input/build comparison labels, restart continuity, representative history, keyboard focus, stable
automation, system brushes, and `MostRecent` announcements. Full Debug and Release Rust matrices
each pass 181 tests with the same 5 operator profiles ignored. Full Debug and Release Windows
matrices each pass 145 Core, 74 Infrastructure, and 3 loaded-STA smoke tests with the same 5
operator-only Infrastructure tests skipped. XAML/PowerShell parsing, production-lock checks, and
`git diff --check` pass. The Rust matrix was run serially after the unchanged 120 ms heartbeat test
showed its known scheduler sensitivity under the initial parallel run; its four-sample threshold
and production cadence were not changed. No representative/physical campaign, SOP5+, or parked-
stream work ran. SOP2 remains `accepted_with_operator_waiver`; its strict <1% representative gate
remains unevaluated, not passed.

Accepted `SOP3d-accessible-warning-entry-and-acceptance` passes the named
`Verify-WindowsAccessibleWarningEntry.ps1` verifier: 7 focused Core tests and 1 loaded-STA WPF test.
The full Debug Windows solution passes 143 Core tests, 74 Infrastructure tests, and 3 loaded-STA
smoke tests; the same 5 operator-only Infrastructure tests are skipped. Coverage proves the exact
current run reaches History, 4 displayed occurrences equal 4 durable accounted warnings, page
metadata stays separate from the shell recovery-log path, active-to-interrupted terminal handoff
and new-Shell reconstruction, the 25-row/five-page bounds, Alt+W/automation/focus contracts,
system brushes, and latest-only `MostRecent` warning-page announcements. XAML/PowerShell parsing,
production-lock checks, and `git diff --check` pass. No Rust, worker, schema, representative or
physical campaign, SOP4, later optimization, or parked-stream work ran. SOP2 remains
`accepted_with_operator_waiver`; its strict <1% representative gate remains unevaluated, not passed.

Accepted `SOP3c-bounded-current-warning-view-model` passes 4 focused deterministic drilldown tests
and the complete Windows solution: 141 Core tests, 74 Infrastructure tests, and 3 loaded-STA smoke
tests pass; the same 5 operator-only Infrastructure tests are skipped. Coverage proves the 25-row
and five-page bounds, active first-page refresh across durable revisions, exact accounted/live
projection, generation cancellation and late-run silence, fail-closed mixed-revision paging,
restart reconstruction of interrupted terminal truth, and a one-way terminal latch that preserves
the accepted completed page. The existing completed-history paging/navigation tests remain green.
The solution build reports no warnings or errors, and diff checks pass. No Rust, worker, schema,
XAML/WPF entry point, physical/provider/performance campaign, SOP4, later optimization, or parked-
stream work ran.

Accepted `SOP3b-active-warning-page-snapshot` passes all 53 non-performance Core storage tests with
the 4 retained operator profiles ignored, all 25 worker library tests, and the focused typed
WorkerClient lifecycle test. Migration from schema v14 to v15 adds the revision exactly once;
focused coverage
proves atomic row/count/state/revision snapshots, advancing active revisions, stale-cursor rejection
after mutation, restart reconstruction as exact `interrupted` terminal truth, completed-run additive
compatibility, and available/unavailable diagnostic-log metadata. Strict focused Core/storage and
worker Clippy pass after allowing only the ten previously documented diagnostics across six
unchanged lint classes. The Infrastructure test build reports no warnings or errors; documentation,
evidence-hash, and diff checks pass. No Core warning view model, WPF, Performance tab, physical/
provider/mutation/performance campaign, later optimization, or parked-stream work ran.

Accepted `SOP3a-live-warning-accounting` passes the full Core storage target with 52 tests passed
and 4 retained operator performance profiles ignored, plus all 24 worker library tests. Focused
coverage proves one stable fallback row, exact count growth, monotonic regression rejection,
specific-aggregate replacement, a two-family exact active total, interruption/restart preservation,
warning-triggered persistence before event publication, and silence when durable accounting fails.
Strict Core/storage and worker Clippy pass after allowing only the ten previously documented
diagnostics across six unchanged lint classes. Only the three touched Rust files were formatted;
documentation/diff checks pass. No .NET/WPF, schema migration, worker query shape, physical/
provider/mutation/performance campaign, SOP4, later optimization, or parked-stream work ran.

Before authorization, the `SOP2f-representative-v2` package passed PowerShell parsing, protocol JSON
round-trip, exact revision/evidence-hash checks, fixture/order/budget arithmetic, disk/process gates,
and the executable no-state preflight at commit `ca4086f`. The sole approved command then completed
both Release worker builds, the status-probe build, exact fixture creation/validation, and the single
initial full-content conditioning pass. Control warmup 0 started but did not complete before the
predeclared arm deadline. A secondary `New-Evidence` empty-sum error at harness line 1234 prevented
native invalid-evidence serialization and replaced the exact primary exception. Post-exit audit
finds the named GUID campaign root absent and zero scoped worker/probe/product processes. A recovered
incident now occupies the write-once path, so executable preflight/run admission correctly refuses
reuse. No Rust/.NET/WPF product file or accepted full functional matrix changed or reran.

`SOP2f` functional verification passes. `Verify-WindowsScanProgress.ps1` passes 11 exact Rust tests,
5 strict Infrastructure parser tests, 27 Core application/projection tests, and 1 loaded-STA WPF
test. Full Debug and Release Rust each pass 175 tests with 5 ignored. Serialized Debug and isolated
Release Windows matrices each pass 137 Core, 74 Infrastructure, and 3 smoke tests with the same 5
operator-only Infrastructure skips; both real worker/WPF smokes pass the typed-progress/terminal UI
assertions. Debug builds and the isolated Release build/publish report zero warnings/errors. The
standard Release verifier passed optimized Rust but its normal .NET output copy was blocked by the
pre-existing visible Release app PID 48972; an isolated artifacts path supplied equivalent
build/test/publish evidence, and the operator later authorized the verified process to be stopped.
The retained short profile at `docs/evidence/scan-progress-overhead-20260825.json` passes the
operator-approved fixed 100 ms wall/125 ms CPU caps at +66.9243 ms and no detected positive CPU
cost. The v1 campaign's distinct pre-measurement failure remains
`docs/evidence/scan-progress-representative-premeasurement-20260825.json`. The v2 attempt is retained
at `docs/evidence/scan-progress-representative-overhead-sop2f-v2.json` as a recovered invalid-campaign
incident: zero arms completed, no measured aggregate exists, and the <1% gate was not evaluated.
The operator records SOP2f as `waived_by_operator_unmeasured` and SOP2 as
`accepted_with_operator_waiver`; the representative leg is not passed, and its residual overhead
risk is assigned to SOP9.

Accepted `SOP2e` passes the full Windows solution with 137 Core, 74 Infrastructure, and 3 loaded-STA
smoke tests; the same 5 operator-only provider/physical Infrastructure tests are skipped. The
surface renders exactly six bounded outcomes and separate candidate context, removes legacy
`Files hashed`, exposes phase elapsed plus explicit physical-rate windows/cache/device/remaining/
ETA text, and truthfully overrides stale active claims after cancelling or terminal lifecycle.
Core tests prove one/multiple/unavailable devices, all ETA reasons, explicit no-snapshot history,
monotonic cross-run UIA versions, five-second accepted-snapshot coalescing, rejected-frame silence,
and one announcement advance behind the 1,000-frame Shell burst. Loaded-STA evidence proves
narrow-width wrapping, system-theme/system-brush compatibility, stable IDs/names, read-only current
path, Alt+C cancellation/focus handoff, text-without-version silence, and latest-only queued
`MostRecent` notification delivery. Targeted formatting and `git diff --check` pass. No physical
high-contrast/provider/mutation campaign, Rust/worker/schema/profile change, warning drilldown,
Performance tab, cache policy, or parked release-validation gate ran.

Accepted `SOP2d` passes 10/10 latest-only application-gate tests, 5/5 strict JSON parser tests, and
the complete Core/Infrastructure suites. Deterministic coverage feeds 1,000 frames, proves at most
ten accepted applications per half-open second, preserves the latest snapshot behind one delayed
dispatcher closure, and rejects wrong-run, duplicate/out-of-order, regressing,
running-after-cancelling, post-terminal, run-reset, and disposed work. Contract and
projection tests cover exact JSON kinds/casing, canonical decimal-u64 fields, additive unknown
fields, all funnel/rate/cache/device/remaining-work/ETA variants, and explicit units/windows. After
rebuilding the paired worker, the full Windows solution passes 129 Core, 74 Infrastructure, and 3
loaded-STA smoke tests with 5 operator-only Infrastructure tests skipped. Targeted formatting and
`git diff --check` pass; the full formatter still reports one pre-existing whitespace defect in the
unchanged `PreferenceRulesViewModelTests.cs`. No Rust behavior, XAML surface, schema, Release
profile, physical/provider/mutation campaign, or parked release-validation gate changed.

Accepted `SOP2c` passes 22/22 focused worker tests plus the typed-discovery fixture. Deterministic
coverage proves latest-wins coalescing across 1,000 updates, the ten-per-half-open-second bound,
timer delivery without another callback, transport/source ordering, sticky cancellation including
an already-pending frame, terminal suppression, legacy zero-byte semantics, complete decimal-byte
serialization, and matching terminal ordering. The full Rust workspace passes 175 tests with 5
ignored and 0 failed. Strict Core library, focused Core test, and worker Clippy pass with warnings
denied after only documented pre-existing allowances. No .NET/WPF, schema, Release profile,
physical large-drive, provider, mutation, or release-validation campaign ran.

Accepted `SOP2b` passes 8/8 focused hasher tests, 8/8 progress-contract tests, and completed/failed/
cancelled engine reconciliation fixtures. The full Rust workspace passes 169 tests with 5 ignored
and 0 failed. Strict Core library and focused-test Clippy pass with warnings denied after only the
documented pre-existing unchanged-file allowances. Coverage includes mid-bucket and mid-read
publication, exact cache hit/miss/error/store/read-failure accounting, retained bytes and silence
after cancellation, all supplemental logical fields, invariant-valid ordered observations, and
every final live metrics-v2 counter matching status storage. No .NET, WPF, status migration,
physical large-drive, profile, provider, mutation, or release-validation campaign ran.

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
| 2026-08-28 | this session | Complete the operator-boundary causal review of the immutable V2 manifest, four-event journal, build logs, post-exit audit, and runner transition after `build_ready`; establish that the host interruption provides no causal product, runner, watchdog, cleanup, or campaign defect and cannot justify a successor. Preserve all evidence, policies, residual-risk truth, and locks. | Retain SOP9c as `blocked_invalid_campaign`. Only new causal evidence may return a separately versioned successor design for explicit operator approval; any later physical invocation requires another separate approval. Do not rerun V1/V2 or start SOP9d. |
| 2026-08-28 | this session | Consume and retain the sole authorized SOP9c V2 invocation as invalid without retry: preserve its write-once reservation, passed fixed E: preflight, scoped state creation, and pinned builds; record that the consuming host ended before worker start/native finalization, the guarded second admission refused reuse, and post-exit worker/state are absent. Pin zero scan/result/measurement truth without causal invention while preserving SOP2 and all production locks. | Stop physical work. Do not rerun V1/V2 or start SOP9d. A successor design/identity requires a separately reviewed causal defect and explicit operator design and execution authority; otherwise retain the blocker. |
| 2026-08-28 | this session | Accept the design-only SOP9c V2 protocol under new unconsumed identity `sop9c-single-drive-reference-repeat-v2`: retain one pending stdout read, keep the 180-second non-persistence bound, and use five-second owned database/WAL metadata probes with a 15-minute idle limit, 24-hour absolute persistence cap, and bounded journal. Pure/source-only verification preserves V1 diagnostics, SOP2 residual risk, and production locks without physical I/O. | Obtain separate explicit operator approval for exactly one V2 physical invocation. Do not execute it, rerun V1, mutate V1 diagnostics, consume SOP9d, or start parked validation without that authority. |
| 2026-08-28 | this session | Retain and pin the sole SOP9c V1 physical attempt as invalid without retry: its forced E: arm completed discovery/hashing, reached persistence with 20,554 frames and 256.449 GB full reads, then the V1 180-second frame deadline force-stopped the worker before terminal, preserved diagnostic state, and never started reuse. The path-free summary and verifier preserve zero-result/cleanup/SOP2/safety truth. | Stop physical work. Obtain explicit operator authority before designing a separately reviewed V2/new fixed single-drive identity; do not rerun V1, delete diagnostics, consume SOP9d, or start parked validation. |
| 2026-08-27 | this session | Accept the sole SOP9b physical cancellation attempt: retain the fixed D: identity without retry, cancel 2.409 seconds after measured hash-read work begins, reconcile 2.727 million logical files/11.698 TB plus singleton/hard-link/warning/status truth, publish no completed result or post-terminal progress, retain bounded resource/device and SOP2 proxies, and clean up exactly. | Consume only `sop9c-single-drive-reference-repeat-v1` on E:, then verify/document/commit it before SOP9d. |
| 2026-08-27 | this session | Accept SOP9a with a path-free query-only snapshot helper, manifest-before-run/append-only/write-once runner, deterministic valid forced/reuse and injected-failure evidence, exact cleanup, fixed physical preflight, retained historical hashes, focused Release correctness, and production locks. | Consume only `sop9b-representative-cancellation-v1` on D:, then verify/document/commit it before SOP9c. |
| 2026-08-27 | this session | Audit SOP9 dependencies, operator-evidence requirements, retained artifacts/tooling, unresolved SOP2 risk, and available hardware; define five fixed tooling/cancellation/single-drive/multi-drive/acceptance packages with write-once/no-favorable-retry retention and explicit correctness, measurement, cleanup, and acceptance criteria. | Implement only `SOP9a-write-once-campaign-tooling`; do not start representative D:/E: I/O until it accepts. |
| 2026-08-27 | this session | Accept SOP8e and close SOP8: carry measured-default `reuse_verified` plus forced alternate through immutable run truth, closed worker/Core contracts, a generation-scoped accessible Setup choice, and a named verifier pinning all retained evidence, prior read policies, regressions, full Debug/Release matrices, and production locks. | Stop before dependency-ready `SOP9-large-drive-acceptance`; it remains unaudited and not started. |
| 2026-08-27 | this session | Accept the sole SOP8d write-once Release comparison: both same-process and reopened-store reuse saved exact 128 KiB partial plus 512 MiB full/process reads, preserved 64 groups/128 items with zero warnings, improved median wall 89.54%, retained all samples and explicit unavailable values, passed cancellation/cleanup, and selected `reuse_verified` without retry. | Implement only `SOP8e-session-policy-and-acceptance`; production stays forced until it carries the selected default and alternate. |
| 2026-08-27 | this session | Predeclare the executable SOP8d Release profiler with a fixed 512 MiB/64-pair collision-heavy fixture, forced/reuse/reuse/forced order, same-process and reopened-store ownership, exact read/result/cache/cancellation/cleanup validation, complete host/process/device evidence, write-once invalid-or-valid output, and no retry. No physical arm ran. | Execute the profiler exactly once at its pinned output; retain success or failure and select the default from all samples without tuning. |
| 2026-08-27 | this session | Accept SOP8c: the owned bounded store now serves partial and full stages only across qualified before/after windows; a carried partial signature prevents between-stage cache poisoning, parallel store metadata is serialized, metrics v3 carries four partial-cache counters through worker/Core, and forced revalidation remains default with SOP6/SOP7/hard-link/cancellation/memory/deletion locks unchanged. | Execute only the write-once `SOP8d-repeat-policy-measurement`; do not begin UI/default shipping, SOP9, another campaign, or parked validation. |
| 2026-08-27 | this session | Accept SOP8b with a metadata-only fail-closed physical identity/size/modified-time/content-change signature and before/after equality; retain real Windows evidence that rename preserves identity but advances ChangeTime, so rename reuse is rejected and hard-link de-duplication stays upstream. | Implement only `SOP8c-partial-full-cache-integration`; keep measurement, UI, SOP9, and parked validation out of scope until it accepts. |
| 2026-08-27 | this session | Accept SOP8a with an owned/reopenable v2 store, safe forced default, fixed entry/encoding limits, deterministic 1,500,000-to-1,350,000 pruning, bounded recovery, exact replay/conflict, and corrupt/legacy ineligibility while leaving production hash lookup untouched. | Implement only `SOP8b-qualified-content-signature`; keep production hits, telemetry/protocol/UI, measurement, SOP9, and parked validation out of scope until it accepts. |
| 2026-08-27 | this session | Audit the existing full-only path/size/mtime global cache, repair the stale ROADMAP checkpoint, and define five finite SOP8 packages with explicit store/signature/integration/measurement/UI correctness and acceptance boundaries before implementation. | Implement only `SOP8a-versioned-bounded-cache-store`; keep later SOP8 packages, SOP9, unrelated campaigns, and parked release validation out of scope until its dependency accepts. |
| 2026-08-27 | this session | Accept SOP7f and close SOP7 with a named verifier pinning every retained artifact/decision, unchanged SOP6 reader ceilings and production locks, strict focused Clippy, and clean full Debug/Release Rust and Windows matrices. | Advance only to defining finite SOP8 packages; do not start SOP8 implementation, SOP9 campaigns, or parked release validation. |
| 2026-08-27 | this session | Correct SOP7d with separately versioned fixed-factor evidence, scope the sequential hint to rotational/unknown, and accept SOP7e with prefix reuse rejected after exact byte savings but SSD regression and incoherent HDD evidence. | Advance only to `SOP7f-read-path-acceptance`; keep SOP8+, unrelated campaigns, and parked release validation out of scope. |
| 2026-08-26 | this session | Accept `SOP7d-buffer-read-ahead`: use direct/write-through fixtures for physical cache-miss evidence, select 1 MiB only for SSD and 64 KiB for rotational/unknown, enable the Windows sequential hint across media, preserve exact hash/byte/cancellation/memory bounds, and pass serial Debug/Release Rust. This decision is retained historically and corrected by the 2026-08-27 v2 record. | Advance only to `SOP7e-partial-prefix-reuse`; keep SOP8+, unrelated campaigns, and parked release validation out of scope. |
| 2026-08-26 | this session | Accept `SOP7c-bucket-ordering`: retain complete direct SSD/HDD C/T/T/C evidence, select descending exact-size admission from 3.46%/8.43% wall improvements, add a deterministic order regression and bounded transient-handle cleanup, evolve the SOP6 historical-source check, and pass full Debug/Release Rust. | Advance only to `SOP7d-buffer-read-ahead`; keep prefix reuse, SOP8+, unrelated campaigns, and parked release validation out of scope. |
| 2026-08-26 | this session | Accept `SOP7b-path-locality` and reject the product change: retain all fixed SSD/HDD C/T/T/C arms, explicitly scope the SSD result to cache-resident reads, record the small 1.21% idealized rotational direct-read gain, preserve exact checksums/bytes/cleanup, and leave insertion order unchanged. | Advance only to `SOP7c-bucket-ordering`; keep buffer/read-ahead, prefix reuse, SOP8+, unrelated campaigns, and parked release validation out of scope. |
| 2026-08-26 | this session | Accept `SOP7a-read-experiment-contract`: add one test-only platform-neutral harness with fixed one-factor arms, SOP6 reader ceilings, distinct deterministic fixtures, complete write-once evidence, exact hash/byte/cancellation/memory checks, and no production-default change or physical run. | Advance only to `SOP7b-path-locality`; keep bucket order, buffer/read-ahead, prefix reuse, SOP8+, unrelated campaigns, and parked release validation out of scope until their dependencies accept. |
| 2026-08-26 | this session | Accept `SOP6-device-aware-scheduler`: map local Windows volumes to one physical disk/seek-penalty class without candidate-content access, replace nested hash reads with fair bounded per-device queues, select 1 rotational/unknown and 4 SSD readers from retained 1/N/N/1 direct-I/O evidence, and preserve exact cancellation/counters/results plus full regression and production locks. | Advance only to `SOP7-hash-read-path`; define its finite individually measured experiments and keep SOP8+, physical work outside SOP7 authority, and parked release validation out of scope. |
| 2026-08-26 | this session | Accept `SOP5-skip-singleton-size-buckets`: classify exact-size singleton files/bytes as metadata-resolved before the content-I/O seam, admit only multi-file buckets to partial/full hashing, prove zero singleton and unchanged non-singleton reads/results through the injected seam, and retain exact live/durable/physical-I/O truth plus full regression and production-lock evidence. | Advance only to `SOP6-device-aware-scheduler`; define its smallest finite packages before implementation and keep SOP7+, unauthorized representative/physical campaigns, and parked release validation out of scope. |
| 2026-08-26 | this session | Accept `SOP4-performance-tab`: add query-only 25-run and fixed one-run summaries, bounded live Core projection and device/input/build-qualified comparison, and a virtualized keyboard/automation/system-brush Performance tab with explicit unavailable, focus, restart, representative-history, and latest-only announcement evidence. | Implement only `SOP5-skip-singleton-size-buckets`; keep SOP6+, representative/physical campaigns, and parked release validation out of scope. |
| 2026-08-26 | this session | Accept `SOP3d-accessible-warning-entry-and-acceptance` and close SOP3: add one count-aware Alt+W Progress entry into the existing 25-row/five-page History drilldown, preserve warning-grid focus and terminal/restart truth, disclose page-owned diagnostic application-log metadata separately from durable warnings, and retain system-brush/stable-automation/latest-only announcement behavior. | Implement only `SOP4-performance-tab`; keep SOP5+, representative/physical campaigns, and parked release validation out of scope. |
| 2026-08-26 | this session | Accept `SOP3c-bounded-current-warning-view-model`: project the complete durable warning snapshot in one reusable Core active/completed drilldown with generation cancellation, revision-aware five-page caching, 25-row binding, exact accounted/live status, restart reconstruction, and an immutable one-way terminal latch; delegate completed Run history paging without a WPF change. | Implement only `SOP3d-accessible-warning-entry-and-acceptance`; keep SOP4, later optimization, campaigns, and parked release validation out of scope. |
| 2026-08-26 | this session | Accept `SOP3b-active-warning-page-snapshot`: add schema-v15 durable warning revisions, one-snapshot page truth, revision/status-bound cursors, active-mutation and terminal-handoff stale rejection, restart reconstruction, and separate client-configured diagnostic-log metadata. | Implement only `SOP3c-bounded-current-warning-view-model`; keep the WPF/Progress entry, SOP4, later optimization, campaigns, and parked stream out of scope. |
| 2026-08-26 | this session | Accept `SOP3a-live-warning-accounting`: atomically account any higher accepted live warning count with one stable bounded fallback row before publication, fail closed on persistence error, replace the gap with specific phase aggregates, and preserve exact interrupted/restart truth without per-occurrence rows. | Implement only `SOP3b-active-warning-page-snapshot`; keep Core/WPF entry, SOP4, later optimization, representative campaigns, and the parked stream out of scope. |
| 2026-08-26 | this session | Record SOP2f as `waived_by_operator_unmeasured` and SOP2 as `accepted_with_operator_waiver`, citing accepted SOP2a-SOP2e packages, passing functional and fixed-cost short evidence, and both retained invalid representative attempts. The strict <1% wall/CPU gate remains unevaluated, not passed. | Carry the unresolved representative-overhead risk to SOP9. Advance only to `SOP3-current-warning-log`: audit the warning path, define its dependency-ordered ledger, and implement the first smallest coherent package. |
| 2026-08-25 | this session | Execute the sole authorized `SOP2f-representative-v2` invocation. Setup and one-time conditioning completed, proving v1's pre-arm setup infeasibility removed, but control warmup 0 did not complete inside its arm bound. Preserve the resulting empty-aggregate evidence-writer defect and cleanup audit at the write-once path without inventing measurements or retrying. | V2 authority is consumed. Obtain explicit approval to design a new separately versioned protocol, or explicitly waive/reject SOP2. Do not rerun v1/v2, tune, silently change bounds, or advance to SOP3. |
| 2026-08-25 | `ca4086f` | Predeclare executable `SOP2f-representative-v2` without a campaign: keep revisions, fixture, short caps, strict aggregate <1% wall/CPU gate, qualification, counterbalance, isolated state, exact reconciliation, write-once evidence, cleanup, and no-retry rules; replace 12 redundant per-arm full-content passes with one initial pass plus scan-to-scan rewarming; prove a conservative five-hour envelope by no-state preflight. | Obtain explicit approval for exactly one v2 campaign command and its build/temp-process/write/delete authority, or explicitly waive/reject SOP2. Do not run, revise, or advance to SOP3. |
| 2026-08-25 | this session | Commit the fail-closed representative harness, execute the sole approved SOP2f campaign, and retain its pre-measurement failure: clean builds and fixed-fixture creation/validation/initial conditioning passed, but the two-hour watchdog expired before any arm; cleanup passed and no aggregate exists. | Obtain explicit operator approval for a separately versioned representative protocol or an explicit SOP2 waiver/rejection. Do not rerun, tune, or advance to SOP3. |
| 2026-08-25 | this session | Approve the exact two-part SOP2f budget: 100 ms wall/125 ms CPU fixed caps for the retained short fixture plus strictly less than 1% aggregate wall/CPU on one immutable 600,008-file representative campaign; verify and stop the operator-authorized Release app PID 48972. | Commit the predeclared protocol, then implement and run exactly one write-once representative profile without resizing, retrying, excluding outliers, or advancing to SOP3. |
| 2026-08-25 | this session | Complete SOP2f local functional evidence with a named verifier, full Debug/Release matrices, isolated Release publish, and real Debug/Release worker/WPF smoke; retain the sole cross-revision profile failed at +4.39% wall and -3.83% CPU without retrying or stopping the operator's visible Release app. | Keep `SOP2f` blocked and obtain explicit operator disposition: preserve the +1%/+1% gate, approve a reviewed two-part short-absolute/representative-duration budget, or explicitly waive/reject SOP2. Do not advance to SOP3. |
| 2026-08-25 | this session | Accept `SOP2e-accessible-progress-surface` with six bounded funnel outcomes, separate candidate context, terminal-truthful operational text, system-brush/narrow-width accessibility, stable cancellation focus, and a five-second accepted-snapshot-only `MostRecent` UIA channel. | Implement `SOP2f-progress-acceptance` with one focused cross-layer verifier and the predeclared proportionate full acceptance matrix. |
| 2026-08-25 | this session | Accept `SOP2d-core-progress-projection` with strict complete typed JSON parsing, Core invariant validation, one generation-scoped latest-only 100 ms application gate, lifecycle/stale/regression rejection, and exact funnel/rate/cache/device/remaining-work/ETA projections. | Implement `SOP2e-accessible-progress-surface` using only accepted coalesced snapshots and retain the explicit accessibility/non-goal boundaries. |
| 2026-08-25 | this session | Accept `SOP2c-worker-progress-projection` with the complete additive typed snapshot, decimal-string byte quantities, one timer-owned latest-value ten-per-second emitter, sticky cancellation, strict terminal silence, truthful unavailable device state, and typed discovery at the existing 256-file callback. | Implement `SOP2d-core-progress-projection` with defensive latest-only application, stale/lifecycle rejection, and exact display-unit projection. |
| 2026-08-25 | this session | Accept `SOP2b-incremental-pipeline-publication` with one serialized cumulative engine sink, 256-file/8-MiB publication bounds, injected cache/read/failure/cancellation evidence, and exact completed/failed/cancelled live-to-durable reconciliation. | Implement `SOP2c-worker-progress-projection` with additive typed snapshots, deterministic rate/ETA/device projection, and a hard latest-value ten-per-second transport bound. |
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
