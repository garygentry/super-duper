[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Release',
    [switch]$SkipBuild,
    [switch]$ConfirmRecycleBinMutation,
    [switch]$RunProviderNoHydration,
    [string]$CloudRoot,
    [string]$LocallyAvailableFile,
    [string]$OfflinePlaceholder,
    [string[]]$ProviderProcessName,
    [switch]$RunWarmQueryProfile,
    [string]$EvidenceDirectory
)

$ErrorActionPreference = 'Stop'

function Add-MatrixEntry(
    [string]$Gate,
    [string]$Status,
    [string]$Evidence,
    [string]$Boundary
) {
    $script:matrix.Add([ordered]@{
        gate = $Gate
        status = $Status
        evidence = $Evidence
        boundary = $Boundary
    })
}

function Invoke-LoggedNative(
    [string]$Name,
    [string]$FilePath,
    [string[]]$Arguments
) {
    $logPath = Join-Path $script:evidenceRoot "$Name.log"
    $started = Get-Date
    & $FilePath @Arguments 2>&1 | Tee-Object -LiteralPath $logPath | Out-Host
    $exitCode = $LASTEXITCODE
    $elapsed = (Get-Date) - $started
    $result = [ordered]@{
        name = $Name
        command = "$FilePath $($Arguments -join ' ')"
        exitCode = $exitCode
        elapsedMilliseconds = [math]::Round($elapsed.TotalMilliseconds, 2)
        log = $logPath
    }
    $script:commands.Add($result)
    return $result
}

function Assert-Executed([object]$Result) {
    if ($Result.exitCode -ne 0) {
        throw "$($Result.name) failed with exit code $($Result.exitCode). See $($Result.log)."
    }
}

