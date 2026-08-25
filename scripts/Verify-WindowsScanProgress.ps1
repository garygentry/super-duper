[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$coreTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj'
$infrastructureTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj'
$staTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj'
$workerSource = Join-Path $repo 'crates/super-duper-worker/src/lib.rs'
$workerProgressSource = Join-Path $repo 'crates/super-duper-worker/src/progress_projection.rs'
$hasherSource = Join-Path $repo 'crates/super-duper-core/src/hasher/xxhash.rs'
$applicationGate = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/LatestProgressApplicationGate.cs'
$progressViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/ScanProgressViewModel.cs'
$progressView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/ScanProgressView.xaml'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$preflightView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/PreflightView.xaml'
$smokeScript = Join-Path $repo 'scripts/Invoke-WindowsSmoke.ps1'

function Assert-Contains([string]$Path, [string]$Text, [string]$Failure) {
    if (-not [IO.File]::ReadAllText($Path).Contains($Text, [StringComparison]::Ordinal)) {
        throw $Failure
    }
}

function Invoke-Checked([scriptblock]$Command, [string]$Failure) {
    & $Command
    if ($LASTEXITCODE -ne 0) { throw $Failure }
}

function Assert-PowerShellParses([string]$Path) {
    $tokens = $null
    $errors = $null
    [void][Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$errors)
    if ($errors.Count -ne 0) {
        throw "PowerShell parsing failed for $Path`: $($errors -join '; ')"
    }
}

Push-Location $repo
try {
    Assert-Contains $hasherSource 'pub(crate) const HASH_PROGRESS_FILE_QUANTUM: u64 = 256;' `
        'The producer 256-file progress quantum changed.'
    Assert-Contains $hasherSource 'pub(crate) const FULL_READ_PROGRESS_BYTE_QUANTUM: u64 = 8 * 1024 * 1024;' `
        'The producer 8 MiB full-read progress quantum changed.'
    Assert-Contains $workerSource 'const EVENT_INTERVAL: Duration = Duration::from_millis(100);' `
        'The worker ten-per-second progress interval changed.'
    Assert-Contains $workerProgressSource `
        'pub(crate) const PROGRESS_EVENT_INTERVAL_NANOS: u64 = 100_000_000;' `
        'The worker progress coalescer ten-per-second interval changed.'
    Assert-Contains $applicationGate 'TimeSpan.FromMilliseconds(100)' `
        'The Core ten-per-second progress application interval changed.'
    Assert-Contains $progressViewModel 'ProgressAnnouncementIntervalNanos = 5_000_000_000' `
        'The accepted five-second UI Automation announcement interval changed.'
    Assert-Contains $progressView 'AutomationId="ScanProgressFunnel"' `
        'The bounded six-stage progress surface is missing.'
    Assert-Contains $progressView 'AutomationId="ScanProgressAnnouncement"' `
        'The coalesced progress announcement surface is missing.'
    if ([IO.File]::ReadAllText($progressView).Contains('Files hashed', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The deprecated ambiguous Files hashed display returned.'
    }
    Assert-Contains $compositionRoot `
        'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    if ([IO.File]::ReadAllText($preflightView).Contains('Move to Recycle Bin now', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The forbidden Move to Recycle Bin now action is present.'
    }

    [void][xml]([IO.File]::ReadAllText($progressView))
    Assert-PowerShellParses $smokeScript
    Assert-PowerShellParses $PSCommandPath

    Invoke-Checked {
        cargo test -p super-duper-core --lib `
            hasher::xxhash::tests::large_bucket_publishes_partial_progress_before_bucket_resolution `
            -- --exact --nocapture
    } 'Focused mid-bucket progress test failed.'
    Invoke-Checked {
        cargo test -p super-duper-core --lib `
            hasher::xxhash::tests::long_full_read_publishes_physical_bytes_before_completion `
            -- --exact --nocapture
    } 'Focused mid-read progress test failed.'
    Invoke-Checked {
        cargo test -p super-duper-core --lib `
            hasher::xxhash::tests::cache_and_read_outcomes_reconcile_without_mixing_logical_and_physical_bytes `
            -- --exact --nocapture
    } 'Focused logical/physical/cache reconciliation test failed.'
    Invoke-Checked {
        cargo test -p super-duper-core --lib `
            hasher::xxhash::tests::observed_stream_retains_bytes_before_cancellation `
            -- --exact --nocapture
    } 'Focused cancellation observation-silence test failed.'
    Invoke-Checked {
        cargo test -p super-duper-core --test e2e_pipeline_tests `
            test_full_scan_pipeline -- --exact --nocapture
    } 'Focused completed live-to-durable reconciliation test failed.'
    Invoke-Checked {
        cargo test -p super-duper-core --test e2e_pipeline_tests `
            test_scan_cancellation -- --exact --nocapture
    } 'Focused cancellation and post-cancel silence test failed.'
    Invoke-Checked {
        cargo test -p super-duper-core --test e2e_pipeline_tests `
            test_pipeline_failure_is_persisted_as_failed -- --exact --nocapture
    } 'Focused failed-run live-to-durable reconciliation test failed.'
    Invoke-Checked {
        cargo test -p super-duper-worker `
            progress_projection::tests::thousand_updates_are_latest_wins_and_bounded_in_every_half_open_second `
            -- --exact --nocapture
    } 'Focused worker 1,000-update transport-bound test failed.'
    Invoke-Checked {
        cargo test -p super-duper-worker `
            tests::delayed_latest_progress_emits_without_another_callback_and_stops_at_terminal `
            -- --exact --nocapture
    } 'Focused delayed-latest and terminal-suppression worker test failed.'
    Invoke-Checked {
        cargo test -p super-duper-worker `
            tests::completed_run_emits_ordered_coalesced_progress_before_matching_terminal_state `
            -- --exact --nocapture
    } 'Focused real worker progress/terminal-order test failed.'
    Invoke-Checked {
        cargo test -p super-duper-worker `
            tests::second_start_is_busy_and_cancellation_reaches_durable_cancelled_state `
            -- --exact --nocapture
    } 'Focused real worker cancellation/sticky-terminal test failed.'
    Invoke-Checked { cargo build -p super-duper-worker } `
        'Paired Debug worker build failed.'
    Invoke-Checked {
        dotnet test $infrastructureTests --configuration Debug --filter `
            'FullyQualifiedName~WorkerRunProgressParserTests' -m:1
    } 'Focused strict typed-progress client parser tests failed.'
    Invoke-Checked {
        dotnet test $coreTests --configuration Debug --filter `
            'FullyQualifiedName~LatestProgressApplicationGateTests|FullyQualifiedName~ScanProgressViewModelTests|FullyQualifiedName~ThousandProgressFramesQueueOneDispatcherApplicationAndPreserveLatest|FullyQualifiedName~TerminalLifecycleInvalidatesAlreadyPostedProgressBeforeUiExecution' -m:1
    } 'Focused Core application/projection/cancellation/stale-bound tests failed.'
    Invoke-Checked {
        dotnet test $staTests --configuration Debug --filter `
            'FullyQualifiedName~ResultsSurfaces_LoadOnStaWithSystemThemeVirtualizationAndAutomationIds' -m:1
    } 'Focused loaded-STA progress accessibility/focus/announcement test failed.'

    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'
}
finally {
    Pop-Location
}

Write-Output 'SOP2 scan-progress verifier passed with mid-bucket/read advancement, exact live/durable meanings, cancellation and terminal silence, worker/Core bounds, and the accessible WPF projection.'
