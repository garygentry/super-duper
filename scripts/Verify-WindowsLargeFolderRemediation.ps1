[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$invalidJournalPath = Join-Path $repo 'docs/evidence/scan-folder-scale-sop10f-20260905.attempt.jsonl'
$invalidSummaryPath = Join-Path $repo 'docs/evidence/scan-folder-scale-sop10f-v1-invalid-20260905.json'
$scaleJournalPath = Join-Path $repo 'docs/evidence/scan-folder-scale-sop10f-v2-20260905.attempt.jsonl'
$scaleEvidencePath = Join-Path $repo 'docs/evidence/scan-folder-scale-sop10f-v2-20260905.json'
$exactFolderSource = Join-Path $repo 'crates/super-duper-core/src/analysis/exact_folders.rs'
$engineSource = Join-Path $repo 'crates/super-duper-core/src/engine.rs'
$repeatVerifier = Join-Path $PSScriptRoot 'Verify-WindowsRepeatCache.ps1'
$releaseVerifier = Join-Path $PSScriptRoot 'Verify-WindowsRelease.ps1'
$publish = Join-Path $repo 'artifacts/windows-x64'

function Assert-True([bool]$Condition, [string]$Failure) {
    if (-not $Condition) { throw $Failure }
}

function Assert-Contains([string]$Path, [string]$Text, [string]$Failure) {
    if (-not [IO.File]::ReadAllText($Path).Contains($Text, [StringComparison]::Ordinal)) {
        throw $Failure
    }
}

function Assert-Hash([string]$Path, [string]$Expected) {
    Assert-True (Test-Path -LiteralPath $Path -PathType Leaf) "Required evidence is missing: $Path"
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash
    Assert-True ($actual -eq $Expected) "Retained evidence changed: $Path ($actual)"
}

function Assert-NoSuperDuperProcess {
    $active = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
        $_.ProcessName -in @('SuperDuper.Windows', 'super-duper-worker')
    })
    if ($active.Count -ne 0) {
        $names = @($active | ForEach-Object { $_.ProcessName }) -join ', '
        throw "A Super Duper app/worker process is active: $names"
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

Push-Location $repo
try {
    Assert-PowerShellParses $PSCommandPath
    Assert-NoSuperDuperProcess

    Assert-Hash $invalidJournalPath '1EFD472C94F0E2F4CCE575EB698AA2D2101AF0001945C54245974E87B786A183'
    Assert-Hash $invalidSummaryPath '74497F293EA34F112A8FAAFF45C8C86CD1518B7364C62656FE17CA4785E37298'
    Assert-Hash $scaleJournalPath '1DC19ED2829361785E0A8EAC6FD2E8F2BFE0F923DCE1EBABF42BFAD9837AA002'
    Assert-Hash $scaleEvidencePath '4FF6036FFA932AE7F9913E806D5CDF31129401A39617DF9BA6544CB99F371833'

    $invalid = Get-Content -Raw -LiteralPath $invalidSummaryPath | ConvertFrom-Json
    Assert-True ($invalid.status -eq 'invalid_harness_failure') 'SOP10f v1 is no longer retained invalid.'
    Assert-True ($invalid.firstResultRetained -eq $true -and $invalid.rerunSameIdentityAllowed -eq $false) `
        'SOP10f v1 retention or same-identity refusal changed.'
    Assert-True ($invalid.retainedFacts.analysisResultAvailable -eq $false) `
        'SOP10f v1 now claims unavailable product evidence.'
    Assert-True ($invalid.retainedFacts.resourceThresholdEvaluated -eq $false) `
        'SOP10f v1 now claims an unavailable resource evaluation.'

    $scale = Get-Content -Raw -LiteralPath $scaleEvidencePath | ConvertFrom-Json
    Assert-True ($scale.status -eq 'valid') 'The retained SOP10f v2 scale evidence is not valid.'
    Assert-True ($scale.gate -eq 'SOP10f-release-scale-acceptance') 'The SOP10f evidence gate changed.'
    Assert-True ($scale.buildCommit -eq 'de48b3c2c79d4f9bfd65a256389ad02ff56a3c1f') `
        'The retained SOP10f v2 build identity changed.'
    Assert-True ($scale.firstResultRetained -eq $true -and $scale.favorableRerunAllowed -eq $false) `
        'The SOP10f first-result or favorable-rerun rule changed.'
    Assert-True ($scale.configuration -eq 'Release') 'The SOP10f profile was not a Release profile.'
    Assert-True ($scale.input.scannedFileCount -eq 3550000) 'The SOP10f file count changed.'
    Assert-True ($scale.input.expectedDirectoryCount -eq 633000) 'The SOP10f directory count changed.'
    Assert-True ($scale.analysis.directoryCount -eq 633000) 'The reconstructed directory count changed.'
    Assert-True ($scale.analysis.elapsedMilliseconds -lt 1800000) `
        'Folder analysis exceeded the fixed thirty-minute ceiling.'
    Assert-True ($scale.analysis.peakPrivateBytes -lt 2147483648) `
        'Folder analysis exceeded the fixed two-GiB private-memory ceiling.'
    Assert-True ($scale.analysis.scannedFilePasses -le 3) 'Folder analysis exceeded three ordered file passes.'
    Assert-True ($scale.analysis.largestPersistenceBatch -le 1024) 'Folder persistence exceeded 1,024 rows.'
    Assert-True ($scale.analysis.similarityPairs -eq 0) 'Automatic Jaccard similarity work returned.'
    Assert-True ($scale.analysis.retainedGroups -eq 9 -and $scale.analysis.visibleGroups -eq 1) `
        'Exact retained/visible folder results changed.'
    Assert-True ($scale.analysis.warningCount -eq 1 -and $null -eq $scale.analysis.error) `
        'Warning truth or successful completion changed.'
    foreach ($check in $scale.checks.PSObject.Properties) {
        Assert-True ($check.Value -eq $true) "SOP10f check failed: $($check.Name)"
    }
    Assert-True ($scale.cacheAcceptance.normalLiveTargetEntries -eq 5000000) `
        'The qualified-cache normal live target changed.'
    Assert-True ($scale.cacheAcceptance.postPruneTargetEntries -eq 4500000) `
        'The qualified-cache post-prune target changed.'
    Assert-True ($scale.cacheAcceptance.activeHardHighWaterEntries -eq 10000000) `
        'The qualified-cache active hard ceiling changed.'

    Assert-Contains $exactFolderSource 'db.visit_scanned_files_ordered(run_id' `
        'Exact-folder analysis no longer uses its single ordered file visitor.'
    Assert-Contains $exactFolderSource 'state rather than cloned file records or descendant hash sets.' `
        'The no-ancestor-clone/no-descendant-set invariant is missing.'
    Assert-Contains $exactFolderSource 'const PERSIST_BATCH_SIZE: usize = 1_024;' `
        'The exact-folder persistence bound changed.'
    Assert-Contains $engineSource 'exact_folders::analyze_exact_folders_cancellable(' `
        'The production engine no longer invokes streaming exact-folder analysis.'

    & $repeatVerifier
    if ($LASTEXITCODE -ne 0) { throw 'The accepted repeat-cache/full-matrix verifier failed.' }

    Assert-NoSuperDuperProcess
    Invoke-Checked {
        cargo test --release -p super-duper-core --lib `
            hasher::repeat_cache::tests::generated_store_above_legacy_cap_retains_early_and_late_hits_after_reopen `
            -- --ignored --exact
    } 'The above-legacy-cap qualified-cache Release fixture failed.'

    Assert-NoSuperDuperProcess
    & $releaseVerifier -SkipSmoke
    if ($LASTEXITCODE -ne 0) { throw 'The fixed Release publish verification failed.' }

    foreach ($required in @('SuperDuper.Windows.exe', 'super-duper-worker.exe', 'SuperDuper.Windows.dll')) {
        $path = Join-Path $publish $required
        Assert-True (Test-Path -LiteralPath $path -PathType Leaf) "Fixed Release output is missing $required."
        Assert-True ((Get-Item -LiteralPath $path).Length -gt 0) "Fixed Release output is empty: $required."
    }
    Assert-NoSuperDuperProcess
}
finally {
    Pop-Location
}

Write-Output 'SOP10 verifier passed immutable v1/v2 evidence, exact streaming/resource/result bounds, qualified-cache scale and policy, full Debug/Release Rust and Windows matrices, production locks, and fixed Release package artifacts without launching a physical scan.'
