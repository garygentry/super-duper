# ADR-0005: App state lives in %LOCALAPPDATA%\SuperDuper

- **Status:** Accepted
- **Date:** 2026-09-18 (commit `eb9f99b`); recorded retrospectively on 2026-09-19

## Context

The app starts the worker with the worker's own folder, which is the install folder, as its
working directory (`WorkerClient.StartWorker`). Without overrides the worker kept
`super_duper.db`, `scan_status.db` and the hash cache in that working directory. Two problems
followed: a read-only install location failed, and extracting a newer release zip into a new folder
looked like lost scans and review decisions. The diagnostic log and presentation preferences
already lived under `%LOCALAPPDATA%\SuperDuper`.

Developers, tests and smoke runs also need to point a run at disposable state and keep everything
for that run in one place (`AGENTS.md`, Safety Invariants).

## Decision

`apps/windows/src/SuperDuper.Windows.Infrastructure/WorkerStateLocations.cs` resolves all app
state, and the app passes the resolved paths to the worker as environment variables:

| Setting | Resolution |
|---|---|
| Database | `SUPER_DUPER_DB_PATH` if set, otherwise `%LOCALAPPDATA%\SuperDuper\super_duper.db` |
| State folder | The database's folder |
| Status database | `SUPER_DUPER_STATUS_DB_PATH` if set, otherwise `scan_status.db` in the state folder |
| Hash cache | `HASH_CACHE_PATH` if set, otherwise `content_hash_cache.db` in the state folder |
| Presentation preferences | `presentation-preferences.json` in the state folder |

The default folder is created when the app starts the worker, and only when no database override
is set. The standalone worker and CLI defaults are unchanged: without variables they still use
`super_duper.db` relative to their working directory.

## Consequences

- **State survives upgrades.** A new release can be unzipped anywhere, and the install folder can
  be read-only.
- **Overrides stay self-contained.** Setting only `SUPER_DUPER_DB_PATH` moves the status database,
  hash cache and preferences with it, which is what disposable runs and the Debug `.uidev` sidecar
  rely on.
- **The state folder is the unit of ownership.** The single-instance gate is keyed on it, and the
  worker's database lock backs it up ([ADR-0003](0003-one-owner-per-database.md)). With state
  shared by default, a second window became possible and made that decision necessary.
- **Removing the app keeps the data.** Deleting the install folder leaves saved scans, results and
  decisions in `%LOCALAPPDATA%\SuperDuper`.
- **One path does not follow.** The worker diagnostic log is always
  `%LOCALAPPDATA%\SuperDuper\logs\worker.log`, even when the database is overridden
  ([risks](../windows-app-risks.md#diagnostics)).
