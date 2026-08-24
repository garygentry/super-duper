[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$coreTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj'
$infrastructureTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj'
$staTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj'
$operationViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$filesView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/DuplicateFilesView.xaml'
$preflightView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/PreflightView.xaml'
$workerSource = Join-Path $repo 'crates/super-duper-worker/src/lib.rs'
$batcherSource = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Infrastructure/ReviewLiveHintBatcher.cs'
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
    Assert-Contains $batcherSource 'TimeSpan.FromMilliseconds(100)' `
        'The accepted at-most-ten-live-updates-per-second interval changed.'
    Assert-Contains $batcherSource 'MaximumPathsPerBatch = 200' `
        'The bounded 200-path hint batch changed.'
    Assert-Contains $filesView 'AutomationId="FileLiveHintStatus"' `
        'The stable coalesced live-hint automation status is missing.'
    if ([IO.File]::ReadAllText($preflightView).Contains('Move to Recycle Bin now', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The forbidden Move to Recycle Bin now action is present.'
    }
    if ([IO.File]::ReadAllText($workerSource).Contains('"executorEnabled": true', [StringComparison]::Ordinal)) {
        throw 'A worker response reports executorEnabled:true.'
    }

    [void][xml]([IO.File]::ReadAllText($filesView))
    Assert-PowerShellParses $smokeScript
    Assert-PowerShellParses $PSCommandPath

    Invoke-Checked {
        cargo test -p super-duper-core --test storage_tests `
            live_hint_burst_resolves_one_bounded_read_without_mutating_history -- --exact --nocapture
    } 'Focused bounded live-hint storage/history test failed.'
    Invoke-Checked {
        cargo test -p super-duper-worker `
            watcher_overflow_protocol_is_durable_visible_bounded_and_generation_bound -- --nocapture
    } 'Focused batched-hint/overflow/restart worker protocol test failed.'
    Invoke-Checked {
        dotnet test $infrastructureTests --configuration Debug --filter `
            'FullyQualifiedName~ReviewLiveHintBatcherTests' -m:1
    } 'Focused deterministic burst/batching/rate-bound/overflow tests failed.'
    Invoke-Checked {
        dotnet test $coreTests --configuration Debug --filter `
            'FullyQualifiedName~CoalescedLiveHintsProduceOneCacheBindingAndRejectStaleContext|FullyQualifiedName~OneCoalescedWorkerFrameProducesOneDispatcherUpdate|FullyQualifiedName~DirtyRootReconstructionReconcilesOneBoundedBatchAndPreservesMemberCursor|FullyQualifiedName~DirtyRootCancellationAndLateResponseCannotReplaceNewerRunContext|FullyQualifiedName~VisiblePageValidationBindsOnlyCurrentPageAndInvalidatesWorkingChoices|FullyQualifiedName~ValidationCancellationAndLateResponseCannotReplaceNewerContext' -m:1
    } 'Focused Core coalescing/overflow/cancellation/stale-context tests failed.'
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

Write-Output 'WPM12-event-coalescing verifier passed with bounded mass-event frames/cache/bindings, at most ten UI updates per second, overflow fallback, immutable history, stale-context rejection, and production execution disabled.'
