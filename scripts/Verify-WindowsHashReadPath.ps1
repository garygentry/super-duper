[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$hasherSource = Join-Path $repo 'crates/super-duper-core/src/hasher/xxhash.rs'
$cacheSource = Join-Path $repo 'crates/super-duper-core/src/hasher/cache.rs'
$evidenceRoot = Join-Path $repo 'docs/evidence'
$sop6Verifier = Join-Path $PSScriptRoot 'Verify-WindowsDeviceScheduler.ps1'

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

function Read-Evidence([string]$Name) {
    Get-Content -Raw -LiteralPath (Join-Path $evidenceRoot $Name) | ConvertFrom-Json
}

function Assert-RawEvidence([string]$Name) {
    $evidence = Read-Evidence $Name
    Assert-True ($evidence.gate -eq 'SOP7-hash-read-path') "$Name has the wrong gate."
    Assert-True ($evidence.hardwareSerialPersisted -eq $false) "$Name persisted a hardware serial."
    Assert-True ($evidence.fixtureRemovedAfterProfile -eq $true) "$Name did not remove its fixture."
    Assert-True ($evidence.samples.Count -eq 4) "$Name does not retain four samples."
    Assert-True (@(Compare-Object @($evidence.armOrder) @('control', 'treatment', 'treatment', 'control')).Count -eq 0) `
        "$Name changed the fixed arm order."
    $firstChecksums = $evidence.samples[0].checksums | ConvertTo-Json -Compress
    foreach ($sample in $evidence.samples) {
        Assert-True ($sample.cancelled -eq $false) "$Name contains a cancelled retained arm."
        Assert-True ($sample.wallNanos -gt 0) "$Name contains a zero-duration arm."
        Assert-True ($sample.physicalBytesRead -gt 0) "$Name contains a zero-byte arm."
        Assert-True (($sample.checksums | ConvertTo-Json -Compress) -eq $firstChecksums) `
            "$Name checksum vectors differ between arms."
    }
}

$expectedHashes = [ordered]@{
    'scan-read-path-bucket-order-hdd-20260826.json' = 'ABEFD2AFD95AD1E1A3EEE6E6909AA4BD46E813280955AF9007B6F3C4A3851408'
    'scan-read-path-bucket-order-policy-20260826.json' = '6EDD37C22638F3642ED318666DB968C6EED70CC610D18A171EC331273F6C8DB9'
    'scan-read-path-bucket-order-ssd-20260826.json' = '338C80512E70FC4A3173368FE7BF6F6D5175A35901E6B07459BA6E2ADBB19D51'
    'scan-read-path-buffer-read-ahead-policy-20260826.json' = 'F3FE5812D81E00D102EC57799AC848CC7C7CB756211872873A09937AD14D7EBD'
    'scan-read-path-buffer-read-ahead-policy-v2-20260827.json' = 'BE4035E0259332E3714F32E487B31D7048F2881FFD9A5850D37556DEB8E40D18'
    'scan-read-path-buffer-size-hdd-20260826.json' = 'FB48882773606B5E7BE88B6C52D7E8A63B3556403E2CE54C6C697F5BE14AA872'
    'scan-read-path-buffer-size-ssd-20260826.json' = '7C2D48F8CF8D1E5FDFAAC0EE64417E0914FA06A54AFE67FC8081CD8516ED309C'
    'scan-read-path-buffer-size-v2-hdd-20260826.json' = 'C7895B008ACE7ECE40EE26AAEA017872C9E3330B7F185422F79E00B1394705A1'
    'scan-read-path-buffer-size-v2-ssd-20260826.json' = 'C60AF73E97DFEAC5D08534A311C32F74333F66A1A18BDC589A4ACBDA7D48AB0E'
    'scan-read-path-locality-hdd-20260826.json' = '4EDE50A2569F9349A02E3A756C51841E0F3E1C96ABE4FF0E2BCA96DF550383B7'
    'scan-read-path-locality-policy-20260826.json' = 'C6FB5811A23C2A37497B5D8E477947F82B539C89750193FE1CC5EDCAD6BA8A9B'
    'scan-read-path-locality-ssd-20260826.json' = '5BFAB850D5F4644C95EE31A0A15AEEACDEF75375AA4E4BF36D126C08AD8D1903'
    'scan-read-path-prefix-reuse-hdd-20260827.json' = '791D81DF943EC4F5C79D9D348D3454A39B84FE215A1E45C593FD471AFA7D3699'
    'scan-read-path-prefix-reuse-policy-20260827.json' = '289CA9B5CA51E9148AB1686AAAC26DDE2E96223C7E5DF66D09305E818EE0AB9B'
    'scan-read-path-prefix-reuse-ssd-20260827.json' = 'E78965E68199EC9EE365513DD708006781925886C36A75A0A56CF7E9F3727C34'
    'scan-read-path-sequential-hint-hdd-20260826.json' = '0CD18B65220D9416F01EA0DC580ACCFE515625A4A7D9AEF29B55227116D57FF6'
    'scan-read-path-sequential-hint-ssd-20260826.json' = '2ED2766AE36E0482D2E2DF78C3E6A29ACD641E9FCE6C665F239463DC1537C4D5'
    'scan-read-path-sequential-hint-v2-hdd-20260826.json' = '7BDF49A4C5443BD562617AB8CC8D776E7236112F6D7495FF8EB21AD39D7D5F44'
    'scan-read-path-sequential-hint-v2-ssd-20260826.json' = 'C801D0AC80BE18361E8FFF6EA036C23FD535463646AB62402C12740E6135C006'
}

