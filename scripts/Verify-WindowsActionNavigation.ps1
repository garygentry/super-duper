[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$timestamp = [DateTimeOffset]::UtcNow.ToString('yyyyMMdd-HHmmss-fff')
$bundle = Join-Path $repo "artifacts/windows-action-navigation/$timestamp"
$transcriptPath = Join-Path $bundle 'verifier.log'
$coreTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj'
$staTests = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj'
$operationViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$historyViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RunHistoryViewModel.cs'
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

[void][IO.Directory]::CreateDirectory($bundle)
Start-Transcript -LiteralPath $transcriptPath | Out-Null
Push-Location $repo
try {
    Assert-Contains $operationViewModel 'public bool CanSubmit => false;' `
        'RecycleOperationViewModel.CanSubmit is no longer locked false.'
    Assert-Contains $compositionRoot `
        'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    Assert-Contains $historyViewModel `
        'public const string HashWarningCode = "hash_recoverable_warning";' `
        'The action is no longer bound to the single admitted hash warning family.'
    Assert-Contains $historyView 'RunWarningHashResults-{0}' `
        'Hash warning navigation lost its stable aggregate-scoped automation ID.'
    Assert-Contains $historyView 'Cancel opening immutable duplicate-file results' `
        'Discovery warning navigation no longer exposes explicit cancellation.'
    if ([IO.File]::ReadAllText($preflightView).Contains('Move to Recycle Bin now', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The forbidden Move to Recycle Bin now action is present.'
    }
    if ([IO.File]::ReadAllText($workerSource).Contains('"executorEnabled": true', [StringComparison]::Ordinal)) {
        throw 'A worker response reports executorEnabled:true.'
    }

    [void][xml]([IO.File]::ReadAllText($historyView))
    Assert-PowerShellParses $smokeScript
    Assert-PowerShellParses $PSCommandPath

    Invoke-Checked {
        cargo test -p super-duper-worker `
            warning_protocol_pages_bounded_aggregates_rejects_stale_cursors_and_restarts `
            -- --nocapture
    } 'Focused warning stable-target protocol test failed.'
    Invoke-Checked {
        dotnet test $coreTests --configuration Release --filter `
            'FullyQualifiedName~RunHistoryViewModelTests|FullyQualifiedName~ShellViewModelTests' -m:1
    } 'Focused Core navigation/cancellation/stale-context tests failed.'
    Invoke-Checked {
        dotnet test $staTests --configuration Release --filter `
            'FullyQualifiedName~ResultsSurfaces_LoadOnStaWithSystemThemeVirtualizationAndAutomationIds' -m:1
    } 'Focused loaded-WPF automation/dispatcher/focus test failed.'
    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'

    $manifest = [ordered]@{
        schemaVersion = 1
        gate = 'WPM13-action-navigation'
        retainedAtUtc = [DateTimeOffset]::UtcNow.ToString('O')
        result = 'passed'
        warningFamily = 'scan/hash_recoverable_warning'
        targetKind = 'immutable_duplicate_file_run'
        verifierLog = 'verifier.log'
        providerCampaignsRun = $false
        physicalAccessibilityCampaignsRun = $false
        recycleBinMutationRun = $false
        outcomeAuditRun = $false
        broadPerformanceRun = $false
        laterGateCampaignsRun = $false
    }
    [IO.File]::WriteAllText(
        (Join-Path $bundle 'manifest.json'),
        ($manifest | ConvertTo-Json -Depth 4) + [Environment]::NewLine)
}
finally {
    Pop-Location
    Stop-Transcript | Out-Null
}

Write-Output "WPM13-action-navigation verifier passed; retained evidence: $bundle"
