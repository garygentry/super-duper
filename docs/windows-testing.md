# Windows test suites

Reference for the Windows app's test projects, what each needs, which tests skip and why, and the
environment variables that only tests read. Build and run commands are in
[`windows-build.md`](windows-build.md).

## Test projects

All three projects are in `apps/windows/SuperDuper.Windows.sln` under `apps/windows/tests/`.

| Project | Target | Test packages | Parallelism | Needs |
|---|---|---|---|---|
| `SuperDuper.Windows.Core.Tests` | `net10.0` | `MSTest` 4.4.1 meta-package | Method level | Nothing outside the process |
| `SuperDuper.Windows.Infrastructure.Tests` | `net10.0-windows10.0.22000.0` | `MSTest` 4.4.1 meta-package | Method level; worker-backed classes are `[DoNotParallelize]` | A built worker for the worker-backed tests |
| `SuperDuper.Windows.Smoke.Tests` | `net10.0-windows10.0.22000.0`, `win-x64`, WPF | `MSTest` 4.4.1 meta-package | One worker (`Workers = 1`) | No worker and no interactive desktop |

Parallel settings are in each project's `MSTestSettings.cs`.

- **Core.Tests** covers view models, validation and display helpers in
  `SuperDuper.Windows.Core` against in-memory test doubles (`TestDoubles.cs`).
- **Infrastructure.Tests** covers the JSONL protocol and progress parsing, worker discovery and
  state locations, the worker client lifecycle, the presentation preferences store, live-hint
  batching and root watching, the single-instance gate, Shell paths, Explorer and Recycle Bin
  services, the Shell operation executor's non-mutating checks, the disabled executor that
  production registers, and the bounded diagnostic log.
- **Smoke.Tests** creates the real `App`, loads `MainWindow` markup and the XAML views on an STA
  thread, and drives them in process with the Core test doubles. Its fixtures cover the shell,
  populated results, Setup, long-scan monitoring and the file filter editor.

Run the solution's test projects serially with `dotnet test ... -m:1`. Running the Smoke.Tests STA
host next to the Infrastructure host can starve WPF dispatcher startup and produce a false UI
timeout.

## Worker-backed tests

Nine Infrastructure tests start a real `super-duper-worker.exe`:

| Class | Tests |
|---|---|
| `SavedScanRepeatTests` | `ReopenedCacheAndEditedSetupCreateNewRunsWithoutRewritingHistory` |
| `WorkerClientLifecycleTests` | `DisposeAsync_WithConcurrentRequestsStopsOwnedWorker`, `DisposeAsync_DuringActiveRunStopsOwnedWorkerAndPersistsCancellation`, `TypedClient_CreatesSessionRunsScanAndObservesDurableCompletion` |
| `WorkerDatabaseUnavailableTests` | `ConnectAsync_ReportsNewerDatabaseWithoutChangingIt`, `ConnectAsync_SecondClientOnSameDatabaseIsInUseAndOwnerKeepsWorking` |
| `WorkerRecoveryTests` | `KilledOwnedWorker_RaisesTypedExitAndSameClientRestartsForNewRun`, `KilledWorker_ReconcilesActiveRunAsInterruptedAfterRestart` |
| `WorkerStateLocationsTests` | `Scan_CreatesMissingDefaultStateDirectoryAndKeepsAllStateThere` |

- Each test searches upward from the test output folder for `target/debug/super-duper-worker.exe`
  in a Debug test run, or `target/release/super-duper-worker.exe` in a Release run.
- If the worker is missing, the test reports **Inconclusive** and the run still passes. Only
  `cargo build` (with `--release` for Release) produces the worker; `cargo test` and
  `cargo clippy` do not.
- These tests run one at a time, after the parallel batch, because several concurrent workers
  starved CI runners.
- `DisposeAsync_DuringActiveRunStopsOwnedWorkerAndPersistsCancellation` also reports Inconclusive
  if the scan finishes before the client is disposed.

## Opt-in real-environment categories