function Start-HostDiagnosticsSampler([string]$OutputPath) {
    try {
        $null = New-Item -ItemType File -Path $OutputPath -ErrorAction Stop
        return Start-Job -ArgumentList $OutputPath -ScriptBlock {
            param([string]$Destination)
            $ErrorActionPreference = 'Stop'
            try {
                $nativeSource = @(
                    'using System;',
                    'using System.Runtime.InteropServices;',
                    'public static class SuperDuperAcceptanceProcessIo {',
                    '  [StructLayout(LayoutKind.Sequential)]',
                    '  private struct IoCounters {',
                    '    public ulong ReadOperationCount, WriteOperationCount, OtherOperationCount;',
                    '    public ulong ReadTransferCount, WriteTransferCount, OtherTransferCount;',
                    '  }',
                    '  [DllImport("kernel32.dll", SetLastError = true)]',
                    '  private static extern bool GetProcessIoCounters(IntPtr handle, out IoCounters counters);',
                    '  public static bool TryGetTotalTransferBytes(IntPtr handle, out ulong total) {',
                    '    IoCounters counters;',
                    '    bool success = GetProcessIoCounters(handle, out counters);',
                    '    total = success ? counters.ReadTransferCount + counters.WriteTransferCount + counters.OtherTransferCount : 0;',
                    '    return success;',
                    '  }',
                    '}'
                ) -join [Environment]::NewLine
                Add-Type -TypeDefinition $nativeSource -ErrorAction Stop
                function Get-DiagnosticProcessSnapshot {
                    $snapshot = @{}
                    foreach ($item in @(Get-Process -ErrorAction SilentlyContinue)) {
                        try {
                            [uint64]$ioTransferBytes = 0
                            $ioAvailable = [SuperDuperAcceptanceProcessIo]::TryGetTotalTransferBytes(
                                $item.Handle,
                                [ref]$ioTransferBytes)
                            $snapshot[[int]$item.Id] = [pscustomobject][ordered]@{
                                name = $item.ProcessName
                                processId = [int]$item.Id
                                cpuMilliseconds = [double]$item.TotalProcessorTime.TotalMilliseconds
                                workingSetBytes = [long]$item.WorkingSet64
                                ioTransferBytes = $ioTransferBytes
                                ioAvailable = $ioAvailable
                            }
                        }
                        catch {
                            # Protected or exiting processes are omitted from competing-process detail.
                        }
                        finally {
                            $item.Dispose()
                        }
                    }
                    return $snapshot
                }
                $counterDefinitions = @(
                    @{ key = 'processorTotal'; category = 'Processor'; counter = '% Processor Time'; instance = '_Total' },
                    @{ key = 'processorPrivileged'; category = 'Processor'; counter = '% Privileged Time'; instance = '_Total' },
                    @{ key = 'memoryAvailable'; category = 'Memory'; counter = 'Available MBytes'; instance = '' },
                    @{ key = 'memoryCommitted'; category = 'Memory'; counter = '% Committed Bytes In Use'; instance = '' },
                    @{ key = 'memoryPagesInput'; category = 'Memory'; counter = 'Pages Input/sec'; instance = '' },
                    @{ key = 'diskBytes'; category = 'PhysicalDisk'; counter = 'Disk Bytes/sec'; instance = '_Total' },
                    @{ key = 'diskTime'; category = 'PhysicalDisk'; counter = '% Disk Time'; instance = '_Total' },
                    @{ key = 'diskQueue'; category = 'PhysicalDisk'; counter = 'Current Disk Queue Length'; instance = '_Total' },
                    @{ key = 'diskSplit'; category = 'PhysicalDisk'; counter = 'Split IO/Sec'; instance = '_Total' },
                    @{ key = 'processorQueue'; category = 'System'; counter = 'Processor Queue Length'; instance = '' },
                    @{ key = 'contextSwitches'; category = 'System'; counter = 'Context Switches/sec'; instance = '' },
                    @{ key = 'processes'; category = 'System'; counter = 'Processes'; instance = '' },
                    @{ key = 'threads'; category = 'System'; counter = 'Threads'; instance = '' }
                )
                $counters = [ordered]@{}
                foreach ($definition in $counterDefinitions) {
                    $counters[$definition.key] = [Diagnostics.PerformanceCounter]::new(
                        $definition.category,
                        $definition.counter,
                        $definition.instance,
                        $true)
                    $null = $counters[$definition.key].NextValue()
                }
                $previousProcesses = Get-DiagnosticProcessSnapshot
                $previousCapturedAt = (Get-Date).ToUniversalTime()

                while ($true) {
                    Start-Sleep -Seconds 2
                    $values = [ordered]@{}
                    foreach ($definition in $counterDefinitions) {
                        $values[$definition.key] = [double]$counters[$definition.key].NextValue()
                    }
                    $currentProcesses = Get-DiagnosticProcessSnapshot
                    $processDeltas = [Collections.Generic.List[object]]::new()
                    $capturedAt = (Get-Date).ToUniversalTime()
                    $elapsedSeconds = [math]::Max(($capturedAt - $previousCapturedAt).TotalSeconds, 0.001)
                    foreach ($processId in @($currentProcesses.Keys)) {
                        $process = $currentProcesses[$processId]
                        if ($previousProcesses.ContainsKey($processId) -and
                            $process.name -eq $previousProcesses[$processId].name) {
                            $previous = $previousProcesses[$processId]
                            $currentCpu = $process.cpuMilliseconds
                            $previousCpu = $previous.cpuMilliseconds
                            $currentIo = $process.ioTransferBytes
                            $previousIo = $previous.ioTransferBytes
                            if ($currentCpu -ge $previousCpu) {
                                $ioBytesPerSecond = if ($process.ioAvailable -and $previous.ioAvailable -and $currentIo -ge $previousIo) {
                                    [math]::Round(($currentIo - $previousIo) / $elapsedSeconds, 2)
                                }
                                else {
                                    $null
                                }
                                $processDeltas.Add([pscustomobject][ordered]@{
                                    name = $process.Name
                                    processId = $processId
                                    percentProcessorTime = [math]::Round((($currentCpu - $previousCpu) / 1000 / $elapsedSeconds) * 100, 2)
                                    workingSetBytes = [long]$process.workingSetBytes
                                    ioDataBytesPerSecond = $ioBytesPerSecond
                                })
                            }
                        }
                    }
                    $sample = [ordered]@{
                        schemaVersion = 1
                        capturedAtUtc = $capturedAt.ToString('O')
                        status = 'captured'
                        samplingIntervalSeconds = [math]::Round($elapsedSeconds, 3)
                        samplerProcessId = $PID
                        processor = $null
                        memory = $null
                        disk = $null
                        contention = $null
                        topCpuProcesses = @()
                        topIoProcesses = @()
                        error = $null
                    }
                    $sample.processor = [ordered]@{
                        percentProcessorTime = [math]::Round($values.processorTotal, 2)
                        percentPrivilegedTime = [math]::Round($values.processorPrivileged, 2)
                    }
                    $sample.memory = [ordered]@{
                        availableMBytes = [math]::Round($values.memoryAvailable, 2)
                        percentCommittedBytesInUse = [math]::Round($values.memoryCommitted, 2)
                        pagesInputPerSecond = [math]::Round($values.memoryPagesInput, 2)
                    }
                    $sample.disk = [ordered]@{
                        bytesPerSecond = [math]::Round($values.diskBytes, 2)
                        percentDiskTime = [math]::Round($values.diskTime, 2)
                        currentQueueLength = [math]::Round($values.diskQueue, 2)
                        splitIoPerSecond = [math]::Round($values.diskSplit, 2)
                    }
                    $sample.contention = [ordered]@{
                        processorQueueLength = [math]::Round($values.processorQueue, 2)
                        contextSwitchesPerSecond = [math]::Round($values.contextSwitches, 2)
                        processes = [math]::Round($values.processes, 0)
                        threads = [math]::Round($values.threads, 0)
                    }
                    $sample.topCpuProcesses = @($processDeltas |
                        Where-Object { $_.percentProcessorTime -gt 0 } |
                        Sort-Object -Property PercentProcessorTime -Descending |
                        Select-Object -First 8)
                    $sample.topIoProcesses = @($processDeltas |
                        Where-Object { $_.ioDataBytesPerSecond -gt 0 } |
                        Sort-Object -Property IoDataBytesPerSecond -Descending |
                        Select-Object -First 8)
                    $sample | ConvertTo-Json -Depth 6 -Compress | Add-Content -LiteralPath $Destination -Encoding utf8
                    $previousProcesses = $currentProcesses
                    $previousCapturedAt = $capturedAt
                }
            }
            catch {
                [ordered]@{
                    schemaVersion = 1
                    capturedAtUtc = (Get-Date).ToUniversalTime().ToString('O')
                    status = 'unavailable'
                    error = $_.Exception.Message
                } | ConvertTo-Json -Compress | Add-Content -LiteralPath $Destination -Encoding utf8
            }
        }
    }
    catch {
        [ordered]@{
            schemaVersion = 1
            capturedAtUtc = (Get-Date).ToUniversalTime().ToString('O')
            status = 'unavailable'
            error = "Host sampler did not start: $($_.Exception.Message)"
        } | ConvertTo-Json -Compress | Add-Content -LiteralPath $OutputPath -Encoding utf8
        return $null
    }
}

