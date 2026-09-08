[CmdletBinding()]
param([switch]$Show)

$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$project = Join-Path $repository 'apps/windows/tools/SuperDuper.Windows.RedesignFixture/SuperDuper.Windows.RedesignFixture.csproj'
$output = Join-Path $repository 'artifacts/uir03-desktop-fixture'

# This project links in-memory test services. It never calls shipping App.OnStartup,
# creates a WorkerClient, reads a production database or invokes Explorer/deletion.
& dotnet build $project --artifacts-path $output
if ($LASTEXITCODE -ne 0) { throw 'The fictional fixture build failed.' }
$executable = Join-Path $output 'bin/SuperDuper.Windows.RedesignFixture/debug_win-x64/SuperDuper.Windows.RedesignFixture.exe'
Write-Output "Fictional desktop fixture: $executable"
if ($Show) {
    # Explicit -Show is the reviewer's request for this visible interactive window.
    & $executable
}