$rawEvidenceNames = @(
    'scan-read-path-locality-ssd-20260826.json',
    'scan-read-path-locality-hdd-20260826.json',
    'scan-read-path-bucket-order-ssd-20260826.json',
    'scan-read-path-bucket-order-hdd-20260826.json',
    'scan-read-path-buffer-size-ssd-20260826.json',
    'scan-read-path-buffer-size-hdd-20260826.json',
    'scan-read-path-sequential-hint-ssd-20260826.json',
    'scan-read-path-sequential-hint-hdd-20260826.json',
    'scan-read-path-buffer-size-v2-ssd-20260826.json',
    'scan-read-path-buffer-size-v2-hdd-20260826.json',
    'scan-read-path-sequential-hint-v2-ssd-20260826.json',
    'scan-read-path-sequential-hint-v2-hdd-20260826.json',
    'scan-read-path-prefix-reuse-ssd-20260827.json',
    'scan-read-path-prefix-reuse-hdd-20260827.json'
)

Push-Location $repo
try {
    Assert-PowerShellParses $PSCommandPath
    foreach ($entry in $expectedHashes.GetEnumerator()) {
        $path = Join-Path $evidenceRoot $entry.Key
        Assert-True (Test-Path -LiteralPath $path -PathType Leaf) "Missing retained evidence $($entry.Key)."
        Assert-True ((Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash -eq $entry.Value) `
            "Retained SOP7 evidence changed: $($entry.Key)."
    }
    foreach ($name in $rawEvidenceNames) { Assert-RawEvidence $name }

    $localityPolicy = Read-Evidence 'scan-read-path-locality-policy-20260826.json'
    $bucketPolicy = Read-Evidence 'scan-read-path-bucket-order-policy-20260826.json'
    $bufferPolicy = Read-Evidence 'scan-read-path-buffer-read-ahead-policy-v2-20260827.json'
    $prefixPolicy = Read-Evidence 'scan-read-path-prefix-reuse-policy-20260827.json'
    Assert-True ($localityPolicy.decision -eq 'reject_product_change') 'Path locality is no longer rejected.'
    Assert-True ($bucketPolicy.selectedPolicy.bucketOrder -eq 'descending_size') 'Descending buckets are no longer selected.'
    Assert-True ($bufferPolicy.selectedPolicy.solidStateBufferBytes -eq 1048576) 'SSD buffer is no longer 1 MiB.'
    Assert-True ($bufferPolicy.selectedPolicy.rotationalBufferBytes -eq 65536) 'Rotational buffer is no longer 64 KiB.'
    Assert-True ($bufferPolicy.selectedPolicy.unknownBufferBytes -eq 65536) 'Unknown-media buffer is no longer 64 KiB.'
    Assert-True ($bufferPolicy.selectedPolicy.solidStateWindowsSequentialReadHint -eq $false) 'SSD sequential hint was re-enabled.'
    Assert-True ($bufferPolicy.selectedPolicy.rotationalWindowsSequentialReadHint -eq $true) 'Rotational sequential hint was disabled.'
    Assert-True ($bufferPolicy.selectedPolicy.unknownWindowsSequentialReadHint -eq $true) 'Unknown-media sequential hint was disabled.'
    Assert-True ($prefixPolicy.decision -eq 'reject_product_prefix_reuse') 'Prefix reuse is no longer rejected.'
    Assert-True ($prefixPolicy.productDecision.productionPrefixStateBytes -eq 0) 'Production retained prefix state is no longer zero.'
    Assert-True ($prefixPolicy.correctnessAndBounds.savedBytesObserved -eq 2097152) 'Prefix saved-byte result changed.'

    Assert-Contains $hasherSource 'const PARTIAL_HASH_LENGTH: usize = 1024;' 'Partial-hash length changed.'
    Assert-Contains $hasherSource 'const ROTATIONAL_STREAM_BUFFER_LENGTH: usize = 64 * 1024;' 'Rotational buffer constant changed.'
    Assert-Contains $hasherSource 'const SOLID_STATE_STREAM_BUFFER_LENGTH: usize = 1024 * 1024;' 'SSD buffer constant changed.'
    Assert-Contains $hasherSource 'buckets.sort_by(|left, right| right.0.cmp(&left.0));' 'Descending bucket order changed.'
    Assert-Contains $hasherSource 'media != crate::platform::StorageMediaClass::SolidState' 'Media-scoped sequential policy changed.'
    Assert-Contains $cacheSource 'super::xxhash::stream_sequential_hint(media)' 'Cache hashing bypasses the selected hint policy.'
    Assert-True (-not [IO.File]::ReadAllText($hasherSource).Contains('reuse_partial_prefix', [StringComparison]::Ordinal)) `
        'Prefix reuse entered the production hash pipeline.'
    Assert-True (-not [IO.File]::ReadAllText($cacheSource).Contains('reuse_partial_prefix', [StringComparison]::Ordinal)) `
        'Prefix reuse entered the production cache path.'

    Invoke-Checked { & $sop6Verifier } 'The accepted SOP6 scheduler policy or production locks regressed.'
    Invoke-Checked { cargo test -p super-duper-core --lib hasher::read_path } `
        'SOP7 read-path factor, evidence, fixture, bounds, and cancellation tests failed.'
    Invoke-Checked { cargo test -p super-duper-core --lib measured_media_buffers_and_sequential_hint_preserve_exact_hashes } `
        'Media buffer/hint exact-hash test failed.'
    Invoke-Checked { cargo test -p super-duper-core --lib larger_size_buckets_are_admitted_before_smaller_buckets } `
        'Descending bucket-order regression failed.'
    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'
}
finally {
    Pop-Location
}

Write-Output 'SOP7 verifier passed retained evidence identity, one-factor decisions, exact hashes/bytes, descending buckets, media-scoped buffer/read-ahead policy, rejected prefix reuse, SOP6 reader ceilings, cancellation/memory bounds, and production locks.'
