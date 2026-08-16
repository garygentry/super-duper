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
- The WPF MVP exposes no file deletion, Recycle Bin action, deletion plan, shell extension, or
  arbitrary filesystem mutation. Deleting a session removes worker-owned history, not scanned files.
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

## Database Failure Or Suspected Corruption

1. Close Super Duper and confirm its worker exited.
2. Find the database: `SUPER_DUPER_DB_PATH` when set, otherwise `super_duper.db` beside the worker.
3. Copy the database, `-wal`, and `-shm` together to a safe location. Never copy only the main file
   while the worker is running.
4. Preserve the log and exact app/worker versions.
5. To test a clean start without destroying evidence, move the complete database set aside and
   restart. Restore it only while the app is closed.

Unknown old schemas, newer schemas, migration failures, corruption, and inability to persist a
consistent run are fatal by design; the worker does not truncate or silently recreate them.

## Hash Cache Failure

The RocksDB cache is an optimization. Lookup/store failures become warnings. Close the app before
moving a damaged cache aside. Its path is `HASH_CACHE_PATH` when set and
`content_hash_cache.db` beside the worker otherwise. The next scan recreates it and may be slower.
