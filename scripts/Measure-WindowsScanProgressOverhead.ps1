[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$controlRevision = '0a3c1c1'
$treatmentRevision = 'f803cbd'
$evidencePath = Join-Path $repo 'docs/evidence/scan-progress-overhead-20260825.json'
$tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$profileRoot = Join-Path $tempParent ('super-duper-sop2-profile-' + [guid]::NewGuid().ToString('N'))

function Invoke-Checked([scriptblock]$Command, [string]$Failure) {
    & $Command | Out-Host
    if ($LASTEXITCODE -ne 0) { throw $Failure }
}

function Expand-Revision([string]$Revision, [string]$Destination) {
    $archive = "$Destination.tar"
    [IO.Directory]::CreateDirectory($Destination) | Out-Null
    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper archive --format=tar --output=$archive $Revision } `
        "Could not archive revision $Revision."
    Invoke-Checked { tar -xf $archive -C $Destination } "Could not expand revision $Revision."
    [IO.File]::Delete($archive)
}

function Build-Worker([string]$Source, [string]$Target) {
    $previousTarget = $env:CARGO_TARGET_DIR
    try {
        $env:CARGO_TARGET_DIR = $Target
        Push-Location $Source
        try {
            Invoke-Checked { cargo build --release -p super-duper-worker } `
                "Release worker build failed for $Source."
        }
        finally { Pop-Location }
    }
    finally {
        if ($null -eq $previousTarget) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
        else { $env:CARGO_TARGET_DIR = $previousTarget }
    }
    $worker = Join-Path $Target 'release/super-duper-worker.exe'
    if (-not (Test-Path -LiteralPath $worker -PathType Leaf)) {
        throw "Release worker not found: $worker"
    }
    $worker
}

function New-ProfileFixture([string]$Root) {
    [IO.Directory]::CreateDirectory($Root) | Out-Null
    for ($size = 1; $size -le 12000; $size++) {
        $bytes = [byte[]]::new($size)
        [Array]::Fill($bytes, [byte](($size % 251) + 1))
        [IO.File]::WriteAllBytes((Join-Path $Root ('file-{0:D5}.bin' -f $size)), $bytes)
    }
}

function Read-Frame($Connection, [int]$TimeoutSeconds = 120) {
    $read = $Connection.Process.StandardOutput.ReadLineAsync()
    if (-not $read.Wait([TimeSpan]::FromSeconds($TimeoutSeconds))) {
        throw 'Timed out waiting for a worker protocol frame.'
    }
    $line = $read.Result
    if ($null -eq $line) { throw 'Worker stdout closed unexpectedly.' }
    try { $frame = $line | ConvertFrom-Json -Depth 40 }
    catch { throw "Worker stdout was not protocol JSON: $line" }
    if ($frame.type -eq 'event' -and $frame.event -eq 'run.progress') {
        if ($Connection.TerminalSeen) { throw 'Progress arrived after the terminal event.' }
        $Connection.ProgressFrames++
        $Connection.ProgressBytes += [Text.Encoding]::UTF8.GetByteCount($line) + 1
    }
    elseif ($frame.type -eq 'event' -and $frame.event -in @('run.completed', 'run.cancelled', 'run.failed')) {
        $Connection.TerminalSeen = $true
    }
    $frame
}

function Send-Request($Connection, [string]$Method, $Parameters) {
    $Connection.NextId++
    $id = $Connection.NextId.ToString([Globalization.CultureInfo]::InvariantCulture)
    $request = @{ type = 'request'; id = $id; method = $Method; params = $Parameters } |
        ConvertTo-Json -Compress -Depth 40
    $Connection.Process.StandardInput.WriteLine($request)
    $Connection.Process.StandardInput.Flush()
    while ($true) {
        $frame = Read-Frame $Connection
        if ($frame.type -eq 'response' -and $frame.id -eq $id) {
            if (-not $frame.ok) {
                throw "$Method failed: $($frame.error.code): $($frame.error.message)"
            }
            return $frame.result
        }
    }
}

