# ADR-0006: Last-chance exception handling logs, stops the worker, and exits

- **Status:** Accepted
- **Date:** 2026-09-20

## Context

Nothing subscribed to `DispatcherUnhandledException`, `AppDomain.UnhandledException` or
`TaskScheduler.UnobservedTaskException` (issue #43). An exception escaping any of the app's 24
`async void` methods, or a background thread, reached WPF's or the CLR's default handling: the
process ended without a log entry, a message the person could read, or the orderly worker shutdown
in `MainWindow.ShutdownAsync`.

Separately, `WorkerClient.DispatchEvent` invoked event subscribers (`RunProgress`,
`RunLifecycleChanged`, `ResultStateChanged`) inside the same `catch` that guards frame parsing. A
bug in a subscriber — a view model's event handler — was indistinguishable from a real protocol
violation: it failed every pending request and killed the worker, which then showed the recovery
screen for what was really a UI bug.

## Decision

**Process-wide.** `App`'s constructor subscribes all three handlers before any other code runs.
`DispatcherUnhandledException` and `AppDomain.UnhandledException` converge on one fatal path: log
the exception through `IWorkerClient.LogDiagnosticAsync` (blocking on it, since none of these
handlers are `async`), dispose the DI container and the single-instance gate — the same calls
`App.OnExit` makes, not the confirm-and-cancel `ShutdownAsync` flow, which assumes a working window
— show a message box naming the log file, then `Environment.Exit(1)`.
`TaskScheduler.UnobservedTaskException` is not fatal: it logs the same way and calls
`SetObserved()`.

No per-handler `try`/`catch` was added. All 24 `async void` methods were audited: none use
`ConfigureAwait(false)` or a detached `Task.Run`, so every continuation resumes on the dispatcher's
synchronization context, where WPF already routes an unhandled exception to
`DispatcherUnhandledException`. The global handler is the audit's outcome, not a substitute for one
that was skipped.

**Per-subscriber.** `WorkerClient.DispatchEventAsync` now separates parsing frame data (still
wrapped and rethrown as `WorkerProtocolException`, still a real protocol failure worth killing the
pump for) from invoking subscribers (`InvokeSubscribersAsync`), which catches any exception, logs
it through the same `LogDiagnosticAsync` path, and lets the pump continue.

**One writer per diagnostic log, not two.** The first version of this logged through a second,
independent file handle (`CrashReportLog`, opened fresh per write) alongside the stderr relay's
long-lived `BoundedDiagnosticLog` writer. A test that threw from a `run.started` subscriber while a
scan was writing performance lines to the same file caught the bug: two `FileStream`s opened in
`FileMode.Append` each seek to end once, at open time, and then track their own position — they do
not coordinate through the OS on every write — so an interleaved write from one truncated the
other's. `WorkerClient` now owns one `BoundedDiagnosticLog` per connection (opened with the
process, disposed with it, internally serialized with a semaphore so the stderr-relay pump and the
event pump can both call it safely) and exposes it as `IWorkerClient.LogDiagnosticAsync`.
`CrashReportLog` remains, but only as the fallback for when no connection is open to own a writer —
a single writer at that point, so no corruption risk.

**Left out.** A per-request timeout was considered and rejected for this stage: several requests
(a large review or history page, a preference preview over a big run) have no natural upper bound,
so one fixed timeout would misfire on a legitimately slow request rather than a genuinely stuck one.
Recorded on issue #43 rather than attempted here.

## Consequences

- **A bug now ends the app once, visibly.** Instead of a silent process exit, the person sees one
  message box naming the log file, and the log has the exception that caused it.
- **A view-model bug in an event handler no longer looks like a worker crash.** The pump survives a
  throwing subscriber; only a genuine protocol violation still fails pending requests and kills the
  worker.
- **The fatal path is deliberately blunt.** It does not attempt the window's normal close
  confirmation or animation; a corrupted UI thread cannot be trusted to run either. `Environment.Exit`
  after cleanup accepts an abrupt exit over an uncertain one.
- **Logging a crash needs the worker client.** `IWorkerClient` gained `LogDiagnosticAsync` for this;
  every current and future test double implementing the interface must implement it (a no-op is
  fine for tests that do not assert on it).
