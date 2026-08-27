[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$coreTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj'
$infrastructureTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj'
$staTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj'
$performanceView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/PerformanceView.xaml'
$performanceViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/PerformanceViewModel.cs'
$operationViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$workerSource = Join-Path $repo 'crates/super-duper-worker/src/lib.rs'

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
    if ($errors.Count -ne 0) { throw "PowerShell parsing failed for $Path`: $($errors -join '; ')" }
}

Push-Location $repo
try {
    Assert-Contains $operationViewModel 'public bool CanSubmit => false;' `
        'RecycleOperationViewModel.CanSubmit is no longer locked false.'
    Assert-Contains $compositionRoot `
        'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    Assert-Contains $performanceViewModel 'public const int HistoryLimit = 25;' `
        'The Core performance-history binding is not capped at 25 rows.'
    Assert-Contains $performanceViewModel 'public const int DeviceLimit = 64;' `
        'The Core performance-device binding is not capped at 64 rows.'
    Assert-Contains $performanceView 'AutomationId="PerformanceHistoryGrid"' `
        'The bounded performance-history automation surface is missing.'
    Assert-Contains $performanceView 'AutomationId="PerformanceDeviceGrid"' `
        'The bounded device automation surface is missing.'
    Assert-Contains $performanceView 'SystemColors.ActiveBorderBrushKey' `
        'The Performance surface does not use the required system border brush.'
    Assert-Contains $performanceView 'NotificationProcessing="MostRecent"' `
        'The Performance surface does not coalesce UI Automation notifications latest-only.'
    if ([IO.File]::ReadAllText($workerSource).Contains('"executorEnabled": true', [StringComparison]::Ordinal)) {
        throw 'A worker response reports executorEnabled:true.'
    }

    [void][xml]([IO.File]::ReadAllText($performanceView))
    Assert-PowerShellParses $PSCommandPath

    Invoke-Checked {
        cargo test -p super-duper-core --test telemetry_tests status_queries_use_stable_bounded_cursors_and_fixed_summaries
    } 'Focused status-summary persistence and unavailable-value tests failed.'
    Invoke-Checked {
        cargo test -p super-duper-worker performance_queries_are_bounded_persisted_and_execution_disabled
    } 'Focused bounded worker performance protocol/restart tests failed.'
    Invoke-Checked { cargo build -p super-duper-worker } `
        'The worker executable required by typed-client verification did not build.'
    Invoke-Checked {
        dotnet test $coreTests --configuration Debug --filter 'FullyQualifiedName~PerformanceViewModelTests' -m:1
    } 'Focused Core bounds, unavailable-state, comparison, and restart tests failed.'
    Invoke-Checked {
        dotnet test $infrastructureTests --configuration Debug --filter `
            'FullyQualifiedName~TypedClient_CreatesSessionRunsScanAndObservesDurableCompletion' -m:1
    } 'Focused typed-client persisted performance-query verification failed.'
    Invoke-Checked {
        dotnet test $staTests --configuration Debug --filter `
            'FullyQualifiedName~ResultsSurfaces_LoadOnStaWithSystemThemeVirtualizationAndAutomationIds' -m:1
    } 'Focused loaded-STA bounds, focus, automation, system-brush, and announcement tests failed.'
    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'
}
finally {
    Pop-Location
}

Write-Output 'SOP4 Performance-tab verifier passed bounded read-only status queries, live/history projection, unavailable truth, comparison/restart continuity, representative UI bounds, keyboard/focus/automation, system brushes, and production locks.'
