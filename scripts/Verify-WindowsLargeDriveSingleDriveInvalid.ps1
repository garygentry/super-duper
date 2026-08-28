[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$campaignId = 'sop9c-single-drive-reference-repeat-v1'
$campaignRoot = Join-Path $repo "artifacts/windows-sop9-large-drive/$campaignId"
$evidencePath = Join-Path $campaignRoot 'acceptance-evidence.json'
$journalPath = Join-Path $campaignRoot 'attempt.jsonl'
$manifestPath = Join-Path $campaignRoot 'manifest.json'
$stderrPath = Join-Path $campaignRoot 'worker-stderr.log'
$workerBuildPath = Join-Path $campaignRoot 'worker-build.log'
$snapshotBuildPath = Join-Path $campaignRoot 'snapshot-build.log'
$summaryPath = Join-Path $repo 'docs/evidence/scan-large-drive-single-drive-invalid-20260828.json'
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

Assert-Hash $evidencePath 'F1900C634D7F3291F3B2A4A9F91E1D5DB702D2432C081A535346CA00B13E3378'
Assert-Hash $journalPath '8252EB97B00F63009AA3CF40D0746812F73E0BBFBE27933B1ABC343C512C6FEF'
Assert-Hash $manifestPath 'EF91597ADCC8B877E7769F209ADA1D6A37AD45D0D2917EB3E48AFB34EB2ECF17'
Assert-Hash $stderrPath 'AD419954EDCBE0CEFE769A8182A7B5F7F1ADF19FF7D555F87E58128B4AE559A3'
Assert-Hash $workerBuildPath '584E1498A75786370A41671DA1AEFC23B8F32B84883EEF3C6D46524D5815499A'
Assert-Hash $snapshotBuildPath 'D265CB0ED9D7D95792D4A79E177CC4C66897CBD98AF57C4BD074ECEF3D8DBF48'
Assert-Hash $summaryPath 'D5C048A0452F70039B524E845C8698596E713603F9B2A4B8138F0F476EDA61B8'

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
Assert-Equal $evidence.status 'invalid' 'The invalid campaign was reclassified.'
Assert-True $evidence.physical 'The retained campaign is not marked physical.'
Assert-True ($evidence.noFavorableRetry -and $manifest.noFavorableRetry -and
    $summary.noFavorableRetry) 'The no-favorable-retry contract changed.'
Assert-Equal $evidence.favorableRetryCount 0 'A favorable retry was recorded.'
Assert-Equal $evidence.expectedTerminal 'completed' 'The required terminal changed.'
Assert-Equal $evidence.arms.Count 0 'An incomplete arm was represented as measured evidence.'
Assert-True (-not $evidence.comparisons.fileResultsEqual -and
    -not $evidence.comparisons.folderResultsEqual) 'Incomplete results were represented as equal.'
Assert-Equal $evidence.failure `
    'System.Management.Automation.RuntimeException: Timed out waiting for a worker protocol frame.' `
    'The retained failure changed.'

Assert-Equal $manifest.policies.Count 2 'The fixed two-policy campaign changed.'
Assert-Equal $manifest.policies[0] 'revalidate_content' 'The forced-reference arm order changed.'
Assert-Equal $manifest.policies[1] 'reuse_verified' 'The verified-reuse arm order changed.'
Assert-Equal $manifest.expectedTerminal 'completed' 'The manifest terminal changed.'
Assert-True (-not $manifest.cancelAfterFirstHashProgress) 'SOP9c became a cancellation campaign.'
Assert-Equal $manifest.rootIdentitySha256.Count 1 'The fixed single-root identity changed.'
Assert-Equal $manifest.rootIdentitySha256[0] `
    '8697e248ec3caa2ea4732549f9a45e960590a42ff6e26019294af595c187b760' `
    'The hashed root identity changed.'

Assert-Equal (@($events | Where-Object { $_ -eq 'attempt_reserved' }).Count) 1 `
    'The write-once identity contains multiple attempt reservations.'
Assert-Equal (@($events | Where-Object { $_ -eq 'physical_preflight_passed' }).Count) 1 `
    'The physical preflight record changed.'
Assert-Equal (@($events | Where-Object { $_ -eq 'arm_started' }).Count) 1 `
    'The invalid campaign must retain exactly its one started arm.'
Assert-Equal (@($events | Where-Object { $_ -eq 'run_terminal' }).Count) 0 `
    'A worker terminal event was invented after the timeout.'
Assert-Equal (@($events | Where-Object { $_ -eq 'attempt_failed' }).Count) 1 `
    'The timeout failure count changed.'
Assert-Equal (@($events | Where-Object { $_ -eq 'worker_cleanup_failed' }).Count) 1 `
    'The forced-cleanup failure count changed.'
Assert-Equal (@($events | Where-Object { $_ -eq 'evidence_finalized' }).Count) 1 `
    'The invalid evidence finalization count changed.'
Assert-Equal $events[-1] 'evidence_finalized' 'The journal did not finalize invalid evidence last.'

$armStart = $journal | Where-Object { $_.event -eq 'arm_started' } | Select-Object -First 1
Assert-Equal $armStart.data.policy 'revalidate_content' 'The sole started arm was not forced-reference.'
Assert-Equal $armStart.data.ordinal 0 'The sole started arm ordinal changed.'
$reuseStarts = @($journal | Where-Object {
    $_.event -eq 'arm_started' -and $_.data.policy -eq 'reuse_verified'
})
Assert-Equal $reuseStarts.Count 0 'The reuse arm was incorrectly represented as started.'

$lastProgress = $journal | Where-Object { $_.event -eq 'run_progress' } | Select-Object -Last 1
Assert-Equal $lastProgress.data.phase 'persisting' 'The last retained phase changed.'
Assert-Equal ([long]$lastProgress.data.sequence) 20554 'The last retained sequence changed.'
Assert-Equal ([long]$lastProgress.data.progressFrameCount) 20554 'The progress-frame proxy changed.'
Assert-Equal ([UInt64][string]$lastProgress.data.partialHashBytesRead) 583624783 `
    'The last partial-read proxy changed.'
Assert-Equal ([UInt64][string]$lastProgress.data.fullHashBytesRead) 256449196445 `
    'The last full-read proxy changed.'
Assert-Equal $lastProgress.data.unavailableDeviceReason 'mapping_unavailable' `
    'The explicit unavailable-device reason changed.'

$failureEvent = $journal | Where-Object { $_.event -eq 'attempt_failed' } | Select-Object -First 1
$cleanupEvent = $journal | Where-Object { $_.event -eq 'worker_cleanup_failed' } | Select-Object -First 1
$timeoutMilliseconds = (
    [DateTimeOffset]::Parse($failureEvent.utc) - [DateTimeOffset]::Parse($lastProgress.utc)
).TotalMilliseconds
$cleanupMilliseconds = (
    [DateTimeOffset]::Parse($cleanupEvent.utc) - [DateTimeOffset]::Parse($failureEvent.utc)
).TotalMilliseconds
Assert-True ($timeoutMilliseconds -ge 180000 -and $timeoutMilliseconds -lt 181000) `
    'The retained failure no longer occurs at the V1 180-second frame deadline.'
Assert-True ($cleanupMilliseconds -ge 30000 -and $cleanupMilliseconds -lt 31000) `
    'The retained force-stop interval no longer matches the one cleanup attempt.'
Assert-Equal $cleanupEvent.data.error 'Worker exited with code -1.' `
    'The retained worker cleanup error changed.'
Assert-Contains $stderrPath 'performance kind=scan_phase run_id=1 phase=discovering duration_ms=244141.692' `
    'The retained discovery duration is missing.'
Assert-Contains $stderrPath 'performance kind=scan_phase run_id=1 phase=hashing duration_ms=20496093.733' `
    'The retained hashing duration is missing.'

Assert-True (-not $evidence.cleanup.workerStopped -and $evidence.cleanup.workerForced -and
    -not $evidence.cleanup.stateRemovalAttempted -and -not $evidence.cleanup.stateRemoved -and
    $evidence.cleanup.statePreservedForDiagnostics -and $evidence.cleanup.errors.Count -eq 1) `
    'The invalid campaign cleanup truth changed.'
$statePath = "H:\super-duper-sop9-state\$campaignId"
Assert-True (Test-Path -LiteralPath $statePath -PathType Container) `
    'The required diagnostic state is no longer retained.'
$expectedStateNames = @(
    'hash-cache/000008.log',
    'hash-cache/000009.sst',
    'hash-cache/CURRENT',
    'hash-cache/IDENTITY',
    'hash-cache/LOCK',
    'hash-cache/LOG',
    'hash-cache/MANIFEST-000005',
    'hash-cache/OPTIONS-000007',
    'product.db',
    'product.db-shm',
    'product.db-wal',
    'status.db',
    'status.db-shm',
    'status.db-wal'
)
Assert-Equal $summary.diagnosticStateArtifacts.Count $expectedStateNames.Count `
    'The committed diagnostic-state inventory is incomplete.'
Assert-Equal ((@($summary.diagnosticStateArtifacts.name) | Sort-Object) -join '|') `
    (($expectedStateNames | Sort-Object) -join '|') 'The diagnostic-state inventory changed.'
foreach ($artifact in $summary.diagnosticStateArtifacts) {
    $stateArtifactPath = Join-Path $statePath ([string]$artifact.name)
    Assert-Hash $stateArtifactPath ([string]$artifact.sha256)
    $stateArtifact = Get-Item -LiteralPath $stateArtifactPath
    Assert-Equal $stateArtifact.Length ([long]$artifact.bytes) `
        "Diagnostic-state size changed: $($artifact.name)."
    $expectedLastWriteUtc = $artifact.lastWriteUtc.ToUniversalTime().ToString('o')
    Assert-Equal $stateArtifact.LastWriteTimeUtc.ToString('o') $expectedLastWriteUtc `
        "Diagnostic-state timestamp changed: $($artifact.name)."
}

Assert-Equal $summary.status 'blocked_invalid_campaign' 'The committed incident status changed.'
Assert-True $summary.writeOnceIdentityConsumed 'The committed summary does not consume V1.'
Assert-True (-not $summary.blocker.v2Authorized -and -not $summary.blocker.sop9dAuthorized) `
    'The committed summary invented follow-on campaign authority.'
Assert-Equal $summary.measurement.lastProgressFrameCount 20554 `
    'The committed progress-frame proxy changed.'
Assert-Equal ([UInt64]$summary.measurement.fullHashBytesReadAtLastProgress) 256449196445 `
    'The committed full-read proxy changed.'

Assert-True (-not $evidence.sop2ObserverRisk.strictGateEvaluated -and
    -not $evidence.sop2ObserverRisk.strictGatePassed -and
    -not $evidence.sop2ObserverRisk.causalAttributionAvailable -and
    -not $summary.sop2ObserverRisk.strictGateEvaluated -and
    -not $summary.sop2ObserverRisk.strictGatePassed) `
    'SOP2 representative overhead was retroactively claimed as evaluated or passed.'

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

Write-Output 'SOP9c invalid-campaign verifier passed the V1 artifact pins, single forced arm, persistence-timeout cause, forced cleanup and preserved diagnostic state, zero-retry/no-result truth, SOP2 residual risk, SOP6/SOP7/SOP8 policies, and all production locks.'
