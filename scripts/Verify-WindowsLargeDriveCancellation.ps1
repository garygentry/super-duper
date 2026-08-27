[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$campaignId = 'sop9b-representative-cancellation-v1'
$campaignRoot = Join-Path $repo "artifacts/windows-sop9-large-drive/$campaignId"
$evidencePath = Join-Path $campaignRoot 'acceptance-evidence.json'
$journalPath = Join-Path $campaignRoot 'attempt.jsonl'
$manifestPath = Join-Path $campaignRoot 'manifest.json'
$summaryPath = Join-Path $repo 'docs/evidence/scan-large-drive-cancellation-20260827.json'
$runnerPath = Join-Path $PSScriptRoot 'Invoke-WindowsLargeDriveAcceptance.ps1'
$schedulerPath = Join-Path $repo 'crates/super-duper-core/src/hasher/scheduler.rs'
$readPath = Join-Path $repo 'crates/super-duper-core/src/hasher/xxhash.rs'
$runModelsPath = Join-Path $repo 'crates/super-duper-core/src/storage/models.rs'
$operationViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$workerSource = Join-Path $repo 'crates/super-duper-worker/src/lib.rs'

function Assert-True([bool]$Condition, [string]$Failure) {
    if (-not $Condition) { throw $Failure }
}

function Assert-Equal($Actual, $Expected, [string]$Failure) {
    if ($Actual -ne $Expected) { throw "$Failure Expected $Expected; got $Actual." }
}

function Assert-Contains([string]$Path, [string]$Text, [string]$Failure) {
    if (-not [IO.File]::ReadAllText($Path).Contains($Text, [StringComparison]::Ordinal)) {
        throw $Failure
    }
}

function Assert-Hash([string]$Path, [string]$Expected) {
    Assert-True (Test-Path -LiteralPath $Path -PathType Leaf) "Retained evidence is missing: $Path"
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash
    Assert-Equal $actual $Expected "Retained evidence changed: $Path."
}

Assert-Hash $evidencePath '7057503390C2F4B9111075BCE7B454C06B9F2F23876C6E315018224489A7BE79'
Assert-Hash $journalPath 'E8FB061F18AF370CD07802760418063D2BFF3AA34B8A5CF7956BB63C03A1B2A2'
Assert-Hash $manifestPath '97BF404F5E18C55D680DEE7DB95F3D22F2789FE4ADE2E714001310905D997150'
Assert-Hash $summaryPath '97E902CCEB5E0ECC60A469162F3DFE5AA9E66140FD10D80AD017A75785F7343E'

$evidence = Get-Content -Raw -LiteralPath $evidencePath | ConvertFrom-Json -Depth 100
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json -Depth 100
$summary = Get-Content -Raw -LiteralPath $summaryPath | ConvertFrom-Json -Depth 100
$journal = @(
    [IO.File]::ReadAllLines($journalPath) |
        ForEach-Object { $_ | ConvertFrom-Json -Depth 100 }
)
$events = @($journal | ForEach-Object { [string]$_.event })

Assert-Equal $evidence.schemaVersion 1 'Unexpected campaign evidence schema.'
Assert-Equal $evidence.campaignId $campaignId 'The retained campaign identity changed.'
Assert-Equal $evidence.status 'valid' 'The retained campaign is not valid.'
Assert-True $evidence.physical 'The retained campaign is not marked physical.'
Assert-True ($evidence.noFavorableRetry -and $manifest.noFavorableRetry -and $summary.noFavorableRetry) `
    'The no-favorable-retry contract changed.'
Assert-Equal $evidence.favorableRetryCount 0 'A favorable retry was recorded.'
Assert-Equal $evidence.arms.Count 1 'The cancellation campaign must contain exactly one arm.'
Assert-Equal $manifest.policies.Count 1 'The manifest must contain exactly one policy.'
Assert-Equal $manifest.policies[0] 'revalidate_content' 'The manifest policy changed.'
Assert-True $manifest.cancelAfterFirstHashProgress 'The fixed cancellation trigger changed.'

$arm = $evidence.arms[0]
$product = $arm.snapshot.product
$status = $arm.snapshot.status
$counters = $status.counters
Assert-Equal $arm.ordinal 0 'The arm ordinal changed.'
Assert-Equal $arm.policy 'revalidate_content' 'The immutable forced-read policy changed.'
Assert-Equal $product.repeatCachePolicy 'revalidate_content' 'Product history did not reconstruct forced reads.'
Assert-Equal $arm.terminalStatus 'cancelled' 'The worker did not terminate as cancelled.'
Assert-Equal $product.status 'cancelled' 'Product truth did not retain cancellation.'
Assert-Equal $status.state 'cancelled' 'Status truth did not retain cancellation.'
Assert-True $arm.cancellationRequested 'The retained arm lacks a cancellation request.'
Assert-True ($arm.cancellationLatencyMilliseconds -gt 0) 'Cancellation latency was not measured.'
Assert-True ($arm.progressFrameCount -gt 0 -and $arm.progressSerializedBytes -gt 0) `
    'Progress-frame cost proxies are missing.'
Assert-True ($arm.maximumFramesPerObservedSecond -le 10) `
    'Progress publication exceeded ten observed frames per second.'

Assert-Equal ([UInt64]$counters.partial_hashes_attempted) 256 'Unexpected partial-hash attempt count.'
Assert-Equal ([UInt64]$counters.partial_hashes_succeeded) 256 'Unexpected successful partial-hash count.'
Assert-Equal ([UInt64]$counters.partial_hashes_failed) 0 'A partial hash failed before cancellation.'
Assert-Equal ([UInt64]$counters.partial_hash_bytes_read) 262144 'Unexpected pre-cancellation partial bytes.'
Assert-Equal ([UInt64]$counters.full_hash_bytes_read) 0 'Full-content reads occurred before cancellation.'
Assert-Equal ([UInt64]$counters.cancelled_work_items) 1 'Queued cancellation accounting changed.'
Assert-True ([UInt64]$counters.cancel_checks -gt 0) 'Cancellation checks were not retained.'
Assert-Equal $product.filesHashed 0 'A cancelled arm published hashed product files.'
Assert-Equal $product.duplicateFileGroups 0 'A cancelled arm published duplicate files.'
Assert-Equal $product.duplicateFolderGroups 0 'A cancelled arm published duplicate folders.'
$emptyDigest = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
Assert-Equal $product.fileResultSha256 $emptyDigest 'Cancelled file-result truth is not empty.'
Assert-Equal $product.folderResultSha256 $emptyDigest 'Cancelled folder-result truth is not empty.'

$accountedFiles = [UInt64]$counters.candidate_files + [UInt64]$counters.metadata_resolved_files +
    [UInt64]$counters.hard_link_alias_files + [UInt64]$counters.zero_byte_files
Assert-Equal $accountedFiles ([UInt64]$counters.discovered_files) 'The complete file funnel does not reconcile.'
$accountedBytes = [UInt64]$counters.candidate_bytes + [UInt64]$counters.metadata_resolved_bytes +
    [UInt64]$counters.hard_link_alias_bytes
Assert-Equal $accountedBytes ([UInt64]$counters.discovered_bytes) 'The complete byte funnel does not reconcile.'
Assert-Equal ([UInt64]$counters.singleton_size_files) ([UInt64]$counters.metadata_resolved_files) `
    'Singleton files were not resolved as metadata.'
Assert-Equal ([UInt64]$counters.singleton_size_bytes) ([UInt64]$counters.metadata_resolved_bytes) `
    'Singleton bytes were not resolved as metadata.'
Assert-True ([UInt64]$counters.hard_link_alias_files -gt 0) 'Hard-link behavior was not exercised.'
Assert-Equal ([UInt64]$counters.confirmed_duplicate_groups) 0 `
    'A cancelled arm retained confirmed duplicate groups.'
Assert-Equal $product.warningCount ([UInt64]$counters.warnings) 'Product/status warnings disagree.'
Assert-Equal $product.warningOccurrenceCount $product.warningCount 'Warning occurrences do not reconcile.'
Assert-True $product.warningAccountingComplete 'Warning accounting is incomplete.'
Assert-Equal ([UInt64]$counters.telemetry_flush_errors) 0 'Telemetry flush errors occurred.'
Assert-Equal ([UInt64]$counters.telemetry_samples_lost) 0 'Telemetry samples were lost.'

Assert-True ($status.lastSequence -gt 0 -and $arm.statusDatabaseBytes -gt 0 -and
    $arm.processDelta.writeBytes -gt 0) 'Status/process-write observer proxies are missing.'
Assert-Equal $status.flushCount 0 'Terminal-retained replay rows should have been removed.'
Assert-Equal $status.flushPayloadBytes 0 'Terminal-retained replay bytes should have been removed.'
Assert-True ($status.hostSampleCount -gt 0 -and $status.hostSampleCount -le 100000) `
    'Host sample retention is empty or unbounded.'
Assert-True ($status.devices.Count -eq 1 -and $status.devices.Count -le 64) `
    'The single-device sample set is empty or unbounded.'
Assert-True ($status.devices[0].sampleCount -gt 0 -and $status.devices[0].sampleCount -le 100000) `
    'Device sample retention is empty or unbounded.'
Assert-True ($status.processWorkingSetBytes.maximum -gt 0 -and
    $status.processPrivateBytes.maximum -gt 0) 'Peak memory observations are missing.'
Assert-True ($status.hostUnavailableCounterTotal -gt 0 -and
    $status.devices[0].unavailableCounterTotal -gt 0) `
    'Unavailable counter observations were silently omitted.'

Assert-Equal (@($events | Where-Object { $_ -eq 'attempt_reserved' }).Count) 1 `
    'The write-once identity contains multiple attempt reservations.'
Assert-Equal (@($events | Where-Object { $_ -eq 'arm_started' }).Count) 1 `
    'The write-once identity contains multiple arms.'
Assert-Equal (@($events | Where-Object { $_ -eq 'cancellation_requested' }).Count) 1 `
    'The cancellation request count changed.'
Assert-Equal (@($events | Where-Object { $_ -eq 'run_terminal' }).Count) 1 `
    'The terminal event count changed.'
$cancelEvent = $journal | Where-Object { $_.event -eq 'cancellation_requested' }
Assert-Equal $cancelEvent.data.trigger 'first_hash_read_progress' 'The cancellation trigger changed.'
$terminalIndex = [Array]::IndexOf([string[]]$events, 'run_terminal')
Assert-True ($terminalIndex -ge 0) 'The terminal journal entry is missing.'
for ($index = $terminalIndex + 1; $index -lt $events.Count; $index++) {
    Assert-True ($events[$index] -ne 'run_progress') 'Progress was published after terminal.'
}
Assert-Equal $events[-1] 'evidence_finalized' 'The journal did not finalize evidence last.'

Assert-True (-not $evidence.sop2ObserverRisk.strictGateEvaluated -and
    -not $evidence.sop2ObserverRisk.strictGatePassed -and
    -not $evidence.sop2ObserverRisk.causalAttributionAvailable) `
    'SOP2 representative overhead was retroactively claimed as evaluated or passed.'
Assert-True (-not $summary.sop2ObserverRisk.strictGateEvaluated -and
    -not $summary.sop2ObserverRisk.strictGatePassed) 'The committed summary changed SOP2 risk truth.'
Assert-True ($evidence.cleanup.workerStopped -and -not $evidence.cleanup.workerForced -and
    $evidence.cleanup.stateRemoved -and $evidence.cleanup.errors.Count -eq 0) `
    'Campaign cleanup was not exact.'
$statePath = "H:\super-duper-sop9-state\$campaignId"
Assert-True (-not (Test-Path -LiteralPath $statePath)) 'Validated campaign-owned state still exists.'
Assert-True $summary.writeOnceIdentityConsumed 'The committed summary does not mark the identity consumed.'

Assert-Contains $schedulerPath 'pub(crate) const ROTATIONAL_READERS: usize = 1;' `
    'The SOP6 rotational reader ceiling changed.'
Assert-Contains $schedulerPath 'pub(crate) const UNKNOWN_DEVICE_READERS: usize = 1;' `
    'The SOP6 conservative reader ceiling changed.'
Assert-Contains $schedulerPath 'pub(crate) const SOLID_STATE_READERS: usize = 4;' `
    'The SOP6 solid-state reader ceiling changed.'
Assert-Contains $readPath 'StorageMediaClass::SolidState => SOLID_STATE_STREAM_BUFFER_LENGTH' `
    'The SOP7 media-scoped buffer policy changed.'
Assert-Contains $readPath 'media != crate::platform::StorageMediaClass::SolidState' `
    'The SOP7 sequential-hint policy changed.'
Assert-Contains $runModelsPath 'Self::ReuseVerified' 'The SOP8 reuse policy disappeared.'
Assert-Contains $runModelsPath 'RepeatCachePolicy::RevalidateContent' `
    'Legacy forced-read reconstruction disappeared.'
Assert-Contains $runnerPath "'sop9c-single-drive-reference-repeat-v1'" `
    'The next fixed SOP9 campaign identity changed.'
Assert-Contains $operationViewModel 'public bool CanSubmit => false;' `
    'RecycleOperationViewModel.CanSubmit is no longer locked false.'
Assert-Contains $compositionRoot `
    'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
    'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
Assert-True (-not [IO.File]::ReadAllText($workerSource).Contains(
    '"executorEnabled": true', [StringComparison]::Ordinal)) `
    'A worker response reports executorEnabled:true.'

git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check
if ($LASTEXITCODE -ne 0) { throw 'git diff --check failed.' }

Write-Output 'SOP9b cancellation verifier passed the write-once artifact pins, real-drive cancellation, complete funnel/hard-link/warning/status accounting, bounded telemetry/memory observation, cleanup, SOP2 residual-risk truth, SOP6/SOP7/SOP8 policies, and all production locks.'
