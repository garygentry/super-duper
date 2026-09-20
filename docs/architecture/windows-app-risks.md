# Windows app risks and technical debt

## Purpose

This chapter lists known weaknesses in the Windows app so you can weigh them before changing
nearby code. Every item is a finding from reading the source, with the file that shows it; none was
reproduced at run time for this page. Where a GitHub issue already tracks an item, it is named.
Paths are relative to `apps/windows/src/` unless they start with another top-level folder.

## Reliability

- **No process-wide exception handler.** Nothing subscribes to `DispatcherUnhandledException`,
  `AppDomain.UnhandledException` or `TaskScheduler.UnobservedTaskException`. The app has 24
  `async void` methods: `App.OnStartup` and view event handlers in
  `SuperDuper.Windows/Views/DuplicateFilesView.xaml.cs`, `DuplicateFoldersView.xaml.cs` and
  `RunHistoryView.xaml.cs`. An exception escaping one of them reaches WPF's default handling and
  ends the app without the orderly path in `MainWindow.ShutdownAsync`.
- **No per-request timeout.** Only the `hello` handshake (10 seconds) and the shutdown grace
  (2 seconds) are timed. A request the worker never answers waits until its caller cancels or the
  connection fails (`SuperDuper.Windows.Infrastructure/WorkerClient.cs`, `SendRequestAsync`).
- **Cancelling a request only stops waiting.** `ResponseCorrelator.TryCancel` completes the
  caller's task but leaves the pending entry until the worker answers or the connection fails, and
  the worker is not told to stop that work
  (`SuperDuper.Windows.Infrastructure/Protocol/ResponseCorrelator.cs`).
- **Event subscribers can stop the worker.** `WorkerClient.DispatchEvent` invokes subscribers on
  the stdout pump thread inside its error handling, so an exception from any subscriber is treated
  as a protocol failure: pending requests fail and the worker is killed.
- **Setup validation touches the file system on the UI thread.** `SessionDefinitionValidator`
  (`SuperDuper.Windows.Core/Validation/SessionDefinitionValidator.cs`) reads each root's drive type
  with `DriveInfo` and calls `Directory.Exists` for local and removable roots each time Setup is
  validated. The existence check is skipped for network roots for this reason, but a slow removable
  or failing local drive could still stall the window.
- **Error mapping by message text.** `DuplicateFoldersViewModel.ReviewDecisionError`
  (`SuperDuper.Windows.Core/ViewModels/DuplicateFoldersViewModel.cs`) recognizes worker error codes
  such as `review_overlap_conflict` by searching the exception message. The code is available as
  `WorkerProtocolException.Code`, but that type lives in Infrastructure, which Core cannot
  reference. A change to the message format would silently fall through to the generic text.

## Usability defects

- **Duplicate access keys.** In `SuperDuper.Windows/MainWindow.xaml`, **Scan** and **Setup** share
  S, and **Results** and **Review** share R. In `SuperDuper.Windows/Views/RunHistoryView.xaml`,
  **Refresh** and **Refresh current warnings** share R, **Next scans** and **Next warning page**
  share N, and **Previous scans** and **Performance details** share P. WPF moves focus between
  duplicates instead of activating one. Not tested at run time.
- **Promised shortcuts that do not exist.** The previous and next duplicate-set buttons in
  `SuperDuper.Windows/Views/DuplicateFilesView.xaml` announce Alt+P and Alt+N in their tooltips and
  accessible names, but no access key or key binding backs them; the view registers only "v"
  (`DuplicateFilesView.xaml.cs`).
- **Stale set counts while loading.** When the selected duplicate set changes,
  `DuplicateFilesViewModel.SetSelectedGroup` keeps the previous set's review summary until the new
  member page arrives, so the counts briefly describe the wrong set (issue #25).

## Diagnostics

- **The worker log ignores state overrides.** `WorkerClient` always writes worker stderr to
  `%LOCALAPPDATA%\SuperDuper\logs\worker.log`, because `App.xaml.cs` passes no log path and
  `DefaultDiagnosticLogPath` does not consult `WorkerStateLocations`. Disposable and UI-dev runs
  with `SUPER_DUPER_DB_PATH` set therefore log into the default location.
- **`LOG_FILE_PATH` has no effect on the app.** The `.uidev` sidecar
  (`SuperDuper.Windows/App.xaml.cs`) and `scripts/Start-WindowsUiDev.ps1` set it, but only the
  CLI reads it (`crates/super-duper-cli/src/logging.rs`).
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
