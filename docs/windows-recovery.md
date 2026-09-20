# Windows diagnostics, limitations and recovery

How to diagnose worker, database, hash cache and interrupted-operation problems in the Windows app
without destroying the evidence you need.

## Diagnostics

Worker stdout carries only protocol frames. Human diagnostics, recoverable filesystem warnings,
panics and performance records go to stderr. The app drains stderr, keeps the last 16,384
characters to show in connection-failure messages, and writes everything to a rotating log:

```text
%LOCALAPPDATA%\SuperDuper\logs\worker.log
%LOCALAPPDATA%\SuperDuper\logs\worker.log.previous
```

- The log lives in a `logs` folder beside the database: `%LOCALAPPDATA%\SuperDuper\logs` with no
  overrides, or `<folder>\logs` when `SUPER_DUPER_DB_PATH` moves the database elsewhere. This keeps
  a disposable run (smoke, UI-dev session) self-contained instead of writing into the real log
  folder.
- The active log rotates at 5 MiB and keeps one previous file.
- Performance records (`performance kind=scan_phase …` and `performance kind=result_query …`)
  contain run and group identifiers, counts and durations, but no searched path or filter text.
  Filesystem diagnostics are local and can contain paths.
- Set `SUPER_DUPER_LOG` to a Rust tracing filter, such as `super_duper_core=debug`, for more
  detail. Never redirect diagnostics to worker stdout.

Recoverable filesystem warnings are also stored durably with each run, as bounded per-kind
aggregates with an exact count and up to three examples. Review them in the app from
**History** > **Scans** > **Review warnings**; they survive worker restarts.

## Known limitations

- Windows 11 x64 only; releases are an unpackaged, self-contained zip.
- Fixed local drives are the primary target. Removable, mapped and UNC roots are explicitly
  selected and best-effort: disconnects, credentials, provider latency and mapped-drive visibility
  under a different account can produce warnings.
- There is no automatic drive discovery, pause and resume, or scheduled or background scan.
- The app and worker are long-path aware, but a remote provider can impose its own limits.
- Reparse points, junctions and symbolic links are skipped. Hard-linked directory entries are
  recorded, but one physical file is not counted as several recoverable copies.
- Files that disappear, become inaccessible or change metadata after discovery produce warnings and
  are left out of the affected duplicate results, so a completed run can have warnings.
- The app is review-only. It records decisions, runs non-deleting preflight checks, and can show
  recorded Recycle Bin operation evidence, but production Recycle Bin execution is disabled: no
  app action moves, recycles or deletes a file. **Open Recycle Bin** only opens the Windows Recycle
  Bin for your own inspection. The real Shell executor runs only in opt-in tests with disposable
  files (see [`windows-testing.md`](windows-testing.md)).
- Deleting a saved scan removes only the worker's own history for it, never scanned files. It is
  refused while any of its runs has a Recycle Bin operation that is unfinished or needs recovery.
- The database, status database, hash cache and preferences live in `%LOCALAPPDATA%\SuperDuper`
  unless `SUPER_DUPER_DB_PATH` is set; then they follow that database's folder, unless
  `SUPER_DUPER_STATUS_DB_PATH` or `HASH_CACHE_PATH` overrides them individually.

## Worker startup failure

1. Read the recovery screen. **Technical details** shows the worker executable the app tried and
   the diagnostic log path.
2. Check which worker the app starts. A Release app starts only `super-duper-worker.exe` beside
   it. A Debug app tries `SUPER_DUPER_WORKER_PATH`, then the worker beside the app, then
   `target/debug/super-duper-worker.exe`; a stale worker left beside a Debug app wins over a newer
   one in `target/debug`. Build Rust before .NET so the build copies the current worker.
3. Confirm the worker and the app came from the same source or release package.
4. Look in `worker.log` for database open or migration errors and protocol negotiation errors.
   The app waits 10 seconds for the worker's handshake before reporting a failure.
5. Correct the executable, permissions or database problem, then choose **Reconnect**.

## Unexpected exit or interrupted run

