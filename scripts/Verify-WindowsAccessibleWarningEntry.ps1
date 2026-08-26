[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$coreTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj'
$staTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj'
$progressView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/ScanProgressView.xaml'
$historyView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/RunHistoryView.xaml'
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
    Assert-Contains $progressView 'AutomationId="ProgressWarningEntry"' `
        'The Progress warning entry automation ID is missing.'
    Assert-Contains $progressView 'AutomationProperties.AccessKey="Alt+W"' `
        'The Progress warning entry access key is missing.'
    Assert-Contains $historyView 'AutomationId="RunWarningDiagnosticLog"' `
        'The separate diagnostic application-log surface is missing.'
    Assert-Contains $historyView 'this log is not durable warning truth' `
        'The diagnostic application log is not explicitly separated from durable warning truth.'
    Assert-Contains $historyView 'SystemColors.ActiveBorderBrushKey' `
        'The warning surface does not use the required system border brush.'
    if ([IO.File]::ReadAllText($workerSource).Contains('"executorEnabled": true', [StringComparison]::Ordinal)) {
        throw 'A worker response reports executorEnabled:true.'
    }

    [void][xml]([IO.File]::ReadAllText($progressView))
    [void][xml]([IO.File]::ReadAllText($historyView))
    Assert-PowerShellParses $PSCommandPath

    Invoke-Checked {
        dotnet test $coreTests --configuration Debug --filter `
            'FullyQualifiedName~CurrentWarningEntryUsesExactRunContextAndAccessibleCount|FullyQualifiedName~ProgressWarningEntryReusesBoundedHistoryAcrossTerminalAndRestart|FullyQualifiedName~RunWarningDrilldownViewModelTests|FullyQualifiedName~WarningDrilldownPagesBoundsCacheAndRestoresFocus' -m:1
    } 'Focused cross-layer current-warning accounting, terminal/restart, and cache-bound tests failed.'
    Invoke-Checked {
        dotnet test $staTests --configuration Debug --filter `
            'FullyQualifiedName~ResultsSurfaces_LoadOnStaWithSystemThemeVirtualizationAndAutomationIds' -m:1
    } 'Focused loaded-STA warning entry, focus, automation, system-brush, and announcement tests failed.'
    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'
}
finally {
    Pop-Location
}

Write-Output 'SOP3d accessible-warning-entry verifier passed with exact accounting, bounded current/history reuse, diagnostic-log separation, terminal/restart continuity, keyboard/focus/automation, system brushes, and coalesced announcements.'
