[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$runner = Join-Path $PSScriptRoot 'Invoke-WindowsLargeDriveAcceptance.ps1'
$watchdog = Join-Path $PSScriptRoot 'Sop9PersistenceWatchdog.ps1'
$protocol = Join-Path $repo 'docs/scan-large-drive-acceptance-protocol-v2.md'
$plan = Join-Path $repo 'docs/scan-optimization-plan.md'
$handoff = Join-Path $repo 'docs/windows-roadmap-session-handoff.md'
$roadmap = Join-Path $repo 'ROADMAP.md'
$v1Summary = Join-Path $repo 'docs/evidence/scan-large-drive-single-drive-invalid-20260828.json'
$v2EvidenceRoot = Join-Path $repo `
    'artifacts/windows-sop9-large-drive/sop9c-single-drive-reference-repeat-v2'
$operationViewModel = Join-Path $repo `
    'apps/windows/src/SuperDuper.Windows.Core/ViewModels/RecycleOperationViewModel.cs'
$compositionRoot = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/App.xaml.cs'
$workerSource = Join-Path $repo 'crates/super-duper-worker/src/lib.rs'
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) `
    ('super-duper-sop9-v2-protocol-' + [guid]::NewGuid().ToString('N'))

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

function Assert-PowerShellParses([string]$Path) {
    $tokens = $null
    $errors = $null
    [void][Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$errors)
    if ($errors.Count -ne 0) { throw "PowerShell parsing failed for $Path`: $($errors -join '; ')" }
}

