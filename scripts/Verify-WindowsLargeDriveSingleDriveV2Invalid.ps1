[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$campaignId = 'sop9c-single-drive-reference-repeat-v2'
$campaignRoot = Join-Path $repo "artifacts/windows-sop9-large-drive/$campaignId"
$journalPath = Join-Path $campaignRoot 'attempt.jsonl'
$manifestPath = Join-Path $campaignRoot 'manifest.json'
$workerBuildPath = Join-Path $campaignRoot 'worker-build.log'
$snapshotBuildPath = Join-Path $campaignRoot 'snapshot-build.log'
$acceptanceEvidencePath = Join-Path $campaignRoot 'acceptance-evidence.json'
$summaryPath = Join-Path $repo `
    'docs/evidence/scan-large-drive-single-drive-v2-invalid-20260828.json'
$runnerPath = Join-Path $PSScriptRoot 'Invoke-WindowsLargeDriveAcceptance.ps1'
$designVerifierPath = Join-Path $PSScriptRoot `
    'Verify-WindowsLargeDriveSingleDriveV2Protocol.ps1'
$protocolV1Path = Join-Path $repo 'docs/scan-large-drive-acceptance-protocol-v1.md'
$protocolV2Path = Join-Path $repo 'docs/scan-large-drive-acceptance-protocol-v2.md'
$planPath = Join-Path $repo 'docs/scan-optimization-plan.md'
$handoffPath = Join-Path $repo 'docs/windows-roadmap-session-handoff.md'
$roadmapPath = Join-Path $repo 'ROADMAP.md'
$schedulerPath = Join-Path $repo 'crates/super-duper-core/src/hasher/scheduler.rs'
$readPath = Join-Path $repo 'crates/super-duper-core/src/hasher/xxhash.rs'
$runModelsPath = Join-Path $repo 'crates/super-duper-core/src/storage/models.rs'
$operationViewModel = Join-Path $repo `
    'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$workerSource = Join-Path $repo 'crates/super-duper-worker/src/lib.rs'
$statePath = "H:\super-duper-sop9-state\$campaignId"

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
    Assert-Equal $actual $Expected "Retained evidence hash changed: $Path."
}

Push-Location $repo
try {
    Assert-Hash $journalPath `
        'BFB28B05B3175E4707AD0D552CD8A0EB5BAE10E9FE5676E907CCBF95F178408F'
    Assert-Hash $manifestPath `
        'B74224EABFC2BFCD0E4FB263D2C23EE5D3419D0A5318EA9376334B695D0236DB'
    Assert-Hash $snapshotBuildPath `
        '511E8F16AB5EECB99F83F50CE5BB85D4C443DDF719489CA0B5F775399C1811EB'
    Assert-Hash $workerBuildPath `
        'CEAB621D252FED0BC838651F3F1EB4EE080E3B3891A506A3BD3488AB66792807'

    Assert-True (-not (Test-Path -LiteralPath $acceptanceEvidencePath)) `
        'Native acceptance evidence was added after the interrupted host exit.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $campaignRoot 'worker-stderr.log'))) `
        'Worker stderr exists even though no worker-start event was retained.'

    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json -Depth 100
    Assert-Equal $manifest.campaignId $campaignId 'The fixed V2 identity changed.'
    Assert-True ($manifest.physical -and $manifest.rootIds.Count -eq 1 -and
        $manifest.rootIds[0] -eq 'E:') 'The fixed physical root changed.'
    Assert-True ($manifest.policies.Count -eq 2 -and
        $manifest.policies[0] -eq 'revalidate_content' -and
        $manifest.policies[1] -eq 'reuse_verified') 'The fixed V2 arm order changed.'
    Assert-Equal $manifest.expectedTerminal 'completed' 'The V2 terminal requirement changed.'
    Assert-True ($manifest.noFavorableRetry -and
        -not $manifest.sop2ObserverRisk.strictGateEvaluated -and
        -not $manifest.sop2ObserverRisk.strictGatePassed -and
        -not $manifest.sop2ObserverRisk.causalAttributionAvailable) `
        'Write-once or SOP2 residual-risk truth changed.'
    Assert-Equal $manifest.frameWatchdog.Mode 'persistence_activity_v2' `
        'The V2 watchdog mode changed.'
    Assert-Equal $manifest.frameWatchdog.FrameTimeoutSeconds 180 `
        'The non-persistence frame bound changed.'
    Assert-Equal $manifest.frameWatchdog.ProbeIntervalSeconds 5 `
        'The persistence probe interval changed.'
    Assert-Equal $manifest.frameWatchdog.PersistenceIdleTimeoutSeconds 900 `
        'The persistence idle bound changed.'
    Assert-Equal $manifest.frameWatchdog.PersistenceAbsoluteTimeoutSeconds 86400 `
        'The persistence absolute bound changed.'
    Assert-Equal $manifest.frameWatchdog.JournalIntervalSeconds 600 `
        'The persistence journal bound changed.'

    $journal = @(
        [IO.File]::ReadAllLines($journalPath) |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
            ForEach-Object { $_ | ConvertFrom-Json -Depth 100 }
    )
    $events = @($journal | ForEach-Object { [string]$_.event })
    $expectedEvents = @(
        'attempt_reserved',
        'physical_preflight_passed',
        'state_created',
        'build_ready'
    )
    Assert-Equal $events.Count $expectedEvents.Count 'The retained journal event count changed.'
    Assert-Equal ($events -join '|') ($expectedEvents -join '|') `
        'The retained pre-arm event order changed.'
    Assert-Equal (@($events | Where-Object { $_ -eq 'attempt_reserved' }).Count) 1 `
        'The write-once identity contains multiple reservations.'
    Assert-Equal (@($events | Where-Object {
        $_ -in @('worker_started', 'arm_started', 'run_terminal', 'evidence_finalized')
    }).Count) 0 'A worker, arm, terminal, or native finalization was invented.'

    $buildEvent = $journal | Where-Object { $_.event -eq 'build_ready' } | Select-Object -First 1
    Assert-Equal $buildEvent.data.workerSha256 `
        '47196cadb28cc4e772894d4bc014ea16a0d251733fac34a243a30ee78d7bacdc' `
        'The retained Release worker hash changed.'
    Assert-Equal $buildEvent.data.snapshotToolSha256 `
        '3e79d775dffa36a21b37796db4a681f485d4b53f72aee37a37a1dcfd58ee1f00' `
        'The retained snapshot-helper hash changed.'

    $summary = Get-Content -Raw -LiteralPath $summaryPath | ConvertFrom-Json -Depth 100
    Assert-Equal $summary.status 'blocked_invalid_campaign' `
        'The recovered V2 incident was reclassified.'
    Assert-True ($summary.writeOnceIdentityConsumed -and
        $summary.authority.approvedInvocations -eq 1 -and
        $summary.authority.invocationsConsumed -eq 1 -and
        -not $summary.authority.retryAuthorized) 'The consumed execution authority changed.'
    Assert-True (-not $summary.execution.workerStarted -and
        $summary.execution.armsStarted -eq 0 -and
        $summary.execution.terminalEvents -eq 0 -and
        -not $summary.execution.acceptanceEvidenceCreated -and
        $summary.execution.guardedSecondAdmissionObserved -and
        -not $summary.execution.causalDefectEstablished) `
        'The recovered pre-arm execution facts changed.'
    Assert-True (-not $summary.resultEvidence.scanStarted -and
        $summary.resultEvidence.completedArms -eq 0 -and
        -not $summary.resultEvidence.measurementAvailable -and
        -not $summary.resultEvidence.reuseArmStarted) `
        'The recovered incident invented scan or result evidence.'
    Assert-True ($summary.cleanupAudit.scopedStateAbsent -and
        $summary.cleanupAudit.scopedWorkerProcessCount -eq 0 -and
        $summary.cleanupAudit.scopedCampaignHostCount -eq 0 -and
        -not $summary.cleanupAudit.nativeCleanupRecordAvailable) `
        'The post-exit cleanup audit changed.'
    Assert-True (-not $summary.sop2ObserverRisk.strictGateEvaluated -and
        -not $summary.sop2ObserverRisk.strictGatePassed -and
        -not $summary.sop2ObserverRisk.causalAttributionAvailable) `
        'SOP2 representative overhead was retroactively evaluated or passed.'
    Assert-True (-not $summary.blocker.sop9cAccepted -and
        -not $summary.blocker.sop9dAuthorized -and
        -not $summary.blocker.successorProtocolAuthorized) `
        'The recovered incident invented follow-on authority.'

    Assert-Contains $protocolV1Path 'Neither V1 nor V2 may' `
        'The V1 protocol does not retain both consumed identities.'
    Assert-Contains $protocolV2Path 'V2 is invalid and cannot be rerun or overwritten.' `
        'The V2 protocol does not retain the invalid no-rerun disposition.'
    Assert-Contains $planPath '`SOP9c-single-drive-reference-repeat` | `blocked_invalid_campaign`' `
        'The scan plan does not block SOP9c on the invalid V2 outcome.'
    Assert-Contains $handoffPath 'sole separately authorized V2 invocation is now consumed' `
        'The handoff does not retain the consumed V2 authority.'
    Assert-Contains $handoffPath 'WPM8-high-contrast' `
        'The parked release-validation resume point changed.'
    Assert-Contains $roadmapPath 'Blocked after the consumed invalid SOP9c V2 attempt' `
        'ROADMAP does not retain the invalid V2 boundary.'

    Assert-Hash $runnerPath `
        '14263BBB1CF8B04834FA73CD3F0E2A5CC391C517A22EDFC09034CB6F5A60B176'
    Assert-Hash $designVerifierPath `
        '3168E837EF5C6033174EB766F8EED13DD1A8D6FDEBA22410EE7055480BB61D4D'
    Assert-True (-not (Test-Path -LiteralPath $statePath)) `
        'Scoped V2 state unexpectedly exists after the post-exit audit.'
    $workers = @(Get-Process -Name 'super-duper-worker' -ErrorAction SilentlyContinue)
    Assert-Equal $workers.Count 0 'A Super Duper worker is still running.'

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
}
finally {
    Pop-Location
}

Write-Output 'SOP9c V2 invalid-campaign verifier passed the consumed write-once reservation, fixed physical preflight, pinned builds, abrupt pre-worker host exit, guarded second admission, zero-arm/result/measurement truth, post-exit state/process absence, unevaluated SOP2 risk, SOP6/SOP7/SOP8 policies, and all production locks.'
