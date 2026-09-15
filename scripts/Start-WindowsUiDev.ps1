[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Debug',
    [switch]$SkipBuild,
    [switch]$CreateFixture,
    [string]$StateDirectory
)

$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$profile = if ($Configuration -eq 'Release') { 'release' } else { 'debug' }
$app = Join-Path $repository "apps/windows/src/SuperDuper.Windows/bin/$Configuration/net10.0-windows10.0.22000.0/win-x64/SuperDuper.Windows.exe"
$worker = Join-Path $repository "target/$profile/super-duper-worker.exe"

if (-not $SkipBuild) {
    $cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    $cargoExecutable = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe' }
    if (-not (Test-Path -LiteralPath $cargoExecutable -PathType Leaf)) { throw "Cargo missing: $cargoExecutable" }
    if (-not $env:LIBCLANG_PATH) {
        $configuredLibclang = [Environment]::GetEnvironmentVariable('LIBCLANG_PATH', 'User')
        if ($configuredLibclang) { $env:LIBCLANG_PATH = $configuredLibclang }
    }
    Push-Location $repository
    try {
        $cargoArgs = @('build', '-p', 'super-duper-worker', '--locked')
        if ($Configuration -eq 'Release') { $cargoArgs += '--release' }
        & $cargoExecutable @cargoArgs
        if ($LASTEXITCODE -ne 0) { throw 'Rust worker build failed.' }

        & dotnet build 'apps/windows/SuperDuper.Windows.sln' --configuration $Configuration -m:1
        if ($LASTEXITCODE -ne 0) { throw 'Windows solution build failed.' }
    }
    finally {
        Pop-Location
    }
}

if (-not (Test-Path -LiteralPath $app -PathType Leaf)) { throw "Windows app missing: $app" }
if (-not (Test-Path -LiteralPath $worker -PathType Leaf)) { throw "Rust worker missing: $worker" }

$stateBase = [IO.Path]::GetFullPath((Join-Path $repository 'artifacts/ui-dev-session'))
if ([string]::IsNullOrWhiteSpace($StateDirectory)) {
    $state = Join-Path $stateBase ([guid]::NewGuid().ToString('N'))
}
elseif ([IO.Path]::IsPathRooted($StateDirectory)) {
    $state = [IO.Path]::GetFullPath($StateDirectory)
}
else {
    $state = [IO.Path]::GetFullPath((Join-Path $repository $StateDirectory))
}
if (-not $state.StartsWith($stateBase.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "StateDirectory must be a child of $stateBase"
}
New-Item -ItemType Directory -Force -Path $state | Out-Null

if ($CreateFixture) {
    $fixture = Join-Path $state 'fixture'
    foreach ($root in @('Originals', 'Copies')) {
        $trip = Join-Path $fixture "$root/trip"
        New-Item -ItemType Directory -Force -Path $trip | Out-Null
        [IO.File]::WriteAllText((Join-Path $trip 'photo.txt'), 'same fictional photo content')
        [IO.File]::WriteAllText((Join-Path $trip 'notes.txt'), 'same fictional notes content')
    }
    [IO.File]::WriteAllText((Join-Path $fixture 'Copies/unique.txt'), 'fictional unique content')
}

$start = [Diagnostics.ProcessStartInfo]::new()
$start.FileName = $app
$start.WorkingDirectory = Split-Path -Parent $app
$start.UseShellExecute = $false
$start.Environment['SUPER_DUPER_WORKER_PATH'] = $worker
$start.Environment['SUPER_DUPER_DB_PATH'] = Join-Path $state 'super_duper.db'
$start.Environment['SUPER_DUPER_STATUS_DB_PATH'] = Join-Path $state 'scan_status.db'
$start.Environment['HASH_CACHE_PATH'] = Join-Path $state 'hash-cache'
$start.Environment['LOG_FILE_PATH'] = Join-Path $state 'app.log'
$process = [Diagnostics.Process]::Start($start)

Write-Output "APP_PID=$($process.Id)"
Write-Output "STATE_DIRECTORY=$state"
if ($CreateFixture) { Write-Output "FIXTURE_ROOT=$fixture" }
