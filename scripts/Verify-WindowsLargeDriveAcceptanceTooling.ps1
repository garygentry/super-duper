[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$runner = Join-Path $PSScriptRoot 'Invoke-WindowsLargeDriveAcceptance.ps1'
$protocol = Join-Path $repo 'docs/scan-large-drive-acceptance-protocol-v1.md'
$plan = Join-Path $repo 'docs/scan-optimization-plan.md'
$operationViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$workerSource = Join-Path $repo 'crates/super-duper-worker/src/lib.rs'
$toolingRoot = Join-Path ([IO.Path]::GetTempPath()) ('super-duper-sop9-tooling-verifier-' + [guid]::NewGuid().ToString('N'))
$failureRoot = "$toolingRoot-failure"

function Assert-True([bool]$Condition, [string]$Failure) {
    if (-not $Condition) { throw $Failure }
}

function Assert-Contains([string]$Path, [string]$Text, [string]$Failure) {
    if (-not [IO.File]::ReadAllText($Path).Contains($Text, [StringComparison]::Ordinal)) {
        throw $Failure
    }
}

function Assert-PowerShellParses([string]$Path) {
    $tokens = $null
    $errors = $null
    [void][Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$errors)
    if ($errors.Count -ne 0) { throw "PowerShell parsing failed for $Path`: $($errors -join '; ')" }
}

function Invoke-Checked([scriptblock]$Command, [string]$Failure) {
    & $Command
    if ($LASTEXITCODE -ne 0) { throw $Failure }
}

$retainedHashes = [ordered]@{
    'docs/evidence/scan-progress-representative-premeasurement-20260825.json' = 'D2E1757F870D8B1A805741956B0C8375EFE6877C8905F7B72295A01250AB9899'
    'docs/evidence/scan-progress-representative-overhead-sop2f-v2.json' = 'B779E6626D66763F6F6E1608D34EB78CD90C526597691EADFFE2F5A61E1C4077'
    'docs/evidence/scan-device-scheduler-policy-20260826.json' = '7A30A82F47079A1ABC6E1A84B389E2972CEA2A7E936285E467695CD1BAC385FF'
    'docs/evidence/scan-read-path-locality-policy-20260826.json' = 'C6FB5811A23C2A37497B5D8E477947F82B539C89750193FE1CC5EDCAD6BA8A9B'
    'docs/evidence/scan-read-path-bucket-order-policy-20260826.json' = '6EDD37C22638F3642ED318666DB968C6EED70CC610D18A171EC331273F6C8DB9'
    'docs/evidence/scan-read-path-buffer-read-ahead-policy-v2-20260827.json' = 'BE4035E0259332E3714F32E487B31D7048F2881FFD9A5850D37556DEB8E40D18'
    'docs/evidence/scan-read-path-prefix-reuse-policy-20260827.json' = '289CA9B5CA51E9148AB1686AAAC26DDE2E96223C7E5DF66D09305E818EE0AB9B'
    'docs/evidence/scan-repeat-cache-policy-20260827.json' = '7DFC8AA6B44DD00F9E307197B4CA82A4B6B2E28888C3F236B9BD8281A9569DC8'
}

Push-Location $repo
try {
    Assert-PowerShellParses $runner
    Assert-PowerShellParses $PSCommandPath
    Assert-Contains $protocol 'strictGateEvaluated' 'The SOP2 residual-risk contract is missing.'
    Assert-Contains $protocol 'No attempt, arm, tail, unavailable value, cancellation, or cleanup failure may' `
        'The no-favorable-retry contract changed.'
    Assert-Contains $plan '`SOP9a-write-once-campaign-tooling` | `ready`' `
        'SOP9a is not the authorized ready package.'
    Assert-Contains $operationViewModel 'public bool CanSubmit => false;' `
        'RecycleOperationViewModel.CanSubmit is no longer locked false.'
    Assert-Contains $compositionRoot `
        'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    Assert-True (-not [IO.File]::ReadAllText($workerSource).Contains('"executorEnabled": true', [StringComparison]::Ordinal)) `
        'A worker response reports executorEnabled:true.'

    foreach ($entry in $retainedHashes.GetEnumerator()) {
        $path = Join-Path $repo $entry.Key
        Assert-True (Test-Path -LiteralPath $path -PathType Leaf) "Retained evidence is missing: $($entry.Key)"
        Assert-True ((Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash -eq $entry.Value) `
            "Retained evidence changed: $($entry.Key)"
    }

    foreach ($campaign in @(
        'sop9b-representative-cancellation-v1',
        'sop9c-single-drive-reference-repeat-v1',
        'sop9d-multi-drive-reference-repeat-v1')) {
        $preflight = & $runner -Campaign $campaign -PreflightOnly | ConvertFrom-Json
        Assert-True ($preflight.campaignId -eq $campaign) "Preflight returned the wrong campaign: $campaign"
        Assert-True ($preflight.evidencePathAbsent -and $preflight.statePathAbsent) `
            "Physical campaign identity is already consumed: $campaign"
    }

    Invoke-Checked { & $runner -Campaign tooling_fixture -ToolingRoot $toolingRoot } `
        'The deterministic SOP9 tooling fixture failed.'
    $evidencePath = Join-Path $toolingRoot 'evidence/acceptance-evidence.json'
    $manifestPath = Join-Path $toolingRoot 'evidence/manifest.json'
    $journalPath = Join-Path $toolingRoot 'evidence/attempt.jsonl'
    Assert-True (Test-Path -LiteralPath $evidencePath -PathType Leaf) 'Tooling evidence was not retained.'
    Assert-True (Test-Path -LiteralPath $manifestPath -PathType Leaf) 'Tooling manifest was not retained.'
    Assert-True (Test-Path -LiteralPath $journalPath -PathType Leaf) 'Tooling journal was not retained.'
    $evidence = Get-Content -Raw -LiteralPath $evidencePath | ConvertFrom-Json -Depth 100
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json -Depth 100
    Assert-True ($evidence.status -eq 'valid' -and $evidence.arms.Count -eq 2) `
        'Tooling fixture did not retain two valid arms.'
    Assert-True ($manifest.noFavorableRetry -and -not $manifest.sop2ObserverRisk.strictGateEvaluated -and
        -not $manifest.sop2ObserverRisk.strictGatePassed) `
        'Tooling manifest altered write-once or SOP2 residual-risk truth.'
    Assert-True ($evidence.comparisons.fileResultsEqual -and $evidence.comparisons.folderResultsEqual) `
        'Forced/reuse tooling result digests differ.'
    Assert-True ($evidence.cleanup.workerStopped -and $evidence.cleanup.stateRemoved -and
        $evidence.cleanup.errors.Count -eq 0) 'Tooling cleanup did not complete exactly.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $toolingRoot 'state'))) `
        'Tooling state survived validated cleanup.'
    Assert-True ($evidence.arms[0].snapshot.product.warningAccountingComplete -and
        $evidence.arms[1].snapshot.product.warningAccountingComplete) `
        'Tooling warning accounting is incomplete.'
    $forced = $evidence.arms[0].snapshot.status.counters
    $reuse = $evidence.arms[1].snapshot.status.counters
    Assert-True ([UInt64]$forced.singleton_size_files -eq [UInt64]$forced.metadata_resolved_files -and
        [UInt64]$forced.singleton_size_files -gt 0) 'Singleton metadata resolution did not reconcile.'
    Assert-True ([UInt64]$forced.hard_link_alias_files -eq 1) 'Hard-link physical de-duplication changed.'
    Assert-True ([UInt64]$reuse.partial_hash_cache_hits -gt 0 -and [UInt64]$reuse.full_hash_cache_hits -gt 0) `
        'Verified reuse did not exercise both cache stages.'
    Assert-True ($evidence.arms[0].maximumFramesPerObservedSecond -le 10 -and
        $evidence.arms[1].maximumFramesPerObservedSecond -le 10) `
        'Worker progress exceeded ten observed frames per second.'
    Assert-True ($evidence.arms[0].snapshot.status.lastSequence -gt 0 -and
        $evidence.arms[0].snapshot.status.flushCount -eq 0 -and
        $evidence.arms[0].snapshot.status.flushPayloadBytes -eq 0 -and
        $evidence.arms[0].statusDatabaseBytes -gt 0 -and
        $evidence.arms[0].processDelta.writeBytes -gt 0) `
        'The tooling evidence did not retain commit/write volume and terminal replay-row removal.'
    $serializedEvidence = Get-Content -Raw -LiteralPath $evidencePath
    Assert-True (-not $serializedEvidence.Contains($toolingRoot, [StringComparison]::OrdinalIgnoreCase)) `
        'The query-only snapshot leaked a raw fixture root.'

    $injectedFailed = $false
    try {
        & $runner -Campaign tooling_fixture -ToolingRoot $failureRoot -SkipBuild `
            -InjectToolingFailureAfterStateReservation
    }
    catch { $injectedFailed = $true }
    Assert-True $injectedFailed 'The tooling-only injected failure unexpectedly passed.'
    $invalidEvidencePath = Join-Path $failureRoot 'evidence/acceptance-evidence.json'
    $invalidJournalPath = Join-Path $failureRoot 'evidence/attempt.jsonl'
    Assert-True (Test-Path -LiteralPath $invalidEvidencePath -PathType Leaf) `
        'The injected failure did not retain invalid evidence.'
    Assert-True (Test-Path -LiteralPath $invalidJournalPath -PathType Leaf) `
        'The injected failure did not retain its append-only journal.'
    $invalidEvidence = Get-Content -Raw -LiteralPath $invalidEvidencePath | ConvertFrom-Json -Depth 100
    Assert-True ($invalidEvidence.status -eq 'invalid' -and
        $invalidEvidence.cleanup.statePreservedForDiagnostics -and
        -not $invalidEvidence.cleanup.stateRemovalAttempted -and
        (Test-Path -LiteralPath (Join-Path $failureRoot 'state'))) `
        'The injected failure did not preserve diagnostic state under the invalid outcome.'
    Assert-True ((Get-Content -Raw -LiteralPath $invalidJournalPath).Contains(
        'attempt_failed', [StringComparison]::Ordinal)) `
        'The injected failure was not appended to the journal.'

    Invoke-Checked { cargo test --release -p super-duper-core --test storage_tests `
        repeat_cache_policy_is_immutable_and_legacy_snapshots_reconstruct_as_forced -- --exact } `
        'Immutable/legacy run policy reconstruction failed.'
    Invoke-Checked { cargo test --release -p super-duper-core --lib `
        hasher::xxhash::tests::singleton_buckets_never_open_content_and_other_buckets_keep_both_hash_paths -- --exact } `
        'Singleton zero-open correctness failed.'
    Invoke-Checked { cargo test --release -p super-duper-worker `
        tests::run_start_defaults_reuse_rejects_unknown_policy_and_reconstructs_exact_alternate -- --exact } `
        'Worker closed repeat-policy validation failed.'
    Invoke-Checked { cargo check -p super-duper-core --example sop9_evidence_snapshot } `
        'The query-only SOP9 snapshot helper failed to compile.'
    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'
}
finally {
    Pop-Location
    foreach ($cleanupRoot in @($toolingRoot, $failureRoot)) {
      if (Test-Path -LiteralPath $cleanupRoot) {
        $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
        $resolved = [IO.Path]::GetFullPath($cleanupRoot).TrimEnd('\')
        if (-not $resolved.StartsWith($tempRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or
            $resolved -eq $tempRoot) {
            throw "Refusing unsafe tooling cleanup: $resolved"
        }
        $item = Get-Item -LiteralPath $resolved -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Refusing reparse-point tooling cleanup: $resolved"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
      }
    }
}

Write-Output 'SOP9a tooling verifier passed write-once/failure retention, physical preflight, deterministic forced/reuse correctness, singleton/hard-link/cache/warning/status/progress bounds, historical evidence hashes, legacy policy reconstruction, exact cleanup, and all production locks without a representative run.'
