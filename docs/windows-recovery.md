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
  and read-only reconstruction of schema-v10 Recycle Bin operation intent/evidence. The production
  executor is disabled: there is no Recycle Bin action or arbitrary filesystem mutation exposed by
  the app. A separately gated real executor exists only for explicit disposable acceptance tests.
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
completed mutation. This build has no recovery-resolution UI. The production app cannot initiate
Shell work, but the explicitly invoked disposable executor tests can exercise this boundary.
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

Schema v10 has no in-place downgrade. Before a v9-to-v10 first open, close the app/worker and copy
the database, `-wal`, and `-shm` as one set. To return to a v9 build, restore that complete backup
while all processes are closed. Never lower `user_version` or manually drop operation tables.

Unknown old schemas, newer schemas, migration failures, corruption, and inability to persist a
consistent run are fatal by design; the worker does not truncate or silently recreate them.

## Hash Cache Failure

The RocksDB cache is an optimization. Lookup/store failures become warnings. Close the app before
moving a damaged cache aside. Its path is `HASH_CACHE_PATH` when set and
`content_hash_cache.db` beside the worker otherwise. The next scan recreates it and may be slower.
