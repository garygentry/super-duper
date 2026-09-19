# ADR-0001: The Windows app reaches the engine through a worker process

- **Status:** Accepted
- **Date:** 2026-08-15 (the Windows MVP implementation, commit `d8f1e99`); recorded retrospectively
  on 2026-09-19

## Context

All product logic lives in the Rust workspace, and a Windows front end needs some way to call it.

An earlier Windows client, a WinUI 3 app under `ui/windows` built from February 2026 (commits
`e6d9f33` through `bcee9ce`), loaded `super_duper_ffi.dll` in process and called it through
P/Invoke and opaque engine handles. Its history includes lifecycle fixes at that boundary:
disposing the service provider when the window closed so the native engine handle was destroyed
(`0936164`), and cancelling and awaiting an active scan before disposal to avoid a dispose race
(`b05c9d7`). That app was removed on 2026-08-15 (`54c7d48`), and the current app was started as a
new product surface rather than a continuation of it.

The Windows MVP plan (`docs/windows-mvp-plan.md`, preserved at the `v0.1.0` tag) listed among its
fixed decisions: "a long-lived `super-duper-worker` child process, not in-process FFI"; versioned
UTF-8 newline-delimited JSON over stdin and stdout; and that only the Rust worker reads or writes
the product database. The plan does not record why a process was chosen over FFI, and no other
record gives a reason. The FFI client's fixes above are history, not the stated rationale.

## Decision

The WPF app reaches the engine only through `super-duper-worker`, a long-lived child process that
speaks the versioned JSONL protocol in [worker protocol v1](../../worker-protocol-v1.md) over its
standard streams. The app never loads `super-duper-ffi` and never opens a product database.

## Consequences

These follow from the decision and are visible in the code today:

- **A worker failure does not end the app.** `WorkerClient.MonitorExitAsync` notices an unexpected
  exit and fails pending requests; `ShellViewModel` shows the recovery screen, and **Reconnect**
  calls `RestartAsync` to start a fresh worker, whose database open marks abandoned runs
  interrupted (`apps/windows/src/SuperDuper.Windows.Infrastructure/WorkerClient.cs`,
  `apps/windows/src/SuperDuper.Windows.Core/ViewModels/ShellViewModel.cs`).
- **Stdout is protocol-only.** The worker sends tracing and diagnostics to stderr
  (`crates/super-duper-worker/src/main.rs`); the app keeps a bounded tail and a rotating log of it.
- **Framing is strict on both sides.** Frames are UTF-8, LF-terminated and at most 1,048,576
  bytes. Oversized, partial or malformed input is fatal to the worker, and a malformed frame or an
  unknown response ID makes the client kill the worker (`crates/super-duper-worker/src/lib.rs`,
  `apps/windows/src/SuperDuper.Windows.Infrastructure/Protocol/JsonLineProtocol.cs`).
- **The worker is restartable in place.** `IRestartableWorkerClient` lets the shell replace a dead
  process without restarting the app, and shutdown can bound the worker's lifetime: close stdin,
  wait 2 seconds, then kill.
- **Every capability is a protocol method.** New app features need a worker method and a protocol
  document update; additive fields are allowed within v1. The C# contracts in
  `apps/windows/src/SuperDuper.Windows.Core/Workers/WorkerContracts.cs` mirror the worker's JSON
  by hand, so a shape change touches both sides.
- **Database ownership has one place to live.** The worker can hold the database lock for its
  lifetime ([ADR-0003](0003-one-owner-per-database.md)).
- **A second executable ships and must be found.** The app project copies the worker into its
  output, the release script checks both executables' versions, and `WorkerExecutableLocator`
  decides which worker runs.
- **The FFI crate stays app-neutral.** `super-duper-ffi` is kept as a C ABI for future native
  clients and is not used by the Windows app (`AGENTS.md`).
