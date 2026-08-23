[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$solution = Join-Path $repo 'apps/windows/SuperDuper.Windows.sln'
$operationViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$preflightView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/PreflightView.xaml'

function Assert-Contains([string]$Path, [string]$Text, [string]$Failure) {
    $content = [IO.File]::ReadAllText($Path)
    if (-not $content.Contains($Text, [StringComparison]::Ordinal)) {
        throw $Failure
    }
}

Push-Location $repo
try {
    Assert-Contains $operationViewModel 'public bool CanSubmit => false;' `
        'RecycleOperationViewModel.CanSubmit is no longer locked false.'
    Assert-Contains $compositionRoot `
        'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    Assert-Contains $preflightView `
        'Record append-only operator observation; original evidence remains unchanged' `
        'The append-only observation automation contract is missing.'
    Assert-Contains $preflightView `
        'Navigate to start a fresh scan; do not retry this operation' `
        'The fresh-scan/non-retry automation contract is missing.'
    if ([IO.File]::ReadAllText($preflightView).Contains('Move to Recycle Bin now', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'The forbidden Move to Recycle Bin now action is present.'
    }

    dotnet build $solution --configuration Debug -m:1
    if ($LASTEXITCODE -ne 0) { throw 'Debug solution build failed.' }
    dotnet test $solution --configuration Debug --no-build -m:1
    if ($LASTEXITCODE -ne 0) { throw 'Serialized Debug solution tests failed.' }

    & (Join-Path $PSScriptRoot 'Verify-WindowsRelease.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'Release verification and real non-mutating smoke failed.' }
}
finally {
    Pop-Location
}

Write-Output 'WPM11-recovery-review-ui verifier passed with production execution disabled.'