An unexpected worker exit fails the requests in flight but does not change completed history.
The app shows the recovery screen instead of stale progress. **Reconnect** starts a new worker,
negotiates protocol v1 and reloads saved scans and history. When a worker starts and opens the
database, it reconciles work that has no live owner:

| Record | Left in | Becomes |
|---|---|---|
| Scan run | `running`, `cancelling` | `interrupted` |
| Preflight check | `running`, `cancelling` | `interrupted` |
| Recycle Bin operation | `prepared`, `awaiting_confirmation` | `expired` |
| Recycle Bin operation | `submitted`, `executing`, `cancelling` | `recovery_required` |
| Operation batch | `shell_started` | `ambiguous` |
| Item in that batch | `pending` | `unknown`, with a recovery record |

- An interrupted run's partial results are never shown as completed. Inspect the run and the log,
  then start a new scan.
- An interrupted preflight check keeps the observations it had committed, but they are never
  resumed or treated as current. Start a new preflight check while the review is still current.
- An `unknown` item means Shell work may have happened before a result was saved. Do not retry,
  edit or clear the operation. Its run and review stay locked so a possibly completed action cannot
  be repeated.

For a `recovery_required` operation, the **Review** area shows **Recovery review**:

1. Page through every unknown item. Use **Copy stored path** and **Copy evidence**, and inspect the
   Windows Recycle Bin and the source location yourself (**Open Recycle Bin**).
2. Record one observation per item: **Observed in Recycle Bin**, **Observed at source**,
   **Observed in both**, **Observed in neither** or **Deferred unresolved**.
3. To change a recorded observation, use **Correct selected observation** and give a reason. The
   earlier record and the reason stay in the history.

Observations are appended and never change the original `unknown`, `ambiguous` or
`recovery_required` records; even a complete review stays unresolved. The app does not inspect
the source, provider, content or Recycle Bin itself, and never infers an outcome, restores or
deletes. After a failed read or a failed observation request, only **Retry review read** or
**Retry same observation request** is offered; the latter resends the same request, so it cannot
record the observation twice. **Start a fresh scan** leaves the operation as it is and never
replays it.

Never edit SQLite state by hand.

## Database failure

When the worker cannot open its database, the app names the reason instead of reporting a crash:
already open in another window, data from a newer version, an early version that cannot be
upgraded, damaged, read-only, location unavailable, or disk full. Each screen shows the database
path, and the worker does not modify the file in any of these cases.

Super Duper runs one window per data folder; starting it again brings the existing window forward.
The worker also holds `super_duper.db.lock` beside the database while it runs, so a second worker
on the same database (for example a script with the same `SUPER_DUPER_DB_PATH`) is refused instead
of marking the first worker's scan interrupted. The lock file can remain after exit; it is
harmless.

To preserve evidence:

1. Close Super Duper and confirm its worker exited.
2. Find the database: `SUPER_DUPER_DB_PATH` when set, otherwise
   `%LOCALAPPDATA%\SuperDuper\super_duper.db`.
3. Copy the database, `-wal` and `-shm` files together to a safe location. Never copy only the main
   file, and never copy while the worker is running.
4. Keep the log and the exact app and worker versions.
5. To test a clean start, move the complete database set aside and restart. Restore it only while
   the app is closed.

Every schema upgrade is one-way; there is no in-place downgrade. Before a newer build opens a
database for the first time, close the app and copy the database, `-wal` and `-shm` as one set. To
return to an older build, restore that complete backup while all processes are closed. Never lower
`user_version` or drop tables by hand.

Unknown old schemas, newer schemas, migration failures, corruption and an inability to save a
consistent run are fatal by design; the worker never truncates or silently recreates the database.

## Hash cache failure

The RocksDB hash cache is an optimization. Lookup and store failures become warnings. Close the app
before moving a damaged cache aside. Its path is `HASH_CACHE_PATH` when set, otherwise
`content_hash_cache.db` in the database's folder (`%LOCALAPPDATA%\SuperDuper` by default). The next
scan recreates it and may be slower.
