[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$schedulerSource = Join-Path $repo 'crates/super-duper-core/src/hasher/scheduler.rs'
$hasherSource = Join-Path $repo 'crates/super-duper-core/src/hasher/xxhash.rs'
$platformSource = Join-Path $repo 'crates/super-duper-core/src/platform/windows.rs'
$operationViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$workerSource = Join-Path $repo 'crates/super-duper-worker/src/lib.rs'
$ssdEvidencePath = Join-Path $repo 'docs/evidence/scan-device-scheduler-ssd-20260826.json'
$hddEvidencePath = Join-Path $repo 'docs/evidence/scan-device-scheduler-hdd-20260826.json'
$policyEvidencePath = Join-Path $repo 'docs/evidence/scan-device-scheduler-policy-20260826.json'

function Assert-True([bool]$Condition, [string]$Failure) {
    if (-not $Condition) { throw $Failure }
}

function Assert-Contains([string]$Path, [string]$Text, [string]$Failure) {
    if (-not [IO.File]::ReadAllText($Path).Contains($Text, [StringComparison]::Ordinal)) {
        throw $Failure
    }
}

function Invoke-Checked([scriptblock]$Command, [string]$Failure) {
    & $Command
    if ($LASTEXITCODE -ne 0) { throw $Failure }
}

function Assert-PowerShellParses([string]$Path) {
    $tokens = $null
    $errors = $null
    [void][Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$errors)
    if ($errors.Count -ne 0) { throw "PowerShell parsing failed for $Path`: $($errors -join '; ')" }
}

function Assert-ReaderEvidence(
    [object]$Evidence,
    [string]$Media,
    [int[]]$Order,
    [string]$FailurePrefix
) {
    Assert-True ($Evidence.schemaVersion -eq 1) "$FailurePrefix schema is not v1."
    Assert-True ($Evidence.gate -eq 'SOP6-device-aware-scheduler') "$FailurePrefix gate is wrong."
    Assert-True ($Evidence.mediaClass -eq $Media) "$FailurePrefix media class is wrong."
    Assert-True ($Evidence.directUnbufferedReads -eq $true) "$FailurePrefix did not use direct reads."
    Assert-True ($Evidence.hardwareSerialPersisted -eq $false) "$FailurePrefix persisted a serial."
    Assert-True ($Evidence.fixtureRemovedAfterProfile -eq $true) "$FailurePrefix fixture was not removed."
    Assert-True ($Evidence.bytesPerArm -eq 1073741824) "$FailurePrefix bytes per arm changed."
    Assert-True ($Evidence.samples.Count -eq 4) "$FailurePrefix must retain all four samples."
    Assert-True (@(Compare-Object @($Evidence.order) $Order).Count -eq 0) "$FailurePrefix order changed."
    $firstChecksums = ($Evidence.samples[0].checksums | ConvertTo-Json -Compress)
    foreach ($sample in $Evidence.samples) {
        Assert-True ($sample.physicalBytesRequested -eq 1073741824) `
            "$FailurePrefix did not request exactly 1 GiB."
        Assert-True ($sample.processReadBytes -eq 1073741824) `
            "$FailurePrefix process read bytes do not reconcile."
        Assert-True ($sample.deviceUnavailableCounterCount -eq 0) `
            "$FailurePrefix has unavailable device counters."
        Assert-True ((($sample.checksums | ConvertTo-Json -Compress) -eq $firstChecksums)) `
            "$FailurePrefix checksum vector changed between arms."
    }
}

Push-Location $repo
try {
    Assert-PowerShellParses $PSCommandPath
    Assert-Contains $schedulerSource 'pub(crate) const ROTATIONAL_READERS: usize = 1;' `
        'The rotational reader ceiling is no longer one.'
    Assert-Contains $schedulerSource 'pub(crate) const SOLID_STATE_READERS: usize = 4;' `
        'The selected SSD reader ceiling is no longer four.'
    Assert-Contains $schedulerSource 'pub(crate) const UNKNOWN_DEVICE_READERS: usize = 1;' `
        'The unknown-device reader ceiling is no longer one.'
    Assert-Contains $platformSource 'STORAGE_DEVICE_SEEK_PENALTY_PROPERTY' `
        'The Windows media classifier no longer uses the seek-penalty property.'
    Assert-Contains $platformSource 'IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS' `
        'The Windows scheduler no longer maps volumes to physical extents.'
    Assert-True (-not [IO.File]::ReadAllText($hasherSource).Contains('par_iter', [StringComparison]::Ordinal)) `
        'Nested Rayon reads returned to the hash pipeline.'
    Assert-Contains $operationViewModel 'public bool CanSubmit => false;' `
        'RecycleOperationViewModel.CanSubmit is no longer locked false.'
    Assert-Contains $compositionRoot `
        'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    if ([IO.File]::ReadAllText($workerSource).Contains('"executorEnabled": true', [StringComparison]::Ordinal)) {
        throw 'A worker response reports executorEnabled:true.'
    }

    $ssdEvidence = Get-Content -Raw $ssdEvidencePath | ConvertFrom-Json
    $hddEvidence = Get-Content -Raw $hddEvidencePath | ConvertFrom-Json
    $policyEvidence = Get-Content -Raw $policyEvidencePath | ConvertFrom-Json
    Assert-ReaderEvidence $ssdEvidence 'solid_state' @(1, 4, 4, 1) 'SSD evidence'
    Assert-ReaderEvidence $hddEvidence 'rotational' @(1, 2, 2, 1) 'HDD evidence'
    Assert-True ($policyEvidence.selectedPolicy.rotationalReaders -eq 1) `
        'The retained policy does not select one rotational reader.'
    Assert-True ($policyEvidence.selectedPolicy.solidStateReaders -eq 4) `
        'The retained policy does not select four SSD readers.'
    Assert-True ($policyEvidence.selectedPolicy.unknownOrAmbiguousReaders -eq 1) `
        'The retained policy does not fail unknown mapping back to one reader.'
    Assert-True ((Get-FileHash $schedulerSource -Algorithm SHA256).Hash -eq `
        $policyEvidence.softwareBuild.schedulerSourceSha256) `
        'The scheduler source no longer matches the measured software build.'
    Assert-True ($policyEvidence.softwareBuild.hashPipelineSourceSha256 -eq `
        '70A09333FF9551AB7DF8A25F092D55366DBF188686591BA22AC8B3AC58ACBC36') `
        'The retained SOP6 measured hash-pipeline identity changed.'
    Assert-Contains $hasherSource 'execute_device_reads(partial_tasks, cancel, policy' `
        'The current hash pipeline no longer schedules partial reads through SOP6.'
    Assert-Contains $hasherSource 'let scheduled_full_hashes = execute_device_reads(' `
        'The current hash pipeline no longer schedules full reads through SOP6.'
    Assert-Contains $hasherSource '        full_tasks,' `
        'The scheduled full-read call no longer consumes the full-task queue.'
    Assert-True ($policyEvidence.softwareBuild.windowsMappingSourceSha256 -eq `
        '9FAAB0217F216F6C5B1932935DD9D28167D776A2E905C99916909BE89B37D76B') `
        'The retained measured Windows mapping identity changed.'
    Assert-True ($policyEvidence.comparisons[0].candidateWallDeltaPercent -gt 0) `
        'The rotational evidence no longer shows the two-reader wall regression.'
    Assert-True ($policyEvidence.comparisons[1].candidateWallDeltaPercent -lt 0) `
        'The SSD evidence no longer shows the four-reader wall improvement.'
    Assert-True ((Get-FileHash $ssdEvidencePath -Algorithm SHA256).Hash -eq `
        'BE18D52569290337F5918E747DBBDCC5239AAEE69D22D378890632FFD80AD3B6') `
        'The retained SSD evidence changed.'
    Assert-True ((Get-FileHash $hddEvidencePath -Algorithm SHA256).Hash -eq `
        'B209FA1BA9C6CBFAC4B2C4D2CAE97685C461D7475AA173BD769F7176CCBF8C71') `
        'The retained HDD evidence changed.'

    Invoke-Checked { cargo test -p super-duper-core --lib hasher::scheduler::tests } `
        'Focused scheduler policy, concurrency, independence, and cancellation tests failed.'
    Invoke-Checked { cargo test -p super-duper-core --lib platform::windows::tests } `
        'Focused Windows physical-device/media mapping tests failed.'
    Invoke-Checked {
        cargo test -p super-duper-core --lib `
            hash_pipeline_serializes_each_rotational_device_but_overlaps_separate_devices
    } 'Focused scheduled hash-pipeline reconciliation test failed.'
    Invoke-Checked {
        cargo test -p super-duper-core --lib `
            singleton_buckets_never_open_content_and_other_buckets_keep_both_hash_paths
    } 'Accepted SOP5 zero-open regression failed.'
    Invoke-Checked { cargo build -p super-duper-worker } `
        'The paired worker executable did not build.'
    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'
}
finally {
    Pop-Location
}

Write-Output 'SOP6 verifier passed physical-device/media mapping, one/four/unknown policy bounds, cross-device independence, cancellation, exact hash/counter reconciliation, retained SSD/HDD evidence, and production locks.'
