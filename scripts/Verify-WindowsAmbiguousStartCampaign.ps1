[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$EvidenceDirectory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$evidenceRoot = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$repoBoundary = $repo.TrimEnd('\') + '\'
if (-not $evidenceRoot.StartsWith($repoBoundary, [StringComparison]::OrdinalIgnoreCase)) {
    throw "EvidenceDirectory must stay inside the repository: $evidenceRoot"
}
$run = Get-Content -Raw -LiteralPath (Join-Path $evidenceRoot 'campaign-run.json') | ConvertFrom-Json
$fixture = Get-Content -Raw -LiteralPath (Join-Path $evidenceRoot 'fixture-description.json') | ConvertFrom-Json
$statePath = Join-Path $evidenceRoot 'campaign-state.json'
$state = Get-Content -Raw -LiteralPath $statePath | ConvertFrom-Json
$verificationAttempt = (Get-Date).ToUniversalTime().ToString('yyyyMMdd-HHmmss-fff')
$failurePath = Join-Path $evidenceRoot "failure-verify-$verificationAttempt.json"
$database = [string]$fixture.databasePath
$worker = [string]$fixture.workerPath
$hashCache = [string]$fixture.hashCachePath
$hostExecutable = [string]$run.hostExecutable
$defaultWorkerLog = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)) 'SuperDuper/logs/worker.log'

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

