[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$solution = Join-Path $repo 'apps/windows/SuperDuper.Windows.sln'
$evidencePath = Join-Path $repo 'docs/evidence/scan-repeat-cache-policy-20260827.json'
$storeSource = Join-Path $repo 'crates/super-duper-core/src/hasher/repeat_cache.rs'
$engineSource = Join-Path $repo 'crates/super-duper-core/src/engine.rs'
$modelsSource = Join-Path $repo 'crates/super-duper-core/src/storage/models.rs'
$platformSource = Join-Path $repo 'crates/super-duper-core/src/platform/windows.rs'
$workerSource = Join-Path $repo 'crates/super-duper-worker/src/lib.rs'
$setupSource = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/SessionSetupViewModel.cs'
$setupView = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/Views/SessionSetupView.xaml'
$operationViewModel = Join-Path $repo 'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$sop7Verifier = Join-Path $PSScriptRoot 'Verify-WindowsHashReadPath.ps1'

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

Push-Location $repo
try {
    Assert-PowerShellParses $PSCommandPath
    Assert-True (Test-Path -LiteralPath $evidencePath -PathType Leaf) `
        'The retained SOP8 policy evidence is missing.'
    Assert-True ((Get-FileHash -Algorithm SHA256 -LiteralPath $evidencePath).Hash -eq `
        '7DFC8AA6B44DD00F9E307197B4CA82A4B6B2E28888C3F236B9BD8281A9569DC8') `
        'The retained SOP8 policy evidence changed.'
    $evidence = Get-Content -Raw -LiteralPath $evidencePath | ConvertFrom-Json
    Assert-True ($evidence.status -eq 'valid') 'The SOP8 policy evidence is not valid.'
    Assert-True ($evidence.gate -eq 'SOP8-repeat-run-cache') 'The SOP8 evidence gate changed.'
    Assert-True ($evidence.package -eq 'SOP8d-repeat-policy-measurement') 'The SOP8 evidence package changed.'
    Assert-True ($evidence.allSamplesRetained -eq $true) 'The SOP8 evidence discarded a sample.'
    Assert-True ($evidence.retryForFavorableSample -eq $false) 'The SOP8 evidence permits a favorable retry.'
    Assert-True ($evidence.fixtureRemovedAfterProfile -eq $true) 'The SOP8 fixture was not removed.'
    Assert-True ($evidence.cancellationPreflightPassed -eq $true) 'The SOP8 cancellation preflight failed.'
    Assert-True ($evidence.decision.selectedDefault -eq 'reuse_verified') 'Verified reuse is no longer selected.'
    Assert-True ($evidence.decision.reuseWallImprovementBasisPoints -eq 8954) 'The measured wall improvement changed.'
    Assert-True ($evidence.fixture.fileCount -eq 128 -and $evidence.fixture.totalBytes -eq 536870912) `
        'The fixed SOP8 fixture changed.'
    Assert-True (@(Compare-Object @($evidence.armOrder) @(
        'forced_seed', 'reuse_same_process', 'reuse_reopened_store', 'forced_revalidate_tail')).Count -eq 0) `
        'The SOP8 measurement arm order changed.'
    $resultSignature = $evidence.samples[0].resultSignature | ConvertTo-Json -Compress
    foreach ($sample in $evidence.samples) {
        Assert-True ($sample.warningCount -eq 0 -and $sample.cancelledWorkItems -eq 0) `
            "SOP8 arm $($sample.arm) contains a warning or cancellation."
        Assert-True ($sample.confirmedDuplicateGroups -eq 64 -and $sample.confirmedPhysicalItems -eq 128) `
            "SOP8 arm $($sample.arm) changed exact duplicate results."
        Assert-True (($sample.resultSignature | ConvertTo-Json -Compress) -eq $resultSignature) `
            "SOP8 arm $($sample.arm) changed the exact result signature."
    }
    foreach ($sample in @($evidence.samples | Where-Object policy -eq 'reuse_verified')) {
        Assert-True ($sample.partialCacheHits -eq 128 -and $sample.fullCacheHits -eq 128) `
            "Reuse arm $($sample.arm) changed its exact cache hits."
        Assert-True ($sample.partialHashBytesRead -eq 0 -and $sample.fullHashBytesRead -eq 0 -and $sample.processReadBytes -eq 0) `
            "Reuse arm $($sample.arm) performed content reads."
    }
    foreach ($sample in @($evidence.samples | Where-Object policy -eq 'revalidate_content')) {
        Assert-True ($sample.partialHashBytesRead -eq 131072 -and $sample.fullHashBytesRead -eq 536870912) `
            "Forced arm $($sample.arm) changed its declared content reads."
        Assert-True ($sample.processReadBytes -eq 537001984) `
            "Forced arm $($sample.arm) changed its process read bytes."
    }

    Assert-Contains $storeSource 'pub(crate) const STORE_SCHEMA_VERSION: u32 = 2;' `
        'The repeat-cache store schema changed.'
    Assert-Contains $storeSource 'pub(crate) const MAXIMUM_LIVE_ENTRIES: u64 = 1_500_000;' `
        'The repeat-cache live-entry cap changed.'
    Assert-Contains $storeSource 'pub(crate) const PRUNE_TARGET_ENTRIES: u64 = 1_350_000;' `
        'The repeat-cache prune target changed.'
    Assert-Contains $modelsSource 'RepeatCachePolicy::RevalidateContent' `
        'Legacy run snapshots no longer fail back to historical forced reads.'
    Assert-Contains $engineSource 'parameters.repeat_cache_policy' `
        'Hash execution no longer uses immutable run policy truth.'
    Assert-Contains $platformSource 'FILE_READ_ATTRIBUTES' `
        'The Windows signature path no longer uses metadata-only access.'
    Assert-Contains $platformSource 'ChangeTime' `
        'The Windows content-change token changed.'
    Assert-Contains $setupSource 'RepeatCachePolicyNames.ReuseVerified' `
        'The Setup default is no longer verified reuse.'
    Assert-Contains $setupView 'AutomationProperties.AutomationId="RepeatCachePolicy"' `
        'The accessible repeat-cache choice is missing.'
    Assert-Contains $operationViewModel 'public bool CanSubmit => false;' `
        'RecycleOperationViewModel.CanSubmit is no longer locked false.'
    Assert-Contains $compositionRoot `
        'services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();' `
        'Production no longer injects DisabledRecycleOperationCapabilityExecutor.'
    Assert-True (-not [IO.File]::ReadAllText($workerSource).Contains('"executorEnabled": true', [StringComparison]::Ordinal)) `
        'A worker response reports executorEnabled:true.'

    Invoke-Checked { & $sop7Verifier } `
        'The accepted SOP6/SOP7 reader ceilings, read path, evidence, bounds, cancellation, or locks regressed.'
    Invoke-Checked { cargo test -p super-duper-core --lib hasher::repeat_cache } `
        'Repeat-cache store/signature/bounds tests failed.'
    Invoke-Checked { cargo test -p super-duper-core --lib qualified_repeat_cache_reuses_both_hash_stages_after_reopen_and_rejects_edits } `
        'Qualified partial/full reuse and invalidation failed.'
    Invoke-Checked { cargo test -p super-duper-core --lib mutation_between_partial_and_full_never_stores_a_stale_partial_hash } `
        'Between-stage mutation protection failed.'
    Invoke-Checked { cargo test -p super-duper-core --test storage_tests repeat_cache_policy_is_immutable_and_legacy_snapshots_reconstruct_as_forced -- --exact } `
        'Immutable/legacy run policy reconstruction failed.'
    Invoke-Checked { cargo test -p super-duper-worker tests::run_start_defaults_reuse_rejects_unknown_policy_and_reconstructs_exact_alternate -- --exact } `
        'Worker repeat-policy start validation failed.'
    Invoke-Checked { dotnet test apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj --filter 'FullyQualifiedName~SessionSetupViewModelTests|FullyQualifiedName~ShellSessionWorkflowTests' } `
        'Core selected-default, generation, or history tests failed.'
    Invoke-Checked { dotnet test apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj --filter 'FullyQualifiedName~ResultsSurfaces_LoadOnStaWithSystemThemeVirtualizationAndAutomationIds' } `
        'Loaded-STA accessibility/system-theme test failed.'

    Invoke-Checked { cargo clippy -p super-duper-core -p super-duper-worker --all-targets -- `
        -D warnings `
        -A clippy::needless_return `
        -A clippy::let_and_return `
        -A clippy::needless_question_mark `
        -A clippy::too_many_arguments `
        -A clippy::needless_borrows_for_generic_args `
        -A clippy::field_reassign_with_default `
        -A clippy::useless_conversion `
        -A clippy::format_collect } `
        'Strict Core/worker Clippy failed outside the documented unchanged lint classes.'
    Invoke-Checked { cargo test --workspace } 'The full Debug Rust matrix failed.'
    Invoke-Checked { cargo build --workspace } 'The Debug Rust workspace build failed.'
    Invoke-Checked { cargo test --workspace --release } 'The full Release Rust matrix failed.'
    Invoke-Checked { cargo build --workspace --release } 'The Release Rust workspace build failed.'
    Invoke-Checked { dotnet build $solution --configuration Debug } 'The Debug Windows build failed.'
    Invoke-Checked { dotnet test $solution --configuration Debug --no-build -m:1 } `
        'The full Debug Windows matrix failed.'
    Invoke-Checked { dotnet build $solution --configuration Release } 'The Release Windows build failed.'
    Invoke-Checked { dotnet test $solution --configuration Release --no-build -m:1 } `
        'The full Release Windows matrix failed.'
    Invoke-Checked { git -c safe.directory=C:/Users/gary/workspace/super-duper diff --check } `
        'git diff --check failed.'
}
finally {
    Pop-Location
}

Write-Output 'SOP8 verifier passed retained measurement identity, bounded store/signature policy, immutable selected-default/alternate history, generation-safe accessible Setup flow, exact cache/read/telemetry/correctness/cancellation/cloud/hard-link/memory regressions, SOP6/SOP7 decisions, full Debug/Release Rust and Windows matrices, and production locks.'
