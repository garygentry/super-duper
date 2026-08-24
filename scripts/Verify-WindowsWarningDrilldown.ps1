[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$coreTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj'
$staTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj'
$operationViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$historyView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/RunHistoryView.xaml'
$preflightView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/PreflightView.xaml'
$workerSource = Join-Path $repo 'crates/super-duper-worker/src/lib.rs'
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
    if ($errors.Count -ne 0) { throw "PowerShell parsing failed for $Path`: $($errors -join '; ')" }
}

Push-Location $repo
try {
    Assert-Contains $operationViewModel 'public bool CanSubmit => false;' `
        'RecycleOperationViewModel.CanSubmit is no longer locked false.'
    Assert-Contains $compositionRoot `
        'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    Assert-Contains $historyView 'AutomationId="RunWarningGrid"' `
        'The stable run-warning drilldown automation grid is missing.'
    Assert-Contains $historyView 'AutomationId="CancelRunWarningLoad"' `
        'The warning-page cancellation action is missing.'
    if ([IO.File]::ReadAllText($preflightView).Contains('Move to Recycle Bin now', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The forbidden Move to Recycle Bin now action is present.'
    }
    if ([IO.File]::ReadAllText($workerSource).Contains('"executorEnabled": true', [StringComparison]::Ordinal)) {
        throw 'A worker response reports executorEnabled:true.'
    }

    [void][xml]([IO.File]::ReadAllText($historyView))
    Assert-PowerShellParses $smokeScript
    Assert-PowerShellParses $PSCommandPath

    Invoke-Checked { cargo test -p super-duper-core --test storage_tests warning_ -- --nocapture } `
        'Focused schema-v14 migration/persistence/paging/restart/immutability tests failed.'
    Invoke-Checked {
        cargo test -p super-duper-core --test e2e_pipeline_tests `
            files_changed_or_removed_after_discovery_become_warnings_not_false_results -- --exact --nocapture
    } 'Focused end-to-end aggregate source test failed.'
    Invoke-Checked {
        cargo test -p super-duper-worker `
            warning_protocol_pages_bounded_aggregates_rejects_stale_cursors_and_restarts -- --nocapture
    } 'Focused warning protocol cursor/restart/safety test failed.'
    Invoke-Checked {
        dotnet test $coreTests --configuration Debug --filter `
            'FullyQualifiedName~RunHistoryViewModelTests' -m:1
    } 'Focused Core warning binding/cache/cancellation/stale-context tests failed.'
    Invoke-Checked {
        dotnet test $staTests --configuration Debug --filter `
            'FullyQualifiedName~ResultsSurfaces_LoadOnStaWithSystemThemeVirtualizationAndAutomationIds' -m:1
    } 'Focused loaded-STA warning automation/dispatcher test failed.'
    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'
}
finally {
    Pop-Location
}

Write-Output 'WPM13-warning-drilldown verifier passed with exact bounded aggregates/examples, opaque paging, restart reconstruction, immutable history, stale-context rejection, accessibility, dispatcher responsiveness, and production execution disabled.'
