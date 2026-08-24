[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$coreTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj'
$staTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj'
$operationViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$filesView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/DuplicateFilesView.xaml'
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
    if ($errors.Count -ne 0) {
        throw "PowerShell parsing failed for $Path`: $($errors -join '; ')"
    }
}

Push-Location $repo
try {
    Assert-Contains $operationViewModel 'public bool CanSubmit => false;' `
        'RecycleOperationViewModel.CanSubmit is no longer locked false.'
    Assert-Contains $compositionRoot `
        'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    Assert-Contains $filesView 'AutomationId="FileDirtyRootWarning"' `
        'The stable visible dirty-root automation ID is missing.'
    Assert-Contains $filesView 'AutomationId="FileReconcileDirtyRoot"' `
        'The stable bounded reconciliation automation ID is missing.'
    Assert-Contains $filesView 'AutomationId="FileCancelDirtyRootReconciliation"' `
        'The stable dirty-root cancellation automation ID is missing.'
    if ([IO.File]::ReadAllText($preflightView).Contains('Move to Recycle Bin now', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The forbidden Move to Recycle Bin now action is present.'
    }
    if ([IO.File]::ReadAllText($workerSource).Contains('"executorEnabled": true', [StringComparison]::Ordinal)) {
        throw 'A worker response reports executorEnabled:true.'
    }

    [void][xml]([IO.File]::ReadAllText($filesView))
    Assert-PowerShellParses $smokeScript
    Assert-PowerShellParses $PSCommandPath

    Invoke-Checked { cargo test -p super-duper-core root_reconciliation -- --nocapture } `
        'Focused dirty-root storage/restart/bounded-reconciliation tests failed.'
    Invoke-Checked {
        cargo test -p super-duper-core version_twelve_migrates_dirty_root_reconciliation_transactionally -- --nocapture
    } 'Focused schema-v13 migration test failed.'
    Invoke-Checked {
        cargo test -p super-duper-worker watcher_overflow_protocol_is_durable_visible_bounded_and_generation_bound -- --nocapture
    } 'Focused watcher-overflow protocol test failed.'
    Invoke-Checked {
        dotnet test $coreTests --configuration Debug --filter `
            'FullyQualifiedName~DirtyRootReconstructionReconcilesOneBoundedBatchAndPreservesMemberCursor|FullyQualifiedName~DirtyRootCancellationAndLateResponseCannotReplaceNewerRunContext|FullyQualifiedName~VisiblePageValidationBindsOnlyCurrentPageAndInvalidatesWorkingChoices|FullyQualifiedName~ValidationCancellationAndLateResponseCannotReplaceNewerContext' -m:1
    } 'Focused Core dirty-state/bounds/cancellation/stale-context tests failed.'
    Invoke-Checked {
        dotnet test $staTests --configuration Debug --filter `
            'FullyQualifiedName~ResultsSurfaces_LoadOnStaWithSystemThemeVirtualizationAndAutomationIds' -m:1
    } 'Focused loaded-STA dispatcher/automation/focus test failed.'

    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'
}
finally {
    Pop-Location
}

Write-Output 'WPM12-watcher-overflow verifier passed with durable visible dirty state, bounded explicit reconciliation, immutable history, and production execution disabled.'