function Wait-Terminal($Connection, [long]$RunId) {
    while ($true) {
        $frame = Read-Frame $Connection
        if ($frame.type -eq 'event' -and
            $frame.event -in @('run.completed', 'run.cancelled', 'run.failed') -and
            [long]$frame.data.run.id -eq $RunId) {
            if ($frame.data.run.status -ne 'completed') {
                throw "Profile run $RunId ended as $($frame.data.run.status)."
            }
            return
        }
    }
}

function Measure-Run(
    [string]$Mode,
    [string]$Worker,
    [string]$Fixture,
    [string]$StateRoot,
    [int]$Ordinal,
    [bool]$Warmup
) {
    $runRoot = Join-Path $StateRoot ("$Mode-$Ordinal")
    [IO.Directory]::CreateDirectory($runRoot) | Out-Null
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Worker
    $start.WorkingDirectory = Split-Path $Worker
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.Environment['SUPER_DUPER_DB_PATH'] = Join-Path $runRoot 'product.db'
    $start.Environment['SUPER_DUPER_STATUS_DB_PATH'] = Join-Path $runRoot 'status.db'
    $start.Environment['HASH_CACHE_PATH'] = Join-Path $runRoot 'hash-cache'
    $start.Environment['SUPER_DUPER_LOG'] = 'off'
    $process = [Diagnostics.Process]::Start($start)
    if ($null -eq $process) { throw "Windows did not start the $Mode worker." }
    $connection = [pscustomobject]@{
        Process = $process
        Stderr = $process.StandardError.ReadToEndAsync()
        NextId = 0
        ProgressFrames = 0L
        ProgressBytes = 0L
        TerminalSeen = $false
    }
    try {
        $hello = Send-Request $connection 'hello' @{
            protocolVersions = @(1)
            client = @{ name = 'sop2-overhead-profile'; version = '1.0.0' }
        }
        if ($hello.protocolVersion -ne 1) { throw 'Protocol V1 negotiation failed.' }
        $session = (Send-Request $connection 'session.create' @{
            name = "SOP2 $Mode profile"
            roots = @($Fixture)
            ignorePatterns = @()
            cloudPolicy = 'exclude_registered_roots'
            manualLocationExclusions = @()
            registeredCloudLocations = @()
            cloudDetectionStatus = 'complete'
        }).session
        $connection.ProgressFrames = 0
        $connection.ProgressBytes = 0
        $connection.TerminalSeen = $false
        $process.Refresh()
        $cpuBefore = $process.TotalProcessorTime.Ticks
        $wallBefore = [Diagnostics.Stopwatch]::GetTimestamp()
        $run = (Send-Request $connection 'run.start' @{ sessionId = $session.id }).run
        Wait-Terminal $connection $run.id
        $wallAfter = [Diagnostics.Stopwatch]::GetTimestamp()
        $process.Refresh()
        $cpuAfter = $process.TotalProcessorTime.Ticks
        $process.StandardInput.Close()
        $remaining = $process.StandardOutput.ReadToEnd()
        if (-not $process.WaitForExit(10000)) { throw "$Mode worker did not stop after EOF." }
        if ($remaining.Length -ne 0) { throw "$Mode worker emitted unconsumed JSONL after terminal." }
        if ($process.ExitCode -ne 0) {
            throw "$Mode worker exited with code $($process.ExitCode): $($connection.Stderr.Result)"
        }
        [pscustomobject]@{
            mode = $Mode
            warmup = $Warmup
            ordinal = $Ordinal
            wallNanos = [Diagnostics.Stopwatch]::GetElapsedTime($wallBefore, $wallAfter).Ticks * 100
            cpuNanos = ($cpuAfter - $cpuBefore) * 100
            progressFrames = $connection.ProgressFrames
            progressBytes = $connection.ProgressBytes
        }
    }
    finally {
        if (-not $process.HasExited) {
            try { $process.Kill($true) } catch { }
            $process.WaitForExit(5000) | Out-Null
        }
        $process.Dispose()
    }
}

function Get-Median([object[]]$Values) {
    $ordered = @($Values | Sort-Object)
    $ordered[[int][Math]::Floor($ordered.Count / 2)]
}

function Get-BasisPoints([long]$Control, [long]$Treatment) {
    [long][Math]::Round((($Treatment - $Control) * 10000.0) / $Control, 0, [MidpointRounding]::AwayFromZero)
}