Push-Location $repo
try {
    if (Get-Process -Id $state.appProcessId -ErrorAction SilentlyContinue) {
        throw "The WPF app process $($state.appProcessId) is still running; close it normally before verification."
    }
    if (Test-Path -LiteralPath $defaultWorkerLog -PathType Leaf) {
        Copy-Item -LiteralPath $defaultWorkerLog -Destination (Join-Path $evidenceRoot 'worker-after-wpf-review.log')
    }

    & $hostExecutable --mode verify --worker $worker --database $database --hash-cache $hashCache `
        --operation-id $fixture.recycleOperationId --output (Join-Path $evidenceRoot 'reviewed-protocol.json') 2>&1 |
        Tee-Object -LiteralPath (Join-Path $evidenceRoot 'reviewed-protocol-command.log') | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Post-review protocol reconstruction failed.' }
    & cargo run --quiet -p super-duper-core --example windows_ambiguous_start_evidence -- `
        $database $fixture.recycleOperationId (Join-Path $evidenceRoot 'reviewed-source.json') 2>&1 |
        Tee-Object -LiteralPath (Join-Path $evidenceRoot 'reviewed-source-command.log') | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Post-review source snapshot failed.' }

    $recovered = Get-Content -Raw -LiteralPath (Join-Path $evidenceRoot 'recovered-source.json') | ConvertFrom-Json
    $reviewed = Get-Content -Raw -LiteralPath (Join-Path $evidenceRoot 'reviewed-source.json') | ConvertFrom-Json
    $recoveredProtocol = Get-Content -Raw -LiteralPath (Join-Path $evidenceRoot 'recovered-protocol.json') | ConvertFrom-Json
    $reviewedProtocol = Get-Content -Raw -LiteralPath (Join-Path $evidenceRoot 'reviewed-protocol.json') | ConvertFrom-Json

    Assert-True ($recovered.databaseSchemaVersion -eq 11 -and $reviewed.databaseSchemaVersion -eq 11) `
        'The preserved database is not schema v11.'
    Assert-True ($recovered.sourceEvidence.operation.Count -eq 1 -and $recovered.sourceEvidence.operation[0].status -eq 'recovery_required') `
        'Restart did not reconstruct exactly one recovery_required operation.'
    Assert-True ($recovered.sourceEvidence.batches.Count -eq 1 -and $recovered.sourceEvidence.batches[0].status -eq 'ambiguous') `
        'Restart did not reconstruct exactly one ambiguous batch.'
    Assert-True ($recovered.sourceEvidence.items.Count -eq 2 -and @($recovered.sourceEvidence.items | Where-Object result_status -ne 'unknown').Count -eq 0) `
        'Restart did not reconstruct every pending item as immutable unknown.'
    Assert-True ($recovered.sourceEvidence.recovery.Count -eq 2 -and @($recovered.sourceEvidence.recovery | Where-Object reason_code -ne 'worker_interrupted_after_shell_start').Count -eq 0) `
        'Restart did not create one stable recovery row for every unknown.'
    Assert-True (@($recovered.sourceEvidence.reports | Where-Object report_kind -eq 'result').Count -eq 0) `
        'An operation result report was unexpectedly submitted.'
    Assert-True ($recovered.derivedReview.state -eq 'not_started' -and $recovered.derivedReview.observedItemCount -eq 0) `
        'Recovery review was not initially reconstructed as not_started.'

    $beforeSource = $recovered.sourceEvidence | ConvertTo-Json -Depth 100 -Compress
    $afterSource = $reviewed.sourceEvidence | ConvertTo-Json -Depth 100 -Compress
    Assert-True ($beforeSource -ceq $afterSource) `
        'Original operation, batch, item, recovery, or report rows changed during review.'
    foreach ($property in @('operations', 'batches', 'items', 'recoveryRows', 'reports')) {
        Assert-True ($recovered.globalCounts.$property -eq $reviewed.globalCounts.$property) `
            "Global $property count changed during review."
    }
    Assert-True ($reviewed.derivedReview.state -eq 'review_complete_with_unresolved_evidence' -and
        $reviewed.derivedReview.unknownItemCount -eq 2 -and $reviewed.derivedReview.observedItemCount -eq 2) `
        'The accepted checklist did not account for every unknown item.'
    Assert-True ($reviewed.observations.Count -eq 3) 'Expected two current observations plus one superseded prior observation.'
    $corrections = @($reviewed.observations | Where-Object { $null -ne $_.supersedes_observation_id })
    Assert-True ($corrections.Count -eq 1 -and -not [string]::IsNullOrWhiteSpace($corrections[0].correction_reason)) `
        'Explicit supersession with a correction reason was not retained.'
    $supersededIds = @($corrections | ForEach-Object supersedes_observation_id)
    Assert-True (@($reviewed.observations | Where-Object { $supersededIds -contains $_.id }).Count -eq 1) `
        'The prior superseded observation was not retained.'

    Assert-True ($recoveredProtocol.operation.status -eq 'recovery_required' -and
        $recoveredProtocol.unknownItems.total -eq 2 -and
        $recoveredProtocol.review.review.state -eq 'not_started') `
        'Initial worker protocol reconstruction did not match the durable source snapshot.'
    Assert-True ($reviewedProtocol.operation.status -eq 'recovery_required' -and
        $reviewedProtocol.unknownItems.total -eq 2 -and
        $reviewedProtocol.review.review.state -eq 'review_complete_with_unresolved_evidence' -and
        $reviewedProtocol.history.total -eq 3 -and $reviewedProtocol.current.total -eq 2) `
        'Post-review worker protocol reconstruction did not retain the complete append-only checklist.'
    Assert-True (-not $reviewedProtocol.executorEnabled.review -and
        -not $reviewedProtocol.executorEnabled.history -and -not $reviewedProtocol.executorEnabled.current) `
        'A recovery-review worker response reported executorEnabled:true.'

    $operationViewModel = Get-Content -Raw -LiteralPath (Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs')
    $compositionRoot = Get-Content -Raw -LiteralPath (Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs')
    $preflightView = Get-Content -Raw -LiteralPath (Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/PreflightView.xaml')
    $campaignHost = Get-Content -Raw -LiteralPath (Join-Path $repo 'apps/windows/tools/SuperDuper.Windows.AmbiguousStartHost/Program.cs')
    Assert-True $operationViewModel.Contains('public bool CanSubmit => false;', [StringComparison]::Ordinal) `
        'RecycleOperationViewModel.CanSubmit is no longer locked false.'
    Assert-True $compositionRoot.Contains('services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();', [StringComparison]::Ordinal) `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    Assert-True (-not $preflightView.Contains('Move to Recycle Bin now', [StringComparison]::OrdinalIgnoreCase)) `
        'The forbidden Move to Recycle Bin now action is present.'
    Assert-True $campaignHost.Contains('await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken);', [StringComparison]::Ordinal) `
        'The disposable host no longer blocks before PerformOperations.'

    $notes = @(
        '# WPM11 ambiguous-start operator observations',
        '',
        '- Gate: WPM11-ambiguous-start',
        "- Operation: $($fixture.recycleOperationId)",
        "- Unknown items: $($reviewed.derivedReview.unknownItemCount)",
        "- Current observations: $($reviewed.derivedReview.observedItemCount)",
        '- The operator used the WPF checklist and manually opened the preserved fixture source folder and Windows Recycle Bin.',
        '- The app performed no automatic filesystem/provider/content/Recycle Bin inspection or inference.',
        '- One initial deferred observation was explicitly corrected after manual inspection; the prior record and correction reason remain append-only.',
        '- Every current observation records only the operator-selected Option A classification; original Shell evidence remains unresolved.',
        '',
        '## Retained observation rows',
        ''
    )
    foreach ($observation in $reviewedProtocol.history.observations) {
        $notes += "- observation $($observation.id), item $($observation.itemId): $($observation.observation); current=$($observation.isCurrent); note=$($observation.note); supersedes=$($observation.supersedesObservationId); correction=$($observation.correctionReason)"
    }
    $notes | Set-Content -LiteralPath (Join-Path $evidenceRoot 'operator-observations.md') -Encoding utf8

    [ordered]@{
        schemaVersion = 1
        gate = 'WPM11-ambiguous-start'
        result = 'passed'
        verifiedAtUtc = (Get-Date).ToUniversalTime().ToString('O')
        databaseSchemaVersion = 11
        operationStatus = 'recovery_required'
        batchStatus = 'ambiguous'
        unknownItemCount = 2
        observationHistoryCount = 3
        currentObservationCount = 2
        sourceEvidenceUnchanged = $true
        operationWorkRetried = $false
        operationWorkReplayed = $false
        operationWorkResubmitted = $false
        outcomeInferred = $false
        itemRestored = $false
        itemDeletedByCampaign = $false
        itemCopiedForward = $false
        productionEnabled = $false
        milestone11Complete = $false
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $evidenceRoot 'campaign-verification.json') -Encoding utf8
    [ordered]@{
        schemaVersion = 1
        state = 'verified'
        updatedAtUtc = (Get-Date).ToUniversalTime().ToString('O')
        result = 'passed'
    } | ConvertTo-Json | Set-Content -LiteralPath $statePath -Encoding utf8
    Write-Output "WPM11-ambiguous-start exact verifier passed: $evidenceRoot"
}
catch {
    [ordered]@{
        schemaVersion = 1
        gate = 'WPM11-ambiguous-start'
        stage = 'verify'
        capturedAtUtc = (Get-Date).ToUniversalTime().ToString('O')
        message = $_.Exception.Message
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $failurePath -Encoding utf8
    throw
}
finally {
    Pop-Location
}
