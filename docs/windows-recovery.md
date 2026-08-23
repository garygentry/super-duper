# Windows Diagnostics, Limitations, And Recovery

## Diagnostics

Worker stdout is protocol-only. Human diagnostics, recoverable filesystem warnings, panics, and
performance records go to stderr. The WPF host drains stderr, retains a bounded tail for connection
failures, and writes a rotating local log:

```text
%LOCALAPPDATA%\SuperDuper\logs\worker.log
%LOCALAPPDATA%\SuperDuper\logs\worker.log.previous
```

The active log rotates at approximately 5 MiB during a worker lifetime and retains one previous
segment. Performance records contain run/group identifiers, counts, and durations but no searched
path/filter text. Filesystem diagnostics are local and can contain paths needed for
troubleshooting. Set `SUPER_DUPER_LOG` to a Rust tracing filter such as
`super_duper_core=debug` for more detail. Never redirect diagnostics to worker stdout.

## Known MVP Limitations

- Windows 11 x64 only; output is unpackaged and framework-dependent.
- Fixed local drives are primary. Removable, mapped, and UNC roots are explicitly selected and
  best-effort. Disconnects, credentials, provider latency, and mapped-drive visibility under a
  different account can produce warnings.
- There is no automatic drive discovery, reconnect, pause/resume, or scheduled/background scan.
- The app and worker are long-path aware, but a remote provider can impose its own limits.
- Reparse points, junctions, and symbolic links are skipped. Hard-linked directory entries are
  snapshotted but one physical file is not counted as multiple recoverable copies.
- Files that disappear, become inaccessible, or change metadata after discovery are warned and
  excluded from affected duplicate results. A completed run can therefore have warnings.
- V1 exposes a warning count and local diagnostics; `warning.page` remains reserved.
- The WPF post-MVP review surface exposes non-deleting review decisions, preflight observations,
  and reconstruction of schema-v10 Recycle Bin operation intent/evidence. Schema v11 adds a bounded
  append-only recovery-review checklist for manual operator observations and corrections. The production
  executor is disabled: there is no Recycle Bin submission/mutation action or arbitrary filesystem
  mutation exposed by the app. Opening the Recycle Bin is navigation for independent inspection
  only. A separately gated real executor exists only for explicit disposable acceptance tests.
  Deleting a session removes worker-owned history only when no operation lock requires its evidence;
  it never deletes scanned files.
- The database and hash cache use the worker working directory unless `SUPER_DUPER_DB_PATH` and
  `HASH_CACHE_PATH` are set. Keep unpackaged output in a user-writable location.

## Worker Startup Failure

1. Read the recovery screen; it includes the attempted executable and diagnostic log paths.
2. Build Rust before .NET, or set `SUPER_DUPER_WORKER_PATH` to an absolute compatible worker.
3. Verify worker and WPF app came from the same source/release output.
4. Inspect `worker.log` for database migration/open or protocol negotiation errors.
5. Restart after correcting the executable, permissions, or database issue.

## Unexpected Exit Or Interrupted Run

An unexpected worker exit fails pending UI requests but does not alter completed history. The app
shows a dedicated recovery screen instead of leaving stale scan progress. Choose **Restart worker**
to start a fresh owned process, negotiate protocol V1, reload sessions/history, and reconcile
durable `running` or `cancelling` rows to `interrupted`. Partial results are not presented as
completed. Inspect the interrupted run and log, then start a new immutable run. If restart fails,
the recovery screen retains the executable and diagnostic-log paths. Do not edit SQLite state
manually.

An abandoned preflight is likewise reconciled to `interrupted`. Its already committed observations
remain queryable, but they are never resumed or treated as execution authority. If the review
revision is still current, start a new preflight operation to obtain a fresh validation generation.

An abandoned schema-v10 operation that never reached durable submission becomes `expired` on
startup. A test-injected operation in `submitted`, `executing`, or `cancelling` becomes
`recovery_required`. Every pending item in a durable `shell_started` batch becomes `unknown` with a
recovery record because mutation may have occurred before a result was persisted. Do not retry,
edit, or clear that operation: its run/review remain locked to prevent repeating a potentially
completed mutation. Schema v11 can separately append one of the five approved operator
observations per unknown item and preserve corrections as supersession history; even complete
review remains explicitly unresolved and changes none of those original rows. In WPF, page every
unknown item, copy its stored path/evidence, inspect the Windows Recycle Bin independently, record
one of the five observations, and use an explicit correction to supersede a current observation.
The prior record and correction reason remain in history. Failed reads and a failed observation
request expose only exact safe retry; the latter reuses the same idempotent request. **Start a fresh
scan** navigates to new scan authority and never replays an old operation. The app does not inspect
source/provider/content/Recycle Bin state, infer an outcome, restore, delete, clear evidence, or
change any original status. The production app cannot initiate Shell work, but the explicitly
invoked disposable executor tests can exercise this boundary.

The accepted `WPM11-ambiguous-start` development-host campaign is retained at
`artifacts/windows-ambiguous-start/20260823-144048-588`. Its disposable host stopped only after the
durable `shell_started` marker and before `IFileOperation.PerformOperations`; Explorer, providers,
the database, and its worker were not killed. Restart reconstructed two immutable `unknown` items,
an `ambiguous` batch, a `recovery_required` operation, and both recovery rows. The operator then used
the real WPF Option A checklist to append three observations for two items, including one explicit
supersession with its prior record and correction reason retained. The exact verifier proved all
source-evidence rows unchanged and no retry, replay, resubmission, inference, restore, deletion, or
copy-forward. Run it only against the retained bundle after closing WPF normally:

```powershell
./scripts/Verify-WindowsAmbiguousStartCampaign.ps1 `
  -EvidenceDirectory artifacts/windows-ambiguous-start/20260823-144048-588
```

The earlier safe prepare failure is retained separately at
`artifacts/windows-ambiguous-start/20260823-143955-657`; it never reached durable Shell start.

Preserve the timestamped evidence bundle produced by
`Invoke-WindowsRecycleBinAcceptance.ps1` with the database and logs. The acceptance matrix in
[`windows-recycle-bin-acceptance.md`](windows-recycle-bin-acceptance.md) requires numeric callback,
HRESULT, abort, source/survivor, and Recycle Bin observations; absence of a source path alone is
never proof that this app recycled it.

## Database Failure Or Suspected Corruption

1. Close Super Duper and confirm its worker exited.
2. Find the database: `SUPER_DUPER_DB_PATH` when set, otherwise `super_duper.db` beside the worker.
3. Copy the database, `-wal`, and `-shm` together to a safe location. Never copy only the main file
   while the worker is running.
4. Preserve the log and exact app/worker versions.
5. To test a clean start without destroying evidence, move the complete database set aside and
   restart. Restore it only while the app is closed.

Schemas v10 and v11 have no in-place downgrade. Before a v9-to-v10 or v10-to-v11 first open, close
the app/worker and copy the database, `-wal`, and `-shm` as one set. To return to an older build,
restore the matching complete pre-migration backup while all processes are closed. Never lower
`user_version` or manually drop operation/recovery-review tables.

Unknown old schemas, newer schemas, migration failures, corruption, and inability to persist a
consistent run are fatal by design; the worker does not truncate or silently recreate them.

## Hash Cache Failure

The RocksDB cache is an optimization. Lookup/store failures become warnings. Close the app before
moving a damaged cache aside. Its path is `HASH_CACHE_PATH` when set and
`content_hash_cache.db` beside the worker otherwise. The next scan recreates it and may be slower.
