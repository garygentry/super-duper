# ADR-0002: The Windows app is review-only

- **Status:** Accepted
- **Date:** 2026-08-19 (commit `f73f461`, which registered the disabled executor in production);
  recorded retrospectively on 2026-09-19

## Context

Super Duper exists to help an owner decide which duplicate copies to keep. Acting on those
decisions means deleting or recycling files the owner may not be able to recover, on drives that
can include removable media, network shares and cloud-synchronized folders.

The Windows MVP plan (`docs/windows-mvp-plan.md`, preserved at the `v0.1.0` tag) deferred file
deletion, Recycle Bin operations and deletion strategies, and required that the application perform
no deletions. It named Recycle Bin integration through `IFileOperation` as the first post-MVP
candidate.

Later work built that integration's foundation without enabling it: a whole-plan check against
scan snapshots, a revision-bound recycle-operation ledger in the database, a native executor that
drives `IFileOperation` on its own STA thread, and a recovery review for operations whose outcome
is unknown. Enabling production execution was made to depend on physical and provider evidence
(locked and access-denied files, disconnects, capacity, cloud placeholders that must not be
hydrated, the Shell's residual time-of-check to time-of-use interval, large plans, accessibility of
the operation screens) and on a separate explicit authorization. That evidence was not gathered, and
the work is parked; issue #28 now tracks it.

## Decision

Production builds are review-only. `App.xaml.cs` registers
`DisabledRecycleOperationCapabilityExecutor` as the `IRecycleOperationCapabilityExecutor`: it
reports every item as not recyclable and throws if asked to execute.
`RecycleOperationViewModel.CanSubmit` is always false, the worker reports `executorEnabled:false`
in its operation responses and live-state events, and no screen offers an action that deletes,
moves or modifies a scanned file. Changing any of this needs explicit operator direction
(`AGENTS.md`, Safety Invariants).

## Consequences

- **Decisions record intent only.** **Keep copy**, **Mark copy for removal** and the folder
  decisions are durable records in the review plan. **Check marked copies** reads metadata and
  content to compare against the scan and changes nothing. Owners remove files themselves,
  outside the app.
- **Wording must match.** The UI states that marking does not delete files and that moving files to
  the Recycle Bin is not available; no text may promise reclaimed space.
- **A tripwire in the client.** `WorkerClient` treats `executorEnabled: true` in a
  `result.state_changed` event as a protocol violation
  (`apps/windows/src/SuperDuper.Windows.Infrastructure/WorkerClient.cs`).
- **Dormant code stays in the tree.** The recycle-operation client methods,
  `WindowsRecycleOperationExecutor` and the recovery-review screens remain and are tested, but the
  executor is constructed only by tests and the screens appear only when a stored operation exists
  or cannot be loaded. See [risks](../windows-app-risks.md#dormant-code).
- **Real Recycle Bin tests are opt-in.** The `RealRecycleBin` and `RealRecycleBinProvider` test
  categories run only when their environment variables are set, because they change the real
  Recycle Bin with disposable fixtures.
- **Enablement is a new decision.** Turning execution on would supersede this record, after the
  evidence and authorization tracked in issue #28.
