# SOP10 Physical Acceptance Checklist

Status: ready for a separately authorized operator run. This checklist does not authorize a scan.

## Authority and isolation

Before starting, record explicit operator approval for the exact roots, the new acceptance identity,
and the time window. Do not use either consumed SOP9c V1/V2 identity. Do not start the parked Windows
release-validation stream as part of this run.

1. Confirm no `SuperDuper.Windows` or `super-duper-worker` process is active.
2. Use the fixed framework-dependent build in `artifacts/windows-x64` and verify the three SHA-256
   values recorded in `docs/evidence/scan-folder-remediation-sop10-acceptance-20260905.json`.
3. Create a new operator-chosen acceptance state directory. Set `SUPER_DUPER_DB_PATH`,
   `SUPER_DUPER_STATUS_DB_PATH`, and `HASH_CACHE_PATH` to distinct paths under that directory before
   launching the app. Set `SUPER_DUPER_WORKER_PATH` to the fixed packaged worker.
4. Verify that none of those paths is the cancelled run database, its status database, or the
   legacy cache. Do not delete, move, rename, compact, or overwrite the cancelled state.
5. Keep registered-cloud-root exclusion enabled. Review the exact roots and manual exclusions before
   starting. Production Recycle Bin execution remains disabled; perform no deletion operation.

## First fixed run

1. Select `reuse_verified`, the default policy, and record the immutable run ID and parameters.
2. Start only after the separately approved roots and isolated state paths have been rechecked.
3. Observe all four bounded folder substages: hierarchy, structural candidates, verification, and
   persistence. Retain warning/status evidence rather than dismissing or rewriting it.
4. Allow the run to reach a durable terminal state. Record wall time, peak private memory, final
   partial/full cache counters, physical bytes read, warning totals, and ordered exact file/folder
   result digests.

Expected behavior: the old cache did not retain the current stable identity/change-token proof, and
the isolated acceptance cache begins unqualified. Those misses must be read and hashed once. They
must not be treated as verified hits merely because path, size, or modification time matches.
Successful reads seed qualified schema-v3 entries for later runs. Cache/store failures remain
warnings with content-read fallback; they are never permission to weaken signature checks.

## Unchanged repeat run

1. Do not modify, rename, hydrate, replace, or relink files between runs. Use the same isolated
   product/status databases and qualified cache, and keep the same roots/exclusions.
2. Start a new run with `reuse_verified`; do not reuse or mutate the first run record.
3. Retain the same counters and ordered result digests as for the first run.

Expected behavior: entries with the complete stable identity/change-token proof reuse qualified
partial and, where available, full hashes across the reopened store. Unqualified, changed,
unavailable-signature, corrupt, or evicted entries remain misses and are read normally. Exact result
digests, hard-link survivor semantics, cloud exclusions, and warning accounting must remain equal.

`revalidate_content` remains the user override. If the operator separately chooses it for a later
diagnostic run, it must bypass qualified hits and read current content while preserving an immutable
run-policy snapshot; it is not required for the two-run SOP10 acceptance above.

## Stop and retain evidence

Cancel and retain the state without cleanup if any of these occurs: a root/exclusion mismatch,
unexpected cloud access or hydration, signature-only reuse without full qualification, incorrect
hard-link recovery counts, a result-digest mismatch, a folder substage that regresses or stalls
without bounded updates, false warning/cancellation state, cache growth beyond the published
5,000,000/4,500,000/10,000,000 policy, or sustained resource behavior inconsistent with the retained
Release-scale result. Do not tune and rerun under the same identity.
