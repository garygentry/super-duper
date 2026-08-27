[CmdletBinding()]
param(
    [ValidateSet(
        'tooling_fixture',
        'sop9b-representative-cancellation-v1',
        'sop9c-single-drive-reference-repeat-v1',
        'sop9d-multi-drive-reference-repeat-v1')]
    [string]$Campaign = 'tooling_fixture',
    [switch]$PreflightOnly,
    [switch]$SkipBuild,
    [switch]$InjectToolingFailureAfterStateReservation,
    [string]$ToolingRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$worker = Join-Path $repo 'target/release/super-duper-worker.exe'
$snapshotTool = Join-Path $repo 'target/release/examples/sop9_evidence_snapshot.exe'
$script:connection = $null
$script:journalPath = $null
$script:evidencePath = $null
$script:stateRoot = $null
$script:stateParent = $null
$script:ownedState = $false
$script:cleanup = [ordered]@{
    workerStopped = $false
    workerForced = $false
    stateRemovalAttempted = $false
    stateRemoved = $false
    statePreservedForDiagnostics = $false
    errors = [Collections.Generic.List[string]]::new()
}

function Get-CampaignDefinition([string]$Name, [string]$FixtureRoot) {
    switch ($Name) {
        'tooling_fixture' {
            return [pscustomobject]@{
                Id = 'tooling-fixture'
                Physical = $false
                RootIds = @('tooling-fixture')
                Roots = @($FixtureRoot)
                Policies = @('revalidate_content', 'reuse_verified')
                ExpectedTerminal = 'completed'
                CancelAfterFirstHashProgress = $false
                EvidenceRoot = Join-Path (Split-Path -Parent $FixtureRoot) 'evidence'
                StateRoot = Join-Path (Split-Path -Parent $FixtureRoot) 'state'
            }
        }
        'sop9b-representative-cancellation-v1' {
            return [pscustomobject]@{
                Id = $Name
                Physical = $true
                RootIds = @('D:')
                Roots = @('D:\')
                Policies = @('revalidate_content')
                ExpectedTerminal = 'cancelled'
                CancelAfterFirstHashProgress = $true
                EvidenceRoot = Join-Path $repo "artifacts/windows-sop9-large-drive/$Name"
                StateRoot = "H:\super-duper-sop9-state\$Name"
            }
        }
        'sop9c-single-drive-reference-repeat-v1' {
            return [pscustomobject]@{
                Id = $Name
                Physical = $true
                RootIds = @('E:')
                Roots = @('E:\')
                Policies = @('revalidate_content', 'reuse_verified')
                ExpectedTerminal = 'completed'
                CancelAfterFirstHashProgress = $false
                EvidenceRoot = Join-Path $repo "artifacts/windows-sop9-large-drive/$Name"
                StateRoot = "H:\super-duper-sop9-state\$Name"
            }
        }
        'sop9d-multi-drive-reference-repeat-v1' {
            return [pscustomobject]@{
                Id = $Name
                Physical = $true
                RootIds = @('D:', 'E:')
                Roots = @('D:\', 'E:\')
                Policies = @('revalidate_content', 'reuse_verified')
                ExpectedTerminal = 'completed'
                CancelAfterFirstHashProgress = $false
                EvidenceRoot = Join-Path $repo "artifacts/windows-sop9-large-drive/$Name"
                StateRoot = "H:\super-duper-sop9-state\$Name"
            }
        }
    }
}

function Get-Sha256([string]$Value) {
    $bytes = [Text.Encoding]::UTF8.GetBytes($Value)
    $hash = [Security.Cryptography.SHA256]::HashData($bytes)
    [Convert]::ToHexString($hash).ToLowerInvariant()
}

function Write-NewJson([string]$Path, $Value) {
    $json = $Value | ConvertTo-Json -Depth 100
    $bytes = [Text.Encoding]::UTF8.GetBytes($json + [Environment]::NewLine)
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try {
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }
}

function Add-Journal([string]$Event, $Data = $null) {
    if ($null -eq $script:journalPath) { return }
    $entry = [ordered]@{ utc = [DateTime]::UtcNow.ToString('o'); event = $Event; data = $Data }
    $line = ($entry | ConvertTo-Json -Compress -Depth 30) + [Environment]::NewLine
    $bytes = [Text.Encoding]::UTF8.GetBytes($line)
    $stream = [IO.File]::Open($script:journalPath, [IO.FileMode]::Append, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try {
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }
}

function New-ToolingFixture([string]$Root) {
    $scanRoot = Join-Path $Root 'scan-root'
    [IO.Directory]::CreateDirectory((Join-Path $scanRoot 'exact-a/nested')) | Out-Null
    [IO.Directory]::CreateDirectory((Join-Path $scanRoot 'exact-b/nested')) | Out-Null
    [IO.File]::WriteAllBytes((Join-Path $scanRoot 'pair-a.bin'), [byte[]](1..255) * 20)
    [IO.File]::WriteAllBytes((Join-Path $scanRoot 'pair-b.bin'), [byte[]](1..255) * 20)
    [IO.File]::WriteAllText((Join-Path $scanRoot 'singleton.txt'), 'singleton-size-contract')
    [IO.File]::WriteAllText((Join-Path $scanRoot 'exact-a/nested/data.txt'), 'exact-folder-contract')
    [IO.File]::WriteAllText((Join-Path $scanRoot 'exact-b/nested/data.txt'), 'exact-folder-contract')
    $hardLinkSource = Join-Path $scanRoot 'hard-link-source.bin'
    [IO.File]::WriteAllBytes($hardLinkSource, [byte[]](9..241))
    New-Item -ItemType HardLink -Path (Join-Path $scanRoot 'hard-link-alias.bin') -Target $hardLinkSource | Out-Null
    $scanRoot
}

function Assert-PhysicalPreflight($Definition) {
    $expected = @{ 'D:' = 0; 'E:' = 1 }
    foreach ($rootId in $Definition.RootIds) {
        $letter = $rootId.Substring(0, 1)
        $root = "$letter`:\"
        if (-not (Test-Path -LiteralPath $root -PathType Container)) {
            throw "Representative root is unavailable: $rootId"
        }
        $drive = [IO.DriveInfo]::new($root)
        if (-not $drive.IsReady -or $drive.DriveFormat -ne 'NTFS' -or $drive.TotalSize -lt 12TB) {
            throw "Representative volume no longer matches the predeclared NTFS/14-TB contract: $rootId"
        }
        $partition = Get-Partition -DriveLetter $letter
        if ($partition.DiskNumber -ne $expected[$rootId]) {
            throw "Representative volume $rootId moved from physical disk $($expected[$rootId]) to $($partition.DiskNumber)."
        }
        $disk = Get-PhysicalDisk -DeviceNumber $partition.DiskNumber
        if ($disk.MediaType -ne 'HDD' -or $disk.HealthStatus -ne 'Healthy') {
            throw "Representative device $rootId is not a healthy HDD."
        }
    }
    $stateDrive = [IO.DriveInfo]::new('H:\')
    if (-not $stateDrive.IsReady -or $stateDrive.AvailableFreeSpace -lt 50GB) {
        throw 'The isolated H: state volume requires at least 50 GiB free.'
    }
}

function Assert-SafeStatePath([string]$Path, [string]$Parent) {
    $resolvedPath = [IO.Path]::GetFullPath($Path).TrimEnd('\')
    $resolvedParent = [IO.Path]::GetFullPath($Parent).TrimEnd('\')
    if (-not $resolvedPath.StartsWith($resolvedParent + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unsafe campaign state path: $resolvedPath"
    }
    if ($resolvedPath -eq $resolvedParent -or [IO.Path]::GetPathRoot($resolvedPath).TrimEnd('\') -eq $resolvedPath) {
        throw "Refusing broad campaign state path: $resolvedPath"
    }
    if (Test-Path -LiteralPath $resolvedPath) {
        $item = Get-Item -LiteralPath $resolvedPath -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Refusing reparse-point campaign state: $resolvedPath"
        }
    }
    $resolvedPath
}

function Invoke-Checked([scriptblock]$Command, [string]$Description, [string]$LogPath) {
    & $Command *>&1 | Tee-Object -FilePath $LogPath
    if ($LASTEXITCODE -ne 0) { throw "$Description failed with exit code $LASTEXITCODE." }
}

function Start-CampaignWorker([string]$ProductDb, [string]$StatusDb, [string]$CachePath) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $worker
    $start.WorkingDirectory = Split-Path $worker
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.Environment['SUPER_DUPER_DB_PATH'] = $ProductDb
    $start.Environment['SUPER_DUPER_STATUS_DB_PATH'] = $StatusDb
    $start.Environment['HASH_CACHE_PATH'] = $CachePath
    $start.Environment['SUPER_DUPER_LOG'] = 'super_duper_core=info,super_duper_worker=info'
    $process = [Diagnostics.Process]::Start($start)
    if ($null -eq $process) { throw 'Windows did not start the SOP9 worker.' }
    [pscustomobject]@{
        Process = $process
        Stderr = $process.StandardError.ReadToEndAsync()
        NextId = 0L
        Stopped = $false
        Terminal = @{}
        LastProgress = @{}
        ProgressFrameCount = @{}
        ProgressBytes = @{}
        ProgressSecondCounts = @{}
        MaximumFramesPerSecond = @{}
        LastJournalPhase = @{}
    }
}

function Register-WorkerFrame($Connection, $Frame, [int]$SerializedBytes) {
    if ($Frame.type -ne 'event') { return }
    if ($Frame.event -eq 'run.progress') {
        $runId = [long]$Frame.data.runId
        $key = $runId.ToString([Globalization.CultureInfo]::InvariantCulture)
        $Connection.LastProgress[$key] = $Frame.data
        $Connection.ProgressFrameCount[$key] = 1L + [long]($Connection.ProgressFrameCount[$key] ?? 0L)
        $Connection.ProgressBytes[$key] = [long]$SerializedBytes + [long]($Connection.ProgressBytes[$key] ?? 0L)
        $second = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds().ToString([Globalization.CultureInfo]::InvariantCulture)
        $secondKey = "$key|$second"
        $Connection.ProgressSecondCounts[$secondKey] = 1 + [int]($Connection.ProgressSecondCounts[$secondKey] ?? 0)
        $Connection.MaximumFramesPerSecond[$key] = [Math]::Max(
            [int]($Connection.MaximumFramesPerSecond[$key] ?? 0),
            [int]$Connection.ProgressSecondCounts[$secondKey])
        $phase = [string]$Frame.data.progress.phase
        if ($Connection.LastJournalPhase[$key] -ne $phase -or $Connection.ProgressFrameCount[$key] % 600 -eq 0) {
            $Connection.LastJournalPhase[$key] = $phase
            Add-Journal 'run_progress' ([ordered]@{
                runId = $runId
                phase = $phase
                sequence = [long]$Frame.data.sequence
                progressFrameCount = [long]$Connection.ProgressFrameCount[$key]
                partialHashBytesRead = [string]$Frame.data.progress.counters.partialHashBytesRead
                fullHashBytesRead = [string]$Frame.data.progress.counters.fullHashBytesRead
                unavailableDeviceReason = $Frame.data.progress.activeDevices.reason
            })
        }
        return
    }
    if ($Frame.event -in @('run.completed', 'run.cancelled', 'run.failed')) {
        $runId = [long]$Frame.data.run.id
        $Connection.Terminal[$runId.ToString([Globalization.CultureInfo]::InvariantCulture)] = $Frame.data.run
        Add-Journal 'run_terminal' ([ordered]@{ runId = $runId; status = $Frame.data.run.status })
    }
}

function Read-WorkerFrame($Connection, [int]$TimeoutSeconds = 180) {
    $read = $Connection.Process.StandardOutput.ReadLineAsync()
    if (-not $read.Wait([TimeSpan]::FromSeconds($TimeoutSeconds))) {
        throw 'Timed out waiting for a worker protocol frame.'
    }
    $line = $read.Result
    if ($null -eq $line) { throw 'Worker stdout closed unexpectedly.' }
    try { $frame = $line | ConvertFrom-Json -Depth 100 }
    catch { throw 'Worker stdout contained malformed protocol JSON.' }
    Register-WorkerFrame $Connection $frame ([Text.Encoding]::UTF8.GetByteCount($line) + 1)
    $frame
}

function Send-WorkerRequest($Connection, [string]$Method, $Parameters) {
    $Connection.NextId++
    $id = $Connection.NextId.ToString([Globalization.CultureInfo]::InvariantCulture)
    $request = @{ type = 'request'; id = $id; method = $Method; params = $Parameters } |
        ConvertTo-Json -Compress -Depth 100
    $Connection.Process.StandardInput.WriteLine($request)
    $Connection.Process.StandardInput.Flush()
    while ($true) {
        $frame = Read-WorkerFrame $Connection
        if ($frame.type -eq 'response' -and $frame.id -eq $id) {
            if (-not $frame.ok) { throw "$Method failed: $($frame.error.code): $($frame.error.message)" }
            return $frame.result
        }
    }
}

function Wait-RunTerminal($Connection, [long]$RunId, [bool]$CancelAfterHashProgress) {
    $key = $RunId.ToString([Globalization.CultureInfo]::InvariantCulture)
    $cancelRequested = $false
    $cancelStarted = $null
    while (-not $Connection.Terminal.ContainsKey($key)) {
        $null = Read-WorkerFrame $Connection
        if ($CancelAfterHashProgress -and -not $cancelRequested -and $Connection.LastProgress.ContainsKey($key)) {
            $lastProgress = $Connection.LastProgress[$key]
            $progress = $lastProgress.progress
            if ($lastProgress.phase -eq 'hashing' -and
                ([UInt64][string]$progress.counters.partialHashBytesRead -gt 0 -or
                 [UInt64][string]$progress.counters.fullHashBytesRead -gt 0)) {
                $cancelStarted = [Diagnostics.Stopwatch]::StartNew()
                Add-Journal 'cancellation_requested' ([ordered]@{ runId = $RunId; trigger = 'first_hash_read_progress' })
                $response = Send-WorkerRequest $Connection 'run.cancel' @{ runId = $RunId }
                if ($response.run.status -notin @('cancelling', 'cancelled')) {
                    throw 'Representative cancellation did not enter a cancelling state.'
                }
                $cancelRequested = $true
            }
        }
    }
    if ($null -ne $cancelStarted) { $cancelStarted.Stop() }
    [pscustomobject]@{
        Run = $Connection.Terminal[$key]
        CancellationRequested = $cancelRequested
        CancellationLatencyMilliseconds = if ($null -eq $cancelStarted) { $null } else { $cancelStarted.Elapsed.TotalMilliseconds }
    }
}

function Stop-CampaignWorker($Connection, [string]$StderrPath) {
    if ($null -eq $Connection -or $Connection.Stopped) { return }
    try {
        $Connection.Process.StandardInput.Close()
        if (-not $Connection.Process.WaitForExit(30000)) {
            $Connection.Process.Kill($true)
            $Connection.Process.WaitForExit(10000) | Out-Null
            $script:cleanup.workerForced = $true
        }
        [IO.File]::WriteAllText($StderrPath, $Connection.Stderr.Result)
        if ($Connection.Process.ExitCode -ne 0) {
            throw "Worker exited with code $($Connection.Process.ExitCode)."
        }
        $script:cleanup.workerStopped = $true
    }
    finally {
        $Connection.Process.Dispose()
        $Connection.Stopped = $true
    }
}

function Get-Snapshot([string]$ProductDb, [string]$StatusDb, [long]$RunId) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $snapshotTool
    $start.ArgumentList.Add($ProductDb)
    $start.ArgumentList.Add($StatusDb)
    $start.ArgumentList.Add($RunId.ToString([Globalization.CultureInfo]::InvariantCulture))
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [Diagnostics.Process]::Start($start)
    try {
        $stdout = $process.StandardOutput.ReadToEnd()
        $stderr = $process.StandardError.ReadToEnd()
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) { throw "SOP9 snapshot helper failed: $stderr" }
        $stdout | ConvertFrom-Json -Depth 100
    }
    finally { $process.Dispose() }
}

$toolingBase = if ([string]::IsNullOrWhiteSpace($ToolingRoot)) {
    Join-Path ([IO.Path]::GetTempPath()) ('super-duper-sop9-tooling-' + [guid]::NewGuid().ToString('N'))
} else {
    [IO.Path]::GetFullPath($ToolingRoot)
}
$fixtureRoot = if ($Campaign -eq 'tooling_fixture') { New-ToolingFixture $toolingBase } else { $null }
$definition = Get-CampaignDefinition $Campaign $fixtureRoot

if ($definition.Physical -and $SkipBuild) { throw 'Physical SOP9 campaigns may not skip the pinned Release build.' }
if ($definition.Physical -and $InjectToolingFailureAfterStateReservation) {
    throw 'Failure injection is restricted to the non-representative tooling fixture.'
}
if (Test-Path -LiteralPath $definition.EvidenceRoot) {
    throw "SOP9 evidence is write-once and already exists: $($definition.EvidenceRoot)"
}
if (Test-Path -LiteralPath $definition.StateRoot) {
    throw "SOP9 campaign state already exists: $($definition.StateRoot)"
}

if ($PreflightOnly) {
    if ($definition.Physical) { Assert-PhysicalPreflight $definition }
    [pscustomobject]@{
        campaignId = $definition.Id
        evidencePathAbsent = $true
        statePathAbsent = $true
        rootIds = $definition.RootIds
        policies = $definition.Policies
        expectedTerminal = $definition.ExpectedTerminal
    } | ConvertTo-Json -Depth 10
    return
}

[IO.Directory]::CreateDirectory($definition.EvidenceRoot) | Out-Null
$script:journalPath = Join-Path $definition.EvidenceRoot 'attempt.jsonl'
$script:evidencePath = Join-Path $definition.EvidenceRoot 'acceptance-evidence.json'
[IO.File]::WriteAllText($script:journalPath, '')
$manifest = [ordered]@{
    schemaVersion = 1
    campaignId = $definition.Id
    createdAtUtc = [DateTime]::UtcNow.ToString('o')
    physical = $definition.Physical
    rootIds = $definition.RootIds
    rootIdentitySha256 = @($definition.Roots | ForEach-Object { Get-Sha256 ([IO.Path]::GetFullPath($_).ToUpperInvariant()) })
    policies = $definition.Policies
    expectedTerminal = $definition.ExpectedTerminal
    cancelAfterFirstHashProgress = $definition.CancelAfterFirstHashProgress
    noFavorableRetry = $true
    sop2ObserverRisk = [ordered]@{
        strictGateEvaluated = $false
        strictGatePassed = $false
        causalAttributionAvailable = $false
        residualRisk = 'Progress-frame/status-write/resource proxies are measured at SOP9 scale; no observer-off counterfactual exists.'
    }
}
Write-NewJson (Join-Path $definition.EvidenceRoot 'manifest.json') $manifest
Add-Journal 'attempt_reserved' ([ordered]@{ campaignId = $definition.Id; physical = $definition.Physical })

$script:stateRoot = $definition.StateRoot
$script:stateParent = Split-Path -Parent $script:stateRoot
$null = Assert-SafeStatePath $script:stateRoot $script:stateParent
$arms = [Collections.Generic.List[object]]::new()
$previousProcess = [ordered]@{
    cpuNanos = 0L; readOperations = 0L; readBytes = 0L; writeOperations = 0L; writeBytes = 0L
}
$failure = $null
$valid = $false
$productDb = Join-Path $script:stateRoot 'product.db'
$statusDb = Join-Path $script:stateRoot 'status.db'
$cachePath = Join-Path $script:stateRoot 'hash-cache'
$stderrPath = Join-Path $definition.EvidenceRoot 'worker-stderr.log'

try {
    if ($definition.Physical) {
        Assert-PhysicalPreflight $definition
        Add-Journal 'physical_preflight_passed' ([ordered]@{ rootIds = $definition.RootIds })
    }
    [IO.Directory]::CreateDirectory($script:stateRoot) | Out-Null
    $script:ownedState = $true
    Add-Journal 'state_created' ([ordered]@{ stateIdentitySha256 = Get-Sha256 $script:stateRoot.ToUpperInvariant() })
    if ($InjectToolingFailureAfterStateReservation) {
        throw 'Injected SOP9 tooling failure after state reservation.'
    }
    if (-not $SkipBuild) {
        Push-Location $repo
        try {
            Invoke-Checked { cargo build --release -p super-duper-worker } 'Release worker build' (Join-Path $definition.EvidenceRoot 'worker-build.log')
            Invoke-Checked { cargo build --release -p super-duper-core --example sop9_evidence_snapshot } 'Release snapshot-helper build' (Join-Path $definition.EvidenceRoot 'snapshot-build.log')
        }
        finally { Pop-Location }
    }
    if (-not (Test-Path -LiteralPath $worker -PathType Leaf) -or
        -not (Test-Path -LiteralPath $snapshotTool -PathType Leaf)) {
        throw 'Pinned Release worker or snapshot helper is missing.'
    }
    Add-Journal 'build_ready' ([ordered]@{
        workerSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $worker).Hash.ToLowerInvariant()
        snapshotToolSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $snapshotTool).Hash.ToLowerInvariant()
    })

    $script:connection = Start-CampaignWorker $productDb $statusDb $cachePath
    Add-Journal 'worker_started' ([ordered]@{ processId = $script:connection.Process.Id })
    $hello = Send-WorkerRequest $script:connection 'hello' @{
        protocolVersions = @(1)
        client = @{ name = 'sop9-large-drive-acceptance'; version = '1.0.0' }
    }
    if ($hello.protocolVersion -ne 1) { throw 'Worker did not negotiate protocol V1.' }
    $session = (Send-WorkerRequest $script:connection 'session.create' @{
        name = "SOP9 $($definition.Id)"
        roots = $definition.Roots
        ignorePatterns = @()
        cloudPolicy = 'exclude_registered_roots'
        manualLocationExclusions = @()
        registeredCloudLocations = @()
        cloudDetectionStatus = 'complete'
    }).session

    foreach ($policy in $definition.Policies) {
        $startedAt = [DateTime]::UtcNow
        Add-Journal 'arm_started' ([ordered]@{ policy = $policy; ordinal = $arms.Count })
        $run = (Send-WorkerRequest $script:connection 'run.start' @{
            sessionId = $session.id
            repeatCachePolicy = $policy
        }).run
        $terminal = Wait-RunTerminal $script:connection $run.id $definition.CancelAfterFirstHashProgress
        $snapshot = Get-Snapshot $productDb $statusDb $run.id
        $processCumulative = [ordered]@{
            cpuNanos = [long]($snapshot.status.processCpuNanos.maximum ?? 0L)
            readOperations = [long]($snapshot.status.processReadOperations.maximum ?? 0L)
            readBytes = [long]($snapshot.status.processReadBytes.maximum ?? 0L)
            writeOperations = [long]($snapshot.status.processWriteOperations.maximum ?? 0L)
            writeBytes = [long]($snapshot.status.processWriteBytes.maximum ?? 0L)
        }
        $processDelta = [ordered]@{}
        foreach ($metric in @('cpuNanos', 'readOperations', 'readBytes', 'writeOperations', 'writeBytes')) {
            $processDelta[$metric] = [Math]::Max(0L, [long]$processCumulative[$metric] - [long]$previousProcess[$metric])
            $previousProcess[$metric] = $processCumulative[$metric]
        }
        $key = $run.id.ToString([Globalization.CultureInfo]::InvariantCulture)
        $arms.Add([ordered]@{
            ordinal = $arms.Count
            policy = $policy
            runId = $run.id
            terminalStatus = $terminal.Run.status
            wallMilliseconds = ([DateTime]::UtcNow - $startedAt).TotalMilliseconds
            cancellationRequested = $terminal.CancellationRequested
            cancellationLatencyMilliseconds = $terminal.CancellationLatencyMilliseconds
            progressFrameCount = [long]($script:connection.ProgressFrameCount[$key] ?? 0L)
            progressSerializedBytes = [long]($script:connection.ProgressBytes[$key] ?? 0L)
            maximumFramesPerObservedSecond = [int]($script:connection.MaximumFramesPerSecond[$key] ?? 0)
            processCumulative = $processCumulative
            processDelta = $processDelta
            statusDatabaseBytes = (Get-Item -LiteralPath $statusDb).Length
            statusWalBytes = if (Test-Path -LiteralPath "$statusDb-wal") { (Get-Item -LiteralPath "$statusDb-wal").Length } else { 0L }
            productDatabaseBytes = (Get-Item -LiteralPath $productDb).Length
            productWalBytes = if (Test-Path -LiteralPath "$productDb-wal") { (Get-Item -LiteralPath "$productDb-wal").Length } else { 0L }
            snapshot = $snapshot
        })
        if ($terminal.Run.status -ne $definition.ExpectedTerminal) {
            throw "Run $($run.id) ended $($terminal.Run.status), expected $($definition.ExpectedTerminal)."
        }
    }
    if ($definition.ExpectedTerminal -eq 'completed' -and $arms.Count -eq 2) {
        if ($arms[0].snapshot.product.fileResultSha256 -ne $arms[1].snapshot.product.fileResultSha256 -or
            $arms[0].snapshot.product.folderResultSha256 -ne $arms[1].snapshot.product.folderResultSha256) {
            throw 'Forced and verified-reuse result digests differ.'
        }
    }
    $valid = $true
}
catch {
    $failure = $_.Exception.ToString()
    Add-Journal 'attempt_failed' ([ordered]@{ error = $failure })
}
finally {
    try { Stop-CampaignWorker $script:connection $stderrPath }
    catch {
        $script:cleanup.errors.Add($_.Exception.Message)
        Add-Journal 'worker_cleanup_failed' ([ordered]@{ error = $_.Exception.Message })
    }
    if ($script:ownedState) {
        if ($null -eq $failure -or $arms.Count -gt 0) {
            $script:cleanup.stateRemovalAttempted = $true
            try {
                $validatedState = Assert-SafeStatePath $script:stateRoot $script:stateParent
                Remove-Item -LiteralPath $validatedState -Recurse -Force
                $script:cleanup.stateRemoved = -not (Test-Path -LiteralPath $validatedState)
                if (-not $script:cleanup.stateRemoved) { throw 'Campaign state remained after cleanup.' }
            }
            catch {
                $script:cleanup.errors.Add($_.Exception.Message)
                Add-Journal 'state_cleanup_failed' ([ordered]@{ error = $_.Exception.Message })
            }
        }
        else {
            $script:cleanup.statePreservedForDiagnostics = $true
        }
    }
}

$evidence = [ordered]@{
    schemaVersion = 1
    campaignId = $definition.Id
    status = if ($valid -and $script:cleanup.workerStopped -and $script:cleanup.stateRemoved -and $script:cleanup.errors.Count -eq 0) { 'valid' } else { 'invalid' }
    physical = $definition.Physical
    completedAtUtc = [DateTime]::UtcNow.ToString('o')
    noFavorableRetry = $true
    favorableRetryCount = 0
    expectedTerminal = $definition.ExpectedTerminal
    arms = $arms
    comparisons = [ordered]@{
        fileResultsEqual = $arms.Count -eq 2 -and $arms[0].snapshot.product.fileResultSha256 -eq $arms[1].snapshot.product.fileResultSha256
        folderResultsEqual = $arms.Count -eq 2 -and $arms[0].snapshot.product.folderResultSha256 -eq $arms[1].snapshot.product.folderResultSha256
    }
    sop2ObserverRisk = $manifest.sop2ObserverRisk
    failure = $failure
    cleanup = $script:cleanup
}
try {
    Write-NewJson $script:evidencePath $evidence
    Add-Journal 'evidence_finalized' ([ordered]@{ status = $evidence.status })
}
catch {
    Add-Journal 'evidence_finalization_failed' ([ordered]@{ error = $_.Exception.ToString() })
    throw
}

if ($evidence.status -ne 'valid') {
    throw "SOP9 campaign retained an invalid outcome at $script:evidencePath"
}
Write-Output "SOP9_CAMPAIGN_EVIDENCE=$script:evidencePath"
