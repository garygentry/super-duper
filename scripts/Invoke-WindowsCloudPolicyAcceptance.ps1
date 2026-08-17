[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Debug',
    [string]$CloudRoot = $env:OneDriveConsumer,
    [string]$LocallyAvailableFile,
    [string]$OfflinePlaceholder,
    [string]$ProviderId = 'operator-registered-provider',
    [string]$ProviderName = 'Registered cloud provider',
    [string[]]$ProviderProcessName = @('OneDrive.exe'),
    [switch]$ExpectProviderUnavailable,
    [switch]$SkipBuild,
    [switch]$KeepArtifacts
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$profile = if ($Configuration -eq 'Release') { 'release' } else { 'debug' }
$worker = Join-Path $repo "target/$profile/super-duper-worker.exe"
$stateRoot = Join-Path ([IO.Path]::GetTempPath()) ("super-duper-cloud-acceptance-" + [guid]::NewGuid().ToString('N'))
$database = Join-Path $stateRoot 'acceptance.db'
$cache = Join-Path $stateRoot 'hash-cache'
$connection = $null

function Assert-Acceptance([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw "Cloud-policy acceptance failed: $Message" }
}

function Invoke-Checked([scriptblock]$Command, [string]$Description) {
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

function Get-NormalizedPath([string]$Path) {
    $value = [IO.Path]::GetFullPath($Path).Replace('/', '\')
    if ($value.StartsWith('\\?\UNC\', [StringComparison]::OrdinalIgnoreCase)) {
        $value = '\\' + $value.Substring(8)
    } elseif ($value.StartsWith('\\?\', [StringComparison]::OrdinalIgnoreCase)) {
        $value = $value.Substring(4)
    }
    return $value.TrimEnd('\')
}

function Test-PathWithin([string]$Path, [string]$Ancestor) {
    $candidate = Get-NormalizedPath $Path
    $parent = Get-NormalizedPath $Ancestor
    return $candidate.Equals($parent, [StringComparison]::OrdinalIgnoreCase) -or
        $candidate.StartsWith($parent + '\', [StringComparison]::OrdinalIgnoreCase)
}

function Get-FileState([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    Assert-Acceptance (-not $item.PSIsContainer) "Fixture is not a file: $Path"
    $attributes = [uint32][IO.File]::GetAttributes($item.FullName)
    $allocationHigh = [uint32]0
    $allocationLow = [CloudPolicyAcceptance.NativeMethods]::GetCompressedFileSize(
        $item.FullName,
        [ref]$allocationHigh)
    $lastError = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
    if ($allocationLow -eq [uint32]::MaxValue -and $lastError -ne 0) {
        throw "GetCompressedFileSize failed for $Path with Win32 error $lastError."
    }
    [pscustomobject]@{
        Path = $item.FullName
        Length = [uint64]$item.Length
        AllocationBytes = ([uint64]$allocationHigh -shl 32) -bor [uint64]$allocationLow
        Attributes = $attributes
        AttributesHex = '0x{0:X8}' -f $attributes
        PlaceholderStateFlags = $attributes -band [uint32]0x005C1000
        LastWriteTimeUtcTicks = $item.LastWriteTimeUtc.Ticks
    }
}

function Assert-FileStateUnchanged($Before, $After, [string]$Label) {
    foreach ($property in @('Length', 'AllocationBytes', 'Attributes', 'PlaceholderStateFlags', 'LastWriteTimeUtcTicks')) {
        Assert-Acceptance ($Before.$property -eq $After.$property) "$Label $property changed from $($Before.$property) to $($After.$property)."
    }
}

function Get-ProviderTransferSnapshot {
    $names = @($ProviderProcessName | ForEach-Object { $_.ToLowerInvariant() })
    $processes = @(Get-CimInstance Win32_Process | Where-Object { $_.Name.ToLowerInvariant() -in $names })
    $snapshot = @{}
    foreach ($process in $processes) {
        $key = "$($process.Name)|$($process.ProcessId)"
        $snapshot[$key] = [pscustomobject]@{
            ReadTransferCount = [uint64]$process.ReadTransferCount
            WriteTransferCount = [uint64]$process.WriteTransferCount
            OtherTransferCount = [uint64]$process.OtherTransferCount
        }
    }
    return $snapshot
}

function Assert-ProviderTransferUnchanged($Before, $After) {
    Assert-Acceptance ($Before.Count -eq $After.Count) 'Provider process set changed during the scan.'
    foreach ($key in $Before.Keys) {
        Assert-Acceptance $After.ContainsKey($key) "Provider process $key exited or restarted during the scan."
        foreach ($property in @('ReadTransferCount', 'WriteTransferCount', 'OtherTransferCount')) {
            Assert-Acceptance ($Before[$key].$property -eq $After[$key].$property) "Provider process $key $property changed during the scan. Pause unrelated sync activity and rerun."
        }
    }
}

function Start-AcceptanceWorker {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $worker
    $start.WorkingDirectory = Split-Path $worker
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.Environment['SUPER_DUPER_DB_PATH'] = $database
    $start.Environment['HASH_CACHE_PATH'] = $cache
    $process = [Diagnostics.Process]::Start($start)
    Assert-Acceptance ($null -ne $process) 'Windows did not start the worker.'
    [pscustomobject]@{
        Process = $process
        Stderr = $process.StandardError.ReadToEndAsync()
        NextId = 0
        Stopped = $false
    }
}

function Read-WorkerFrame($Connection, [int]$TimeoutSeconds = 30) {
    $read = $Connection.Process.StandardOutput.ReadLineAsync()
    if (-not $read.Wait([TimeSpan]::FromSeconds($TimeoutSeconds))) {
        throw 'Timed out waiting for a worker protocol frame.'
    }
    $line = $read.Result
    if ($null -eq $line) { throw 'Worker stdout closed unexpectedly.' }
    try { return $line | ConvertFrom-Json -Depth 30 }
    catch { throw "Worker stdout was not protocol JSON: $line" }
}

function Send-WorkerRequest($Connection, [string]$Method, $Parameters) {
    $Connection.NextId++
    $id = $Connection.NextId.ToString([Globalization.CultureInfo]::InvariantCulture)
    $request = @{ type = 'request'; id = $id; method = $Method; params = $Parameters } |
        ConvertTo-Json -Compress -Depth 30
    $Connection.Process.StandardInput.WriteLine($request)
    $Connection.Process.StandardInput.Flush()
    while ($true) {
        $frame = Read-WorkerFrame $Connection
        if ($frame.type -eq 'response' -and $frame.id -eq $id) {
            if (-not $frame.ok) {
                throw "$Method failed: $($frame.error.code): $($frame.error.message)"
            }
            return $frame.result
        }
    }
}

function Wait-RunTerminal($Connection, [long]$RunId, [int]$TimeoutSeconds = 60) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $frame = Read-WorkerFrame $Connection 30
        if ($frame.type -eq 'event' -and
            $frame.event -in @('run.completed', 'run.cancelled', 'run.failed') -and
            [long]$frame.data.run.id -eq $RunId) {
            return $frame.data.run
        }
    }
    throw "Timed out waiting for run $RunId."
}

function Stop-AcceptanceWorker($Connection) {
    if ($null -eq $Connection -or $Connection.Stopped) { return }
    $Connection.Stopped = $true
    $Connection.Process.StandardInput.Close()
    if (-not $Connection.Process.WaitForExit(15000)) {
        throw 'Worker did not exit after protocol EOF.'
    }
    $stderr = $Connection.Stderr.GetAwaiter().GetResult()
    Assert-Acceptance ($Connection.Process.ExitCode -eq 0) "Worker exited with code $($Connection.Process.ExitCode): $stderr"
    $Connection.Process.Dispose()
}

function Invoke-ExcludedRun(
    $Connection,
    [string]$Name,
    [string]$Root,
    [string[]]$ManualExclusions,
    [string]$ExpectedExcludedPath) {
    $session = (Send-WorkerRequest $Connection 'session.create' @{
        name = $Name
        roots = @($Root)
        ignorePatterns = @()
        cloudPolicy = 'exclude_registered_roots'
        manualLocationExclusions = $ManualExclusions
        registeredCloudLocations = @(@{
            path = $script:resolvedCloudRoot
            providerId = $ProviderId
            displayName = $ProviderName
        })
        cloudDetectionStatus = 'complete'
    }).session
    $started = (Send-WorkerRequest $Connection 'run.start' @{ sessionId = $session.id }).run
    $completed = Wait-RunTerminal $Connection $started.id
    Assert-Acceptance ($completed.status -eq 'completed') "$Name ended as $($completed.status): $($completed.errorMessage)"
    Assert-Acceptance ([long]$completed.filesDiscovered -eq 0) "$Name discovered $($completed.filesDiscovered) files."
    $page = Send-WorkerRequest $Connection 'run_exclusion.page' @{ runId = $completed.id; offset = 0; limit = 500 }
    $cloudExclusion = @($page.exclusions | Where-Object {
        $_.reasonCode -eq 'registered_cloud_root_excluded' -and
        (Get-NormalizedPath $_.path).Equals(
            (Get-NormalizedPath $ExpectedExcludedPath),
            [StringComparison]::OrdinalIgnoreCase)
    })
    Assert-Acceptance ($cloudExclusion.Count -eq 1) "$Name did not record exactly one registered cloud-root exclusion for $ExpectedExcludedPath."
    $durable = (Send-WorkerRequest $Connection 'run.get' @{ runId = $completed.id }).run
    Assert-Acceptance ($durable.parameters.cloudPolicy -eq 'exclude_registered_roots') "$Name lost its immutable cloud-policy snapshot."
    Assert-Acceptance ([long]$durable.excludedSubtreeCount -eq [long]$page.total) "$Name exclusion summary and page total differ."
    return $durable
}

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace CloudPolicyAcceptance {
    public static class NativeMethods {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern uint GetCompressedFileSize(string fileName, out uint fileSizeHigh);
    }
}
'@

try {
    Assert-Acceptance (-not [string]::IsNullOrWhiteSpace($CloudRoot)) 'CloudRoot is required (or OneDriveConsumer must be set).'
    $resolvedCloudRoot = (Resolve-Path -LiteralPath $CloudRoot).Path.TrimEnd('\')
    $broadAncestor = [IO.Directory]::GetParent($resolvedCloudRoot).FullName.TrimEnd('\')
    Assert-Acceptance (Test-PathWithin $resolvedCloudRoot $broadAncestor) 'Cloud root is not inside its broad ancestor.'

    $metadataFiles = $null
    if ([string]::IsNullOrWhiteSpace($OfflinePlaceholder) -or [string]::IsNullOrWhiteSpace($LocallyAvailableFile)) {
        $metadataFiles = @(Get-ChildItem -LiteralPath $resolvedCloudRoot -File -Force -Recurse -Depth 6 -ErrorAction SilentlyContinue)
    }
    if ([string]::IsNullOrWhiteSpace($OfflinePlaceholder)) {
        $offline = $metadataFiles | Where-Object {
            (([uint32]$_.Attributes) -band [uint32]0x00441000) -ne 0
        } | Sort-Object Length | Select-Object -First 1
        Assert-Acceptance ($null -ne $offline) 'No offline/recall-on-access placeholder was found. Supply -OfflinePlaceholder explicitly.'
        $OfflinePlaceholder = $offline.FullName
    }
    if ([string]::IsNullOrWhiteSpace($LocallyAvailableFile)) {
        $local = $metadataFiles | Where-Object {
            $_.Length -ge 1024 -and
            (([uint32]$_.Attributes) -band [uint32]0x00441000) -eq 0 -and
            (([uint32]$_.Attributes) -band [uint32]0x00000006) -eq 0
        } | Sort-Object Length | Select-Object -First 1
        Assert-Acceptance ($null -ne $local) 'No locally available file was found. Supply -LocallyAvailableFile explicitly.'
        $LocallyAvailableFile = $local.FullName
    }

    $localBefore = Get-FileState $LocallyAvailableFile
    $offlineBefore = Get-FileState $OfflinePlaceholder
    Assert-Acceptance (Test-PathWithin $localBefore.Path $resolvedCloudRoot) 'Locally available fixture is outside CloudRoot.'
    Assert-Acceptance (Test-PathWithin $offlineBefore.Path $resolvedCloudRoot) 'Offline placeholder fixture is outside CloudRoot.'
    Assert-Acceptance ($localBefore.PlaceholderStateFlags -eq 0) 'Locally available fixture has offline/recall placeholder flags.'
    Assert-Acceptance ($offlineBefore.PlaceholderStateFlags -ne 0) 'Offline fixture does not have offline/recall placeholder flags.'

    $providerBefore = Get-ProviderTransferSnapshot
    if ($ExpectProviderUnavailable) {
        Assert-Acceptance ($providerBefore.Count -eq 0) 'Provider process is running; make it unavailable before using -ExpectProviderUnavailable.'
    } else {
        Assert-Acceptance ($providerBefore.Count -gt 0) 'No provider process was found. Use -ExpectProviderUnavailable only after intentionally making it unavailable.'
    }

    if (-not $SkipBuild) {
        Push-Location $repo
        try {
            if ($Configuration -eq 'Release') {
                Invoke-Checked { cargo build -p super-duper-worker --release } 'Release worker build'
            } else {
                Invoke-Checked { cargo build -p super-duper-worker } 'Debug worker build'
            }
        }
        finally { Pop-Location }
    }
    Assert-Acceptance (Test-Path -LiteralPath $worker -PathType Leaf) "Worker was not found at $worker."

    $infraProject = Join-Path $repo 'apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj'
    $savedExpectedRoot = $env:SUPER_DUPER_EXPECTED_CLOUD_ROOT
    $env:SUPER_DUPER_EXPECTED_CLOUD_ROOT = $resolvedCloudRoot
    Push-Location ([IO.Path]::GetTempPath())
    try {
        Invoke-Checked {
            dotnet test $infraProject --configuration $Configuration --filter 'FullyQualifiedName~DetectAsync_FindsOperatorExpectedWindowsRegistrationWithoutContentAccess' --verbosity minimal
        } 'Real Windows registration discovery test'
    }
    finally {
        Pop-Location
        if ($null -eq $savedExpectedRoot) {
            Remove-Item Env:SUPER_DUPER_EXPECTED_CLOUD_ROOT -ErrorAction SilentlyContinue
        } else {
            $env:SUPER_DUPER_EXPECTED_CLOUD_ROOT = $savedExpectedRoot
        }
    }

    [IO.Directory]::CreateDirectory($stateRoot) | Out-Null
    $manualSiblingExclusions = @(Get-ChildItem -LiteralPath $broadAncestor -Force | Where-Object {
        -not (Test-PathWithin $resolvedCloudRoot $_.FullName)
    } | ForEach-Object FullName)

    $connection = Start-AcceptanceWorker
    $hello = Send-WorkerRequest $connection 'hello' @{
        protocolVersions = @(1)
        client = @{ name = 'cloud-policy-operator-acceptance'; version = '1.0.0' }
    }
    Assert-Acceptance ($hello.protocolVersion -eq 1) 'Worker did not negotiate protocol V1.'

    $broadRun = Invoke-ExcludedRun $connection 'Cloud acceptance broad ancestor' $broadAncestor $manualSiblingExclusions $resolvedCloudRoot
    $explicitRun = Invoke-ExcludedRun $connection 'Cloud acceptance explicit root' $resolvedCloudRoot @() $resolvedCloudRoot
    Stop-AcceptanceWorker $connection
    $connection = $null

    $localAfter = Get-FileState $LocallyAvailableFile
    $offlineAfter = Get-FileState $OfflinePlaceholder
    Assert-FileStateUnchanged $localBefore $localAfter 'Locally available file'
    Assert-FileStateUnchanged $offlineBefore $offlineAfter 'Offline placeholder'
    $providerAfter = Get-ProviderTransferSnapshot
    Assert-ProviderTransferUnchanged $providerBefore $providerAfter

    Write-Output 'Windows cloud-policy operator acceptance passed.'
    Write-Output "CLOUD_ROOT=$resolvedCloudRoot"
    Write-Output "BROAD_ANCESTOR=$broadAncestor"
    Write-Output "LOCAL_FILE_STATE=$($localAfter.AttributesHex)|ALLOCATED=$($localAfter.AllocationBytes)|LOGICAL=$($localAfter.Length)"
    Write-Output "OFFLINE_FILE_STATE=$($offlineAfter.AttributesHex)|ALLOCATED=$($offlineAfter.AllocationBytes)|LOGICAL=$($offlineAfter.Length)"
    Write-Output "PROVIDER_STATE=$(if ($ExpectProviderUnavailable) { 'unavailable' } else { 'available' })"
    Write-Output 'PROVIDER_TRANSFER_COUNTERS_UNCHANGED=true'
    Write-Output "BROAD_RUN_ID=$($broadRun.id)|EXCLUDED=$($broadRun.excludedSubtreeCount)|FILES=$($broadRun.filesDiscovered)"
    Write-Output "EXPLICIT_RUN_ID=$($explicitRun.id)|EXCLUDED=$($explicitRun.excludedSubtreeCount)|FILES=$($explicitRun.filesDiscovered)"
    Write-Output "ACCEPTANCE_STATE=$stateRoot"
}
finally {
    if ($null -ne $connection) {
        try { Stop-AcceptanceWorker $connection } catch { Write-Warning $_.Exception.Message }
    }
    if (-not $KeepArtifacts -and (Test-Path -LiteralPath $stateRoot)) {
        $resolvedState = (Resolve-Path -LiteralPath $stateRoot).Path
        $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
        $stateItem = Get-Item -LiteralPath $resolvedState -Force
        if (-not $resolvedState.StartsWith($tempRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
            -not $stateItem.PSIsContainer -or
            ($stateItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            -not ([IO.Path]::GetFileName($resolvedState)).StartsWith('super-duper-cloud-acceptance-', [StringComparison]::Ordinal)) {
            throw "Refusing to clean unexpected cloud acceptance state path: $resolvedState"
        }
        Remove-Item -LiteralPath $resolvedState -Recurse -Force
    }
}
