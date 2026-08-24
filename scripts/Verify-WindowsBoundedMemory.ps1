[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$timestamp = [DateTimeOffset]::UtcNow.ToString('yyyyMMdd-HHmmss-fff')
$bundle = Join-Path $repo "artifacts/windows-bounded-memory/$timestamp"
$evidencePath = Join-Path $bundle 'warning-scale-evidence.json'
$transcriptPath = Join-Path $bundle 'verifier.log'
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

[void][IO.Directory]::CreateDirectory($bundle)
Start-Transcript -LiteralPath $transcriptPath | Out-Null
Push-Location $repo
try {
    Assert-Contains $operationViewModel 'public bool CanSubmit => false;' `
        'RecycleOperationViewModel.CanSubmit is no longer locked false.'
    Assert-Contains $compositionRoot `
        'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    Assert-Contains $historyView 'Sorting="OnWarningsSorting"' `
        'The run-warning grid no longer delegates sorting to the server-owned query.'
    Assert-Contains $historyView 'VirtualizingPanel.VirtualizationMode="Recycling"' `
        'The bounded warning grid no longer requests recycling virtualization.'
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
        cargo test --release -p super-duper-core --test storage_tests warning_ -- --nocapture
    } 'Focused Release schema-v14 accounting/paging/restart/immutability tests failed.'

    $priorEvidencePath = $env:SUPER_DUPER_WPM13_EVIDENCE_PATH
    try {
        $env:SUPER_DUPER_WPM13_EVIDENCE_PATH = $evidencePath
        Invoke-Checked {
            cargo test --release -p super-duper-core --test storage_tests `
                warning_hundred_thousand_aggregate_release_fixture_stays_bounded `
                -- --ignored --exact --nocapture
        } 'The retained 100,000-aggregate Release fixture failed.'
    }
    finally {
        if ($null -eq $priorEvidencePath) {
            Remove-Item Env:SUPER_DUPER_WPM13_EVIDENCE_PATH -ErrorAction SilentlyContinue
        }
        else {
            $env:SUPER_DUPER_WPM13_EVIDENCE_PATH = $priorEvidencePath
        }
    }
    if (-not (Test-Path -LiteralPath $evidencePath)) {
        throw 'The retained Release fixture did not write its evidence JSON.'
    }
    $evidence = Get-Content -Raw -LiteralPath $evidencePath | ConvertFrom-Json
    if ($evidence.aggregateCount -ne 100000 -or $evidence.fullHistoryMaterialized -or $evidence.executorEnabled) {
        throw 'The retained Release evidence does not prove the exact 100,000-record non-materializing safety boundary.'
    }

    Invoke-Checked {
        cargo test --release -p super-duper-worker `
            warning_protocol_pages_bounded_aggregates_rejects_stale_cursors_and_restarts `
            -- --nocapture
    } 'Focused Release warning protocol sorting/cursor/restart/safety test failed.'
    Invoke-Checked {
        dotnet test $coreTests --configuration Release --filter `
            'FullyQualifiedName~RunHistoryViewModelTests' -m:1
    } 'Focused Release Core cache/binding/sort/cancellation/stale-context tests failed.'
    Invoke-Checked {
        dotnet test $staTests --configuration Release --filter `
            'FullyQualifiedName~ResultsSurfaces_LoadOnStaWithSystemThemeVirtualizationAndAutomationIds' -m:1
    } 'Focused Release loaded-STA warning virtualization/automation/dispatcher test failed.'
    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'

    $manifest = [ordered]@{
        schemaVersion = 1
        gate = 'WPM13-bounded-memory'
        retainedAtUtc = [DateTimeOffset]::UtcNow.ToString('O')
        result = 'passed'
        releaseEvidence = 'warning-scale-evidence.json'
        verifierLog = 'verifier.log'
        providerCampaignsRun = $false
        physicalAccessibilityCampaignsRun = $false
        recycleBinMutationRun = $false
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

Write-Output "WPM13-bounded-memory verifier passed; retained evidence: $bundle"
