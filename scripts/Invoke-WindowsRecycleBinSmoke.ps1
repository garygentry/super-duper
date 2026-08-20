[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Release',
    [switch]$ConfirmRecycleBinMutation,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

if (-not $ConfirmRecycleBinMutation) {
    throw 'This acceptance moves uniquely named disposable fixtures to the current user Recycle Bin. Rerun with -ConfirmRecycleBinMutation.'
}

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$project = Join-Path $repoRoot 'apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj'
if (-not (Test-Path -LiteralPath $project -PathType Leaf)) {
    throw "Infrastructure test project was not found: $project"
}

Push-Location $repoRoot
try {
    if (-not $SkipBuild) {
        dotnet build $project -c $Configuration --no-restore
        if ($LASTEXITCODE -ne 0) {
            throw "Infrastructure test build failed with exit code $LASTEXITCODE."
        }
    }

    $env:SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS = '1'
    dotnet test $project -c $Configuration --no-build --no-restore --filter 'TestCategory=RealRecycleBin'
    if ($LASTEXITCODE -ne 0) {
        throw "Real Recycle Bin acceptance failed with exit code $LASTEXITCODE."
    }
    Write-Output 'Disposable real Recycle Bin acceptance passed.'
    Write-Output 'Successful fixtures remain recoverable in the current user Recycle Bin; the test does not implement permanent cleanup.'
}
finally {
    Remove-Item Env:SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS -ErrorAction SilentlyContinue
    Pop-Location
}