These Infrastructure tests touch the real Recycle Bin or a real cloud provider. In an ordinary run
they report Inconclusive, so a normal run has five Inconclusive results from this section. They
are meant for the dedicated Windows VM, one category per run.

| Category | Tests | Enable with | Effect |
|---|---|---|---|
| `RealCloudProvider` | 1 (`WindowsCloudLocationServiceTests`) | `SUPER_DUPER_EXPECTED_CLOUD_ROOT` | Read-only: registered cloud root detection |
| `RealRecycleBin` | 3 (`WindowsRecycleOperationExecutorTests`) | `SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS=1` | **Mutating**: moves disposable files into the current user's Recycle Bin |
| `RealRecycleBinProvider` | 1 (`WindowsRecycleOperationProviderAcceptanceTests`) | `SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS=1` plus four fixture variables | Read-only: Recycle Bin eligibility inspection of cloud files |

### RealCloudProvider

Set `SUPER_DUPER_EXPECTED_CLOUD_ROOT` to a cloud sync root registered on this PC, for example a
OneDrive folder. The test passes when Windows cloud registration discovery completes and reports
that root. It reads no file content.

```powershell
$env:SUPER_DUPER_EXPECTED_CLOUD_ROOT = "$env:USERPROFILE\OneDrive"
dotnet test apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj --filter "TestCategory=RealCloudProvider"
```

### RealRecycleBin

These tests run the real Shell executor (`WindowsRecycleOperationExecutor`), which production
never registers. Each creates its own files under `%TEMP%\super-duper-recycle-<id>`:

- `ExecuteBatchAsync_RealRecycleBinPreservesHardLinkAndExactFolderSurvivors` recycles one
  hard-link alias and one folder copy, and checks that the other alias and the other folder copy
  are unchanged. The recycled items stay in the current user's Recycle Bin; the test does not
  empty or clean it.
- `ExecuteBatchAsync_CancellationAfterDurableStartStopsBeforeCurrentItem` cancels after the
  operation starts and checks the file is unchanged.
- `ExecuteBatchAsync_LockedFileReturnsStructuredFailureAndLeavesSource` tries to recycle a file
  held open without sharing and expects a `sharing_violation` failure with the file unchanged.

```powershell
$env:SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS = '1'
dotnet test apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj --filter "TestCategory=RealRecycleBin"
Remove-Item Env:SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS
```

### RealRecycleBinProvider

The test inspects two files in a registered cloud root without recycling them. It expects the
locally available file to be eligible and the offline placeholder to be `non_recyclable` with
reason `cloud_placeholder`, and fails if either file's metadata or allocation, or any provider
process's I/O counters, change.

| Variable | Value |
|---|---|
| `SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS` | `1` |
| `SUPER_DUPER_RECYCLE_CLOUD_ROOT` | Absolute path of a registered Cloud Files sync root |
| `SUPER_DUPER_RECYCLE_LOCAL_FILE` | Absolute path of a fully local file inside that root (no offline or recall attributes) |
| `SUPER_DUPER_RECYCLE_OFFLINE_FILE` | Absolute path of an online-only placeholder inside that root, with zero allocated bytes |
| `SUPER_DUPER_RECYCLE_PROVIDER_PROCESSES` | Provider process names without `.exe`, separated by `;`; at least one must be running |

```powershell
$env:SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS = '1'
$env:SUPER_DUPER_RECYCLE_CLOUD_ROOT = "$env:USERPROFILE\OneDrive"
$env:SUPER_DUPER_RECYCLE_LOCAL_FILE = "$env:USERPROFILE\OneDrive\local.txt"
$env:SUPER_DUPER_RECYCLE_OFFLINE_FILE = "$env:USERPROFILE\OneDrive\online-only.txt"
$env:SUPER_DUPER_RECYCLE_PROVIDER_PROCESSES = 'OneDrive'
dotnet test apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj --filter "TestCategory=RealRecycleBinProvider"
```

## WPF surface captures

Smoke.Tests writes PNG captures when a capture variable names a folder. Use a folder under
`artifacts/`. An unset variable writes nothing.

