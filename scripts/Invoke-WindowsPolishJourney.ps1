[CmdletBinding()]
param([ValidateSet('Debug', 'Release')][string]$Configuration = 'Debug', [switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$project = Join-Path $repository 'apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj'
$workerDirectory = if ($Configuration -eq 'Release') { 'release' } else { 'debug' }
$worker = Join-Path $repository "target/$workerDirectory/super-duper-worker.exe"
# Keep VM compiler concurrency bounded. Ignoring persistent .NET build servers also preserves useful compiler diagnostics.
if (-not $SkipBuild) {
    $cargoArguments = @('build', '--locked', '--jobs', '2', '-p', 'super-duper-worker')
    if ($Configuration -eq 'Release') { $cargoArguments += '--release' }
    Push-Location $repository
    try { & cargo @cargoArguments; if ($LASTEXITCODE -ne 0) { throw 'Worker build failed.' } }
    finally { Pop-Location }
    & dotnet build $project --configuration $Configuration --disable-build-servers -m:1
    if ($LASTEXITCODE -ne 0) { throw 'WPF journey build failed.' }
}
if (-not (Test-Path -LiteralPath $worker)) { throw "Matching worker is missing: $worker" }
$corpusOutput = @(& (Join-Path $PSScriptRoot 'New-WindowsPolishCorpus.ps1'))
$corpus = ($corpusOutput | Where-Object { $_ -like 'CORPUS=*' }).Substring(7)
$state = Join-Path $repository ('artifacts/ui-dev-session/polish-journey-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $state | Out-Null
$settings = @{
    SUPER_DUPER_POLISH_CORPUS = $corpus
    SUPER_DUPER_POLISH_STATE = $state
    SUPER_DUPER_POLISH_WORKER = $worker
    SUPER_DUPER_DB_PATH = (Join-Path $state 'super_duper.db')
    SUPER_DUPER_STATUS_DB_PATH = (Join-Path $state 'scan_status.db')
    HASH_CACHE_PATH = (Join-Path $state 'hash-cache')
    LOG_FILE_PATH = (Join-Path $state 'app.log')
}
$previous = @{}
try {
    foreach ($key in $settings.Keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key); [Environment]::SetEnvironmentVariable($key, $settings[$key]) }
    Write-Output "CORPUS=$corpus"
    Write-Output "EVIDENCE=$state"
    & dotnet test $project --configuration $Configuration --no-build --disable-build-servers -m:1 --filter 'FullyQualifiedName~RealWorkerPolishJourneyTests' --logger 'trx;LogFileName=journey.trx' --results-directory $state --blame-hang-timeout 5m
    if ($LASTEXITCODE -ne 0) { throw "Real-worker WPF journey failed. Evidence: $state" }
}
finally { foreach ($key in $previous.Keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key]) } }