Push-Location $repo
try {
    foreach ($path in @($runner, $watchdog, $PSCommandPath)) { Assert-PowerShellParses $path }

    $selfTest = & $runner -VerifyPersistenceWatchdog | ConvertFrom-Json
    Assert-Equal $selfTest.status 'passed' 'The persistence-watchdog self-test failed.'
    Assert-Equal $selfTest.beforeIdleBoundary 'continue_waiting' `
        'The watchdog stopped before the fixed idle boundary.'
    Assert-Equal $selfTest.atIdleBoundary 'timeout_idle' `
        'The watchdog did not stop at the fixed idle boundary.'
    Assert-Equal $selfTest.atAbsoluteBoundary 'timeout_absolute' `
        'The watchdog did not stop at the fixed absolute boundary.'
    Assert-Equal $selfTest.activityPrecedesIdleEvaluation 'continue_activity' `
        'Fresh persistence activity did not renew the idle bound.'

    $description = & $runner -Campaign sop9c-single-drive-reference-repeat-v2 -DescribeOnly |
        ConvertFrom-Json
    Assert-Equal $description.campaignId 'sop9c-single-drive-reference-repeat-v2' `
        'The fixed V2 identity changed.'
    Assert-True ($description.physical -and $description.rootIds.Count -eq 1 -and
        $description.rootIds[0] -eq 'E:') 'The fixed V2 single-drive contract changed.'
    Assert-True ($description.policies.Count -eq 2 -and
        $description.policies[0] -eq 'revalidate_content' -and
        $description.policies[1] -eq 'reuse_verified') 'The fixed V2 arm order changed.'
    Assert-Equal $description.expectedTerminal 'completed' 'The V2 terminal requirement changed.'
    Assert-True (-not $description.cancelAfterFirstHashProgress) `
        'V2 unexpectedly became a cancellation campaign.'
    Assert-Equal $description.frameWatchdog.Mode 'persistence_activity_v2' `
        'The V2 watchdog mode changed.'
    Assert-Equal $description.frameWatchdog.FrameTimeoutSeconds 180 `
        'The non-persistence frame bound changed.'
    Assert-Equal $description.frameWatchdog.ProbeIntervalSeconds 5 `
        'The persistence probe interval changed.'
    Assert-Equal $description.frameWatchdog.PersistenceIdleTimeoutSeconds 900 `
        'The persistence idle bound changed.'
    Assert-Equal $description.frameWatchdog.PersistenceAbsoluteTimeoutSeconds 86400 `
        'The persistence absolute bound changed.'
    Assert-Equal $description.frameWatchdog.JournalIntervalSeconds 600 `
        'The persistence journal bound changed.'

    $v1Description = & $runner -Campaign sop9c-single-drive-reference-repeat-v1 -DescribeOnly |
        ConvertFrom-Json
    Assert-Equal $v1Description.frameWatchdog.Mode 'fixed_frame_v1' `
        'The consumed V1 identity no longer describes its historical fixed-frame policy.'
    Assert-Equal $v1Description.frameWatchdog.FrameTimeoutSeconds 180 `
        'The consumed V1 identity no longer describes its historical 180-second deadline.'

    $executionAdmissionBlocked = $false
    try {
        & $runner -Campaign sop9c-single-drive-reference-repeat-v2
    }
    catch {
        $executionAdmissionBlocked = $_.Exception.Message.Contains(
            'requires the separately authorized -RunPhysicalCampaign switch.',
            [StringComparison]::Ordinal)
    }
    Assert-True $executionAdmissionBlocked `
        'Naming the V2 identity without the execution switch did not fail closed.'

    $runnerText = [IO.File]::ReadAllText($runner)
    Assert-Equal ([regex]::Matches($runnerText, 'ReadLineAsync\(\)').Count) 1 `
        'The runner no longer owns exactly one stdout read site.'
    Assert-Contains $runner 'PendingRead = $null' `
        'The one-pending-read state is missing.'
    Assert-Contains $runner "Mode = 'fixed_frame_v1'" `
        'Historical campaign identities no longer retain the V1 frame policy.'
    Assert-Contains $runner "reason = 'state_activity_idle_bound'" `
        'The V2 idle failure reason is missing.'
    Assert-Contains $runner "reason = 'absolute_phase_bound'" `
        'The V2 absolute failure reason is missing.'
    Assert-Contains $runner "Add-Journal 'persistence_watchdog_completed'" `
        'The V2 terminal watchdog summary is missing.'
    Assert-Contains $runner `
        "Physical SOP9 execution requires the separately authorized -RunPhysicalCampaign switch." `
        'The physical-execution admission switch is missing.'

    [IO.Directory]::CreateDirectory($tempRoot) | Out-Null
    $productDb = Join-Path $tempRoot 'product.db'
    [IO.File]::WriteAllText($productDb, 'product')
    . $watchdog
    $before = Get-Sop9PersistenceStateFingerprint $productDb
    [IO.File]::AppendAllText("$productDb-wal", 'durable activity')
    $after = Get-Sop9PersistenceStateFingerprint $productDb
    Assert-True ($before -ne $after) `
        'Campaign-owned product database/WAL metadata changes did not advance the fingerprint.'

    Assert-True (-not (Test-Path -LiteralPath $v2EvidenceRoot)) `
        'The design-only slice consumed the V2 evidence identity.'
    Assert-Equal (Get-FileHash -Algorithm SHA256 -LiteralPath $v1Summary).Hash `
        'D5C048A0452F70039B524E845C8698596E713603F9B2A4B8138F0F476EDA61B8' `
        'The immutable V1 incident summary changed.'
    $v1 = Get-Content -Raw -LiteralPath $v1Summary | ConvertFrom-Json -Depth 100
    Assert-True ($v1.writeOnceIdentityConsumed -and -not $v1.blocker.v2Authorized -and
        -not $v1.blocker.sop9dAuthorized) 'Historical V1 authority truth changed.'

    Assert-Contains $protocol 'does not authorize an invocation' `
        'The V2 protocol does not preserve the separate execution decision.'
    Assert-Contains $protocol '`sop9c-single-drive-reference-repeat-v2`' `
        'The V2 protocol identity is missing.'
    Assert-Contains $plan '`SOP9c-single-drive-reference-repeat-v2-protocol` | `accepted`' `
        'The V2 design package is not accepted in the scan plan.'
    Assert-Contains $handoff 'exactly one V2 physical invocation' `
        'The handoff does not stop at the separate V2 execution boundary.'
    Assert-Contains $roadmap 'V2 protocol is committed but unconsumed' `
        'ROADMAP does not preserve the design-only V2 disposition.'

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
    if (Test-Path -LiteralPath $tempRoot) {
        $tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
        $resolved = [IO.Path]::GetFullPath($tempRoot).TrimEnd('\')
        if (-not $resolved.StartsWith($tempParent + '\', [StringComparison]::OrdinalIgnoreCase) -or
            $resolved -eq $tempParent) {
            throw "Refusing unsafe watchdog-fixture cleanup: $resolved"
        }
        $item = Get-Item -LiteralPath $resolved -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Refusing reparse-point watchdog-fixture cleanup: $resolved"
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

Write-Output 'SOP9c V2 protocol verifier passed the fixed unconsumed identity, one-pending-read persistence watchdog, 180-second/5-second/15-minute/24-hour bounds, deterministic controller and metadata-activity tests, immutable V1 incident, separate execution authority, and all production locks without a physical preflight or run.'