function Stop-HostDiagnosticsSampler([object]$Job) {
    if ($null -eq $Job) {
        return
    }
    try {
        Stop-Job -Job $Job -ErrorAction Stop
        Receive-Job -Job $Job -ErrorAction SilentlyContinue | Out-Null
    }
    catch {
        Write-Warning "Host diagnostics sampler did not stop cleanly: $($_.Exception.Message)"
    }
    finally {
        Remove-Job -Job $Job -Force -ErrorAction SilentlyContinue
    }
}

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$project = Join-Path $repoRoot 'apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj'
if (-not (Test-Path -LiteralPath $project -PathType Leaf)) {
    throw "Infrastructure test project was not found: $project"
}

if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
    $EvidenceDirectory = Join-Path $repoRoot "artifacts/windows-recycle-bin-acceptance/$stamp"
}
$evidenceRoot = [IO.Path]::GetFullPath($EvidenceDirectory)
$repoBoundary = $repoRoot.TrimEnd('\') + '\'
if (-not $evidenceRoot.StartsWith($repoBoundary, [StringComparison]::OrdinalIgnoreCase)) {
    throw "EvidenceDirectory must stay inside the repository: $evidenceRoot"
}
$evidencePathToInspect = $evidenceRoot
while (-not $evidencePathToInspect.Equals($repoRoot, [StringComparison]::OrdinalIgnoreCase)) {
    if (Test-Path -LiteralPath $evidencePathToInspect) {
        $evidencePathItem = Get-Item -LiteralPath $evidencePathToInspect -Force
        if (-not $evidencePathItem.PSIsContainer) {
            throw "EvidenceDirectory path component is not a directory: $evidencePathToInspect"
        }
        if ($evidencePathItem.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "EvidenceDirectory must not traverse a reparse point: $evidencePathToInspect"
        }
    }
    $evidenceParent = [IO.Path]::GetDirectoryName($evidencePathToInspect)
    if ([string]::IsNullOrWhiteSpace($evidenceParent) -or $evidenceParent -eq $evidencePathToInspect) {
        throw "EvidenceDirectory parent validation failed: $evidenceRoot"
    }
    $evidencePathToInspect = $evidenceParent
}
if (Test-Path -LiteralPath $evidenceRoot) {
    $existingEvidence = @(Get-ChildItem -LiteralPath $evidenceRoot -Force)
    if ($existingEvidence.Count -gt 0) {
        throw "EvidenceDirectory must be new or empty so an earlier run cannot be overwritten: $evidenceRoot"
    }
}
else {
    $null = New-Item -ItemType Directory -Path $evidenceRoot
}

$matrix = [Collections.Generic.List[object]]::new()
$commands = [Collections.Generic.List[object]]::new()
$measurements = [Collections.Generic.List[object]]::new()
$failure = $null
$gitHead = (& git -c safe.directory=C:/Users/gary/workspace/super-duper rev-parse HEAD).Trim()
$os = [ordered]@{
    caption = [Runtime.InteropServices.RuntimeInformation]::OSDescription
    version = [Environment]::OSVersion.Version.ToString()
    build = [Environment]::OSVersion.Version.Build
}
$computer = [ordered]@{
    model = $null
    logicalProcessors = [Environment]::ProcessorCount
    totalPhysicalMemory = $null
}
$video = @()
try {
    $cimOs = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
    $cimComputer = Get-CimInstance Win32_ComputerSystem -ErrorAction Stop
    $os = [ordered]@{ caption = $cimOs.Caption; version = $cimOs.Version; build = $cimOs.BuildNumber }
    $computer = [ordered]@{
        model = $cimComputer.Model
        logicalProcessors = $cimComputer.NumberOfLogicalProcessors
        totalPhysicalMemory = $cimComputer.TotalPhysicalMemory
    }
    $video = @(Get-CimInstance Win32_VideoController -ErrorAction Stop | ForEach-Object {
        [ordered]@{ name = $_.Name; currentHorizontalResolution = $_.CurrentHorizontalResolution; currentVerticalResolution = $_.CurrentVerticalResolution }
    })
}
catch {
    Write-Warning "CIM host detail is unavailable; recording process-visible fallbacks: $($_.Exception.Message)"
}

Push-Location $repoRoot
try {
    if (-not $SkipBuild) {
        $build = Invoke-LoggedNative 'infrastructure-build' 'dotnet' @(
            'build', $project, '-c', $Configuration, '--no-restore'
        )
        Assert-Executed $build
    }

    $contract = Invoke-LoggedNative 'deterministic-executor-contract' 'dotnet' @(
        'test', $project, '-c', $Configuration, '--no-build', '--no-restore',
        '--filter', 'FullyQualifiedName~WindowsRecycleOperationExecutorTests&TestCategory!=RealRecycleBin',
        '--logger', "trx;LogFileName=deterministic-executor-contract.trx",
        '--results-directory', $evidenceRoot
    )
    Assert-Executed $contract
    Add-MatrixEntry 'Deterministic callback/HRESULT/flag contract' 'passed' `
        'Focused Infrastructure tests and TRX evidence.' `
        'Stable mapping coverage is not a real Shell/provider outcome.'

    if ($ConfirmRecycleBinMutation) {
        $mutation = Invoke-LoggedNative 'real-recycle-bin' 'pwsh' @(
            '-NoProfile', '-File', (Join-Path $PSScriptRoot 'Invoke-WindowsRecycleBinSmoke.ps1'),
            '-Configuration', $Configuration, '-ConfirmRecycleBinMutation', '-SkipBuild'
        )
        Assert-Executed $mutation
        Add-MatrixEntry 'Local success and callback/abort evidence' 'passed' `
            'Disposable hard-link and exact-folder items produced real Shell results.' `
            'Fixtures remain recoverable in the current user Recycle Bin.'
        Add-MatrixEntry 'Post-start cancellation' 'passed' `
            'Real PreDeleteItem cancellation kept its source unchanged.' `
            'Does not prove cancellation during provider work.'
        Add-MatrixEntry 'Locked-file mapping' 'passed' `
            'Real locked source remained byte-identical and returned structured sharing violation.' `
            'Host evidence is not a provider-wide contract.'
    }
    else {
        Add-MatrixEntry 'Local success and callback/abort evidence' 'not_run' `
            'Mutation was not explicitly authorized for this run.' `
            'Use -ConfirmRecycleBinMutation; successful fixtures remain recoverable and are not permanently cleaned.'
        Add-MatrixEntry 'Post-start cancellation' 'not_run' `
            'Mutation was not explicitly authorized for this run.' `
            'Existing automated seams do not replace a real Shell pass.'
        Add-MatrixEntry 'Locked-file mapping' 'not_run' `
            'Mutation was not explicitly authorized for this run.' `
            'Deterministic HRESULT mapping alone does not close this gate.'
    }

    if ($RunProviderNoHydration) {
        foreach ($required in @(
            @{ name = 'CloudRoot'; value = $CloudRoot },
            @{ name = 'LocallyAvailableFile'; value = $LocallyAvailableFile },
            @{ name = 'OfflinePlaceholder'; value = $OfflinePlaceholder }
        )) {
            if ([string]::IsNullOrWhiteSpace($required.value)) {
                throw "$($required.name) is required with -RunProviderNoHydration. Fixture discovery is intentionally not automatic."
            }
        }
        if ($null -eq $ProviderProcessName -or $ProviderProcessName.Count -eq 0) {
            throw 'ProviderProcessName is required with -RunProviderNoHydration.'
        }
        $env:SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS = '1'
        $env:SUPER_DUPER_RECYCLE_CLOUD_ROOT = $CloudRoot
        $env:SUPER_DUPER_RECYCLE_LOCAL_FILE = $LocallyAvailableFile
        $env:SUPER_DUPER_RECYCLE_OFFLINE_FILE = $OfflinePlaceholder
        $env:SUPER_DUPER_RECYCLE_PROVIDER_PROCESSES = $ProviderProcessName -join ';'
        $provider = Invoke-LoggedNative 'provider-no-hydration' 'dotnet' @(
            'test', $project, '-c', $Configuration, '--no-build', '--no-restore',
            '--filter', 'TestCategory=RealRecycleBinProvider',
            '--logger', "trx;LogFileName=provider-no-hydration.trx",
            '--results-directory', $evidenceRoot,
            '--logger', 'console;verbosity=detailed'
        )
        Assert-Executed $provider
        Add-MatrixEntry 'Registered-provider no hydration' 'passed' `
            'Explicit local/offline fixtures retained attributes, allocation, timestamps, and stable provider transfer counters.' `
            'Metadata-only executor inspection; excluded content was never opened or read.'
    }
    else {
        Add-MatrixEntry 'Registered-provider no hydration' 'not_run' `
            'No explicit registered-root fixtures were supplied.' `
            'The script never auto-discovers fixtures or opens excluded content.'
    }

    if ($RunWarmQueryProfile) {
        $profileDiagnosticsPath = Join-Path $evidenceRoot 'representative-review-warm-query.json'
        $hostDiagnosticsPath = Join-Path $evidenceRoot 'representative-review-host-context.jsonl'
        $hostSampler = Start-HostDiagnosticsSampler $hostDiagnosticsPath
        try {
            $env:SUPER_DUPER_REVIEW_PROFILE_EVIDENCE = $profileDiagnosticsPath
            $profile = Invoke-LoggedNative 'representative-review-warm-query' 'cargo' @(
                'test', '-p', 'super-duper-core', '--release', '--test', 'storage_tests',
                'representative_review_workspace_profile', '--', '--ignored', '--exact', '--nocapture', '--test-threads=1'
            )
        }
        finally {
            Remove-Item Env:SUPER_DUPER_REVIEW_PROFILE_EVIDENCE -ErrorAction SilentlyContinue
            Stop-HostDiagnosticsSampler $hostSampler
        }
        $profileDiagnostics = $null
        $profileDiagnosticsError = $null
        if (Test-Path -LiteralPath $profileDiagnosticsPath -PathType Leaf) {
            try {
                $profileDiagnostics = Get-Content -LiteralPath $profileDiagnosticsPath -Raw | ConvertFrom-Json
            }
            catch {
                $profileDiagnosticsError = $_.Exception.Message
            }
        }
        $hostSamples = [Collections.Generic.List[object]]::new()
        $hostInvalidSampleCount = 0
        if (Test-Path -LiteralPath $hostDiagnosticsPath -PathType Leaf) {
            Get-Content -LiteralPath $hostDiagnosticsPath | ForEach-Object {
                if (-not [string]::IsNullOrWhiteSpace($_)) {
                    try {
                        $hostSamples.Add(($_ | ConvertFrom-Json))
                    }
                    catch {
                        $hostInvalidSampleCount++
                    }
                }
            }
        }
        $profileLine = Get-Content -LiteralPath (Join-Path $evidenceRoot 'representative-review-warm-query.log') |
            Where-Object { $_ -like 'review-profile samples=*' } |
            Select-Object -Last 1
        $profileMeasurement = [ordered]@{
            name = 'representative_review_workspace_profile'
            samples = $null
            groupsP50Milliseconds = $null
            groupsP95Milliseconds = $null
            groupsP99Milliseconds = $null
            rootFacetsP95Milliseconds = $null
            driveFacetsP95Milliseconds = $null
            reviewPlanP95Milliseconds = $null
            reviewGroupsP95Milliseconds = $null
            privateGrowthBytes = $null
            testPassed = $profile.exitCode -eq 0
            acceptedAsRepresentative = $false
            acceptanceOverrideAllowed = $false
            queryDiagnostics = $profileDiagnosticsPath
            queryDiagnosticsCaptured = $null -ne $profileDiagnostics
            queryDiagnosticsParseError = $profileDiagnosticsError
            queryTimingDistributions = if ($null -ne $profileDiagnostics) { @($profileDiagnostics.queryTimingDistributions) } else { @() }
            processSnapshotCount = if ($null -ne $profileDiagnostics) { @($profileDiagnostics.processSnapshots).Count } else { 0 }
            hostDiagnostics = $hostDiagnosticsPath
            hostSampleCount = $hostSamples.Count
            hostInvalidSampleCount = $hostInvalidSampleCount
            hostUnavailableSampleCount = @($hostSamples | Where-Object { $_.status -ne 'captured' }).Count
            interpretation = 'p50/p75/p90 describe stable cost; p95/p99/max and aligned host/process counters describe tails. Host contention is diagnostic context only and never waives the 100 ms p95 target.'
        }
        if ($profileLine -match 'samples=(?<samples>\d+) groups-p50=(?<groupsP50>[\d.]+)ms groups-p95=(?<groupsP95>[\d.]+)ms groups-p99=(?<groupsP99>[\d.]+)ms root-facets-p95=(?<rootP95>[\d.]+)ms drive-facets-p95=(?<driveP95>[\d.]+)ms review-plan-p95=(?<planP95>[\d.]+)ms review-groups-p95=(?<reviewP95>[\d.]+)ms private-growth=(?<growth>\d+) bytes') {
            $profileMeasurement.samples = [int]$Matches.samples
            $profileMeasurement.groupsP50Milliseconds = [double]$Matches.groupsP50
            $profileMeasurement.groupsP95Milliseconds = [double]$Matches.groupsP95
            $profileMeasurement.groupsP99Milliseconds = [double]$Matches.groupsP99
            $profileMeasurement.rootFacetsP95Milliseconds = [double]$Matches.rootP95
            $profileMeasurement.driveFacetsP95Milliseconds = [double]$Matches.driveP95
            $profileMeasurement.reviewPlanP95Milliseconds = [double]$Matches.planP95
            $profileMeasurement.reviewGroupsP95Milliseconds = [double]$Matches.reviewP95
            $profileMeasurement.privateGrowthBytes = [long]$Matches.growth
        }
        $measurements.Add($profileMeasurement)
        if ($profile.exitCode -eq 0) {
            Add-MatrixEntry 'Representative review warm queries' 'measured' `
                'The existing 100-sample Release profile completed; timings are in the evidence log and JSON measurement.' `
                'This is an independent Milestone 8 gate, not large-plan Shell-operation evidence.'
        }
        else {
            Add-MatrixEntry 'Representative review warm queries' 'failed' `
                'The 100-sample Release profile exceeded a numeric ceiling; observed metrics remain captured in the log and JSON measurement.' `
                'The gate remains open and this failure is not large-plan Shell-operation evidence.'
            Assert-Executed $profile
        }
    }
    else {
        Add-MatrixEntry 'Representative review warm queries' 'not_run' `
            'The long-running explicit profile was not requested.' `
            'Use -RunWarmQueryProfile on representative hardware.'
    }
}
catch {
    $failure = $_.Exception.Message
}
finally {
    Remove-Item Env:SUPER_DUPER_REVIEW_PROFILE_EVIDENCE -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_RECYCLE_CLOUD_ROOT -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_RECYCLE_LOCAL_FILE -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_RECYCLE_OFFLINE_FILE -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_RECYCLE_PROVIDER_PROCESSES -ErrorAction SilentlyContinue
    Pop-Location
}

Add-MatrixEntry 'Access-denied Shell outcome' 'open' `
    'Stable Win32/copy-engine mappings are automated.' `
    'A real disposable ACL/elevation pass has not been captured.'
Add-MatrixEntry 'Root disconnection' 'open' `
    'Stable Win32/copy-engine mappings are automated.' `
    'Requires real removable/mapped/provider media and controlled disconnect.'
Add-MatrixEntry 'Recycle Bin capacity/oversized item' 'open' `
    'Stable copy-engine capacity mappings are automated.' `
    'No test may permanently delete or silently fall back when capacity is unavailable.'
Add-MatrixEntry 'Provider-specific Shell HRESULTs' 'open' `
    'Provider unavailable/failure/paused mappings are automated.' `
    'Real provider outcomes remain required; no placeholder may be hydrated to induce them.'
Add-MatrixEntry 'Residual Shell TOCTOU' 'open' `
    'Admission and PreDelete identity/type/size/time checks are automated.' `
    'The separately reviewed controlled path-swap campaign remains required.'
Add-MatrixEntry 'Ambiguous-start recovery' 'accepted' `
    'The exact development-host process-loss verifier passed retained evidence at artifacts/windows-ambiguous-start/20260823-144048-588.' `
    'Production remains disabled; this evidence does not accept provider, accessibility, performance, or TOCTOU gates.'
Add-MatrixEntry 'Representative large-plan operation performance' 'open' `
    'No qualifying large disposable operation fixture was executed.' `
    'Small local Shell passes and review-query profiles cannot close this gate.'
Add-MatrixEntry 'Five-minute/60-second/30-second/32-entry constants' 'provisional' `
    'Expiry and bound enforcement are automated.' `
    'Usability and safety decisions require the large-plan operator evidence.'
Add-MatrixEntry 'FOFX_ADDUNDORECORD' 'omitted_pending_decision' `
    'The exact flag regression asserts that 0x20000000 is absent.' `
    'Do not add it until separately reviewed operator evidence decides Windows Undo behavior.'
Add-MatrixEntry 'Narrator/NVDA, OS high contrast, multi-monitor DPI' 'open' `
    'Automated UI Automation/layout contracts remain separate.' `
    'Requires physical listening, theme, and per-monitor transition evidence.'
Add-MatrixEntry 'Production wiring' 'disabled' `
    'App composition, WPF submission, and worker executorEnabled remain disabled.' `
    'This matrix never authorizes production execution or Milestone 11 completion.'

$evidence = [ordered]@{
    schemaVersion = 2
    capturedAt = (Get-Date).ToUniversalTime().ToString('O')
    repository = $repoRoot
    gitHead = $gitHead
    configuration = $Configuration
    host = [ordered]@{
        computerModel = $computer.model
        logicalProcessors = $computer.logicalProcessors
        totalPhysicalMemory = $computer.totalPhysicalMemory
        osCaption = $os.caption
        osVersion = $os.version
        osBuild = $os.build
        videoControllers = $video
    }
    commands = $commands
    measurements = $measurements
    matrix = $matrix
    productionEnabled = $false
    milestone11Complete = $false
    evidenceRetention = 'A new or empty directory is required; failed and passing runs are never overwritten by this collector.'
    failure = $failure
}
$jsonPath = Join-Path $evidenceRoot 'acceptance-evidence.json'
$evidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $jsonPath -Encoding utf8

$report = [Collections.Generic.List[string]]::new()
$report.Add('# Windows Recycle Bin acceptance evidence')
$report.Add('')
$report.Add("Captured: $($evidence.capturedAt)")
$report.Add("")
$report.Add("Git HEAD: ``$gitHead``")
$report.Add('')
$report.Add('| Gate | Status | Evidence | Boundary |')
$report.Add('|---|---|---|---|')
foreach ($entry in $matrix) {
    $report.Add("| $($entry.gate -replace '\|', '\|') | ``$($entry.status)`` | $($entry.evidence -replace '\|', '\|') | $($entry.boundary -replace '\|', '\|') |")
}
$report.Add('')
$report.Add('Production wiring remains disabled and this evidence does not claim Milestone 11 complete.')
if ($null -ne $failure) {
    $report.Add('')
    $report.Add("Run failure: $failure")
}
$reportPath = Join-Path $evidenceRoot 'acceptance-report.md'
$report | Set-Content -LiteralPath $reportPath -Encoding utf8

Write-Output "ACCEPTANCE_EVIDENCE=$jsonPath"
Write-Output "ACCEPTANCE_REPORT=$reportPath"
Write-Output 'PRODUCTION_EXECUTOR_ENABLED=false'
Write-Output 'MILESTONE_11_COMPLETE=false'
if ($null -ne $failure) {
    throw $failure
}
