# Windows app risks and technical debt

## Purpose

This chapter lists known weaknesses in the Windows app so you can weigh them before changing
nearby code. Every item is a finding from reading the source, with the file that shows it; none was
reproduced at run time for this page. Where a GitHub issue already tracks an item, it is named.
Paths are relative to `apps/windows/src/` unless they start with another top-level folder.

## Reliability

- **No per-request timeout.** Only the `hello` handshake (10 seconds) and the shutdown grace
  (2 seconds) are timed. A request the worker never answers waits until its caller cancels or the
  connection fails (`SuperDuper.Windows.Infrastructure/WorkerClient.cs`, `SendRequestAsync`). Left
  out deliberately (issue #43): several requests (large review or history pages, preference
  preview over a big run) have no natural upper bound, so a fixed timeout would misfire on a
  legitimately slow one rather than a stuck one. A future attempt would need a per-request-kind
  budget, not one constant.

## Diagnostics

- **Log failures are silent.** If `BoundedDiagnosticLog` cannot open or write the file, it gives up
  quietly and only the 16 KiB in-memory stderr tail remains
  (`SuperDuper.Windows.Infrastructure/BoundedDiagnosticLog.cs`).
- **Test failure evidence is deleted.** When a worker-backed Infrastructure test fails, the
  worker's stderr log is removed with the test's temporary folder (issue #35).

## Verification gaps

- **The protocol smoke does not use the published worker.** The worker half of
  `scripts/Invoke-WindowsSmoke.ps1` always launches `target/<profile>/super-duper-worker.exe`, even
  with `-AppPath`; only the WPF half runs the published app and the worker beside it. CI runs
  `scripts/Verify-WindowsRelease.ps1 -SkipWpfSmoke` (`.github/workflows/ci.yml`), so no binary from
  the publish folder is exercised there.
- **A missing worker does not fail the build.** Both copy targets in
  `SuperDuper.Windows/SuperDuper.Windows.csproj` run only when the Cargo-built worker exists.
  Debug discovery prefers a worker already beside the app (`WorkerExecutableLocator.cs`), so after
  `cargo clean` a stale copy in `bin/` can run.
- **Smoke tests use an older test stack.** `SuperDuper.Windows.Smoke.Tests.csproj` references
  MSTest 3.11.1 with `Microsoft.NET.Test.Sdk` 17.14.1, while the other test projects use the
  MSTest 4.4.1 meta-package and `AGENTS.md` describes only the latter (issue #35).
- **The release zip is overwritten.** `scripts/Verify-WindowsRelease.ps1` always writes
  `artifacts/super-duper-<version>-win-x64.zip`, so a later local run replaces a verified release
  candidate (issue #35).
- **Accessibility is unverified on a real desktop.** High contrast (the app has no explicit
  high-contrast handling; shell tab highlights use `SystemColors` keys), Narrator and NVDA,
  per-monitor DPI (`SuperDuper.Windows/app.manifest` declares long-path awareness but no
  `dpiAwareness`), and keyboard-only use (issue #23).
- **Real-provider tests are opt-in.** The `RealCloudProvider`, `RealRecycleBin` and
  `RealRecycleBinProvider` test categories in
  `apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/` report Inconclusive unless their
  environment variables are set, and CI does not set them.

## Dormant code

- **The parked Recycle Bin feature.** `IRecycleOperationWorkerClient` in
  `SuperDuper.Windows.Core/Workers/IWorkerClient.cs` declares the full recycle-operation ledger,
  but app code calls only the read and recovery-review methods; prepare, eligibility, confirm,
  cancel and the batch methods are unused. `WindowsRecycleOperationExecutor.cs` (about 820 lines)
  is constructed only by tests, and the Recycle Bin and recovery-review panels in
  `SuperDuper.Windows/Views/PreflightView.xaml` appear only when a stored operation exists or
  loading one failed. The feature is parked (issue #28;
  [ADR-0002](decisions/0002-review-only-windows-app.md)).
- **Unused worker client methods.** `GetRunExclusionsAsync` and `GetPreferenceApplicationAsync`
  have no callers in app code.
- **Unused resources.** `SuperDuper.Windows/Resources/ShellResources.xaml` defines keys nothing
  references: `ShellSpace4` to `ShellSpace24`, `ShellControlCornerRadius`, `ShellCardCornerRadius`,
  `ShellContentInset`, and the `ShellIconSearch`, `ShellIconFilter`, `ShellIconBack` and
  `ShellIconOverflow` geometries. `ShellNavigationItem` is defined there and again in
  `MainWindow.xaml`; the window's copy is the one its tabs resolve, so the shared one is unused.

## Related decisions

- [ADR-0002: The Windows app is review-only](decisions/0002-review-only-windows-app.md)