| Variable | Read by | Renders |
|---|---|---|
| `SUPER_DUPER_UIR03_CAPTURES` | `RedesignShellSurfaceTests.cs` | The empty shell at 1180 × 760 and 900 × 600 (`shell-<size>.png`) |
| `SUPER_DUPER_UIR04_CAPTURES` | `SetupWorkflowFixture.cs` | Setup, its advanced options and the prompt for leaving Setup with unsaved changes |
| `SUPER_DUPER_UIR04B_CAPTURES` | `LongScanMonitoringFixture.cs` | Scan progress in stale, terminal and compact states |
| `SUPER_DUPER_UIR05C_CAPTURES` | `PopulatedShellFixture.cs`, `FileQueryLayoutFixture.cs` | Populated Files, Folders, Review, History and Performance views, themes and text scaling, and the file filter editor states |

[`windows-ui-dev-session.md`](windows-ui-dev-session.md) shows how to use them while the desktop is
unavailable.

## Smoke script

`scripts/Invoke-WindowsSmoke.ps1` is not a test project. It drives the real worker over the
protocol and, on an interactive desktop, the real WPF app through UI Automation. Only the script
covers a running app end to end: real keyboard input and focus, Explorer windows, the cloud
fail-closed launch, and app shutdown with the worker it owns. See
[`windows-smoke.md`](windows-smoke.md).

## Test-only environment variables

| Variable | Read by | Purpose |
|---|---|---|
| `SUPER_DUPER_UIR03_CAPTURES`, `SUPER_DUPER_UIR04_CAPTURES`, `SUPER_DUPER_UIR04B_CAPTURES`, `SUPER_DUPER_UIR05C_CAPTURES` | Smoke.Tests | PNG capture folders (above) |
| `SUPER_DUPER_EXPECTED_CLOUD_ROOT` | Infrastructure.Tests | Enables `RealCloudProvider` |
| `SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS` | Infrastructure.Tests | Enables `RealRecycleBin` when `1` |
| `SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS`, `SUPER_DUPER_RECYCLE_CLOUD_ROOT`, `SUPER_DUPER_RECYCLE_LOCAL_FILE`, `SUPER_DUPER_RECYCLE_OFFLINE_FILE`, `SUPER_DUPER_RECYCLE_PROVIDER_PROCESSES` | Infrastructure.Tests | Enable and configure `RealRecycleBinProvider` |
| `SUPER_DUPER_WPM13_EVIDENCE_PATH` | `crates/super-duper-core/tests/storage_tests.rs` | Output file for the ignored `warning_hundred_thousand_aggregate_release_fixture_stays_bounded` scale test |
| `SUPER_DUPER_REVIEW_PROFILE_EVIDENCE` | `crates/super-duper-core/tests/storage_tests.rs` | Output file for the ignored `representative_review_workspace_profile` test |

`SUPER_DUPER_DISABLE_CLOUD_REGISTRATION_DISCOVERY=1` is a diagnostic switch in the app itself, not
a test hook: it makes cloud detection report unavailable so scans cannot start. The smoke script
uses it for its fail-closed launch.

## Where tests run

| Where | What runs |
|---|---|
| CI `build-and-test` (`windows-latest`, every PR and push to `master`) | `cargo fmt --all --check`, `cargo clippy`, `cargo test` and `cargo build` (Debug), then `dotnet build` and `dotnet test --no-build -m:1` for all three projects. The worker-backed tests run; the opt-in categories report Inconclusive. |
| CI `release-package` (same triggers) | `Verify-WindowsRelease.ps1 -SkipWpfSmoke`: Release Rust and .NET tests, publish, package, and the smoke script's protocol half |
| Dedicated Windows VM | `Verify-WindowsRelease.ps1` with the WPF smoke, the opt-in real-environment categories, and anything that needs the interactive desktop |

CI is defined in `.github/workflows/ci.yml`. Release gates are listed in
[`release-checklist.md`](release-checklist.md).
