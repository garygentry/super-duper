# ADR-0003: One worker owns a database, one window per data folder

- **Status:** Accepted
- **Date:** 2026-09-18 (commit `76def1b`); recorded retrospectively on 2026-09-19

## Context

When the worker opens its database, core marks every run left `running` or `cancelling` as
`interrupted`, and does the same for unfinished whole-plan checks and recycle operations
(`crates/super-duper-core/src/storage/sqlite.rs`, `Database::open`). That repairs state after a
crash, but it is only correct when no other worker is using the same database.

Once app state moved to one shared folder by default
([ADR-0005](0005-app-state-in-localappdata.md)), opening Super Duper twice was enough to start a
second worker on the same database. Its startup reconciliation marked the first worker's live scan
interrupted. The same happened with any second worker, for example from a script.

A second problem was diagnosis: when the database could not be opened, the app only reported that
the worker had exited unexpectedly.

## Decision

- **The worker locks its database.** Before opening `super_duper.db`, the worker takes an
  exclusive lock on `<database>.lock` and holds it for its lifetime, waiting up to 3 seconds because
  Windows releases a dead process's lock asynchronously (`crates/super-duper-worker/src/lib.rs`).
- **It says why it cannot open.** If the lock or the open fails, the worker answers every request,
  `hello` included, with `database_unavailable` and a `reason` (`in_use`, `newer_version`,
  `unsupported_version`, `damaged`, `read_only`, `unavailable`, `disk_full` or `failed`) plus the
  database path, until its input ends, then exits with code 1. Core classifies open failures, and
  rejects a newer database before any pragma so that switching to WAL cannot rewrite its header.
- **The app is single-instance per state folder.** `SingleInstanceGate` creates a named event
  derived from the state folder. A second launch for the same folder signals it, the first window
  comes forward, and the second process exits
  (`apps/windows/src/SuperDuper.Windows.Infrastructure/SingleInstanceGate.cs`). The database lock
  remains the backstop for anything the gate does not cover.

## Consequences

- **A live scan cannot be marked interrupted by a second process.** Only the lock holder runs
  startup reconciliation.
- **Clear failure screens.** `WorkerDatabaseUnavailableException.Describe()` turns each reason into
  a title, guidance and the file path, and **Reconnect** retries
  (`apps/windows/src/SuperDuper.Windows.Core/Workers/WorkerDatabaseUnavailableException.cs`).
- **Different state folders are independent.** Two apps with different `SUPER_DUPER_DB_PATH`
  values run side by side, each with its own worker.
- **Scripts and tests must not share a database between workers.** `AGENTS.md` forbids a second
  concurrent worker on the same database. Worker-backed Infrastructure test classes run one at a
  time (`[DoNotParallelize]`) to avoid overloading the runner.
- **The lock is the worker's, not core's.** The CLI and FFI open databases through core without
  taking it; by default they also use a different file (`super_duper.db` in the current directory).
- **A restart can wait briefly.** A worker started right after another was killed may wait up to
  3 seconds for the lock before answering.