if (Test-Path -LiteralPath $evidencePath) {
    throw "Retained SOP2 profile already exists; refusing a rerun: $evidencePath"
}

[IO.Directory]::CreateDirectory($profileRoot) | Out-Null
try {
    $controlSource = Join-Path $profileRoot 'control-source'
    $treatmentSource = Join-Path $profileRoot 'treatment-source'
    Expand-Revision $controlRevision $controlSource
    Expand-Revision $treatmentRevision $treatmentSource
    $controlWorker = Build-Worker $controlSource (Join-Path $profileRoot 'control-target')
    $treatmentWorker = Build-Worker $treatmentSource (Join-Path $profileRoot 'treatment-target')
    $fixture = Join-Path $profileRoot 'fixture'
    New-ProfileFixture $fixture
    $stateRoot = Join-Path $profileRoot 'state'
    $allRuns = [Collections.Generic.List[object]]::new()
    $allRuns.Add((Measure-Run 'control' $controlWorker $fixture $stateRoot 0 $true))
    $allRuns.Add((Measure-Run 'treatment' $treatmentWorker $fixture $stateRoot 0 $true))
    for ($pair = 1; $pair -le 3; $pair++) {
        $allRuns.Add((Measure-Run 'control' $controlWorker $fixture $stateRoot $pair $false))
        $allRuns.Add((Measure-Run 'treatment' $treatmentWorker $fixture $stateRoot $pair $false))
    }
    $controlRuns = @($allRuns | Where-Object { -not $_.warmup -and $_.mode -eq 'control' })
    $treatmentRuns = @($allRuns | Where-Object { -not $_.warmup -and $_.mode -eq 'treatment' })
    [long]$controlWall = Get-Median @($controlRuns.wallNanos)
    [long]$treatmentWall = Get-Median @($treatmentRuns.wallNanos)
    [long]$controlCpu = Get-Median @($controlRuns.cpuNanos)
    [long]$treatmentCpu = Get-Median @($treatmentRuns.cpuNanos)
    $result = [ordered]@{
        gate = 'SOP2f-progress-acceptance'
        date = '2026-08-25'
        machine = 'designated Windows 11 x64 development machine'
        configuration = 'Release'
        controlRevision = $controlRevision
        treatmentRevision = $treatmentRevision
        fixture = [ordered]@{
            files = 12000
            shape = 'one directory; unique sizes 1 through 12000 bytes; current singleton partial-read baseline'
            warmupRunsPerMode = 1
            measuredRunsPerMode = 3
            order = 'control warmup, treatment warmup, control/treatment repeated three times'
        }
        median = [ordered]@{
            controlWallNanos = $controlWall
            treatmentWallNanos = $treatmentWall
            wallOverheadBasisPoints = Get-BasisPoints $controlWall $treatmentWall
            controlCpuNanos = $controlCpu
            treatmentCpuNanos = $treatmentCpu
            cpuOverheadBasisPoints = Get-BasisPoints $controlCpu $treatmentCpu
        }
        absoluteDifference = [ordered]@{
            wallNanos = $treatmentWall - $controlWall
            cpuNanos = $treatmentCpu - $controlCpu
        }
        publishedThresholdBasisPoints = 100
        runs = $allRuns
    }
    Write-Output 'SOP2_PROFILE_JSON_BEGIN'
    $result | ConvertTo-Json -Depth 20
    Write-Output 'SOP2_PROFILE_JSON_END'
}
finally {
    $resolvedProfileRoot = [IO.Path]::GetFullPath($profileRoot).TrimEnd('\')
    $resolvedParent = [IO.Path]::GetDirectoryName($resolvedProfileRoot).TrimEnd('\')
    if (-not $resolvedParent.Equals($tempParent, [StringComparison]::OrdinalIgnoreCase) -or
        -not ([IO.Path]::GetFileName($resolvedProfileRoot)).StartsWith('super-duper-sop2-profile-', [StringComparison]::Ordinal)) {
        throw "Unsafe profile cleanup path: $resolvedProfileRoot"
    }
    if (Test-Path -LiteralPath $resolvedProfileRoot) {
        Remove-Item -LiteralPath $resolvedProfileRoot -Recurse -Force
    }
}
