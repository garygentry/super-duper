[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Release',
    [switch]$SkipBuild,
    [switch]$ConfirmRecycleBinMutation,
    [switch]$RunProviderNoHydration,
    [string]$CloudRoot,
    [string]$LocallyAvailableFile,
    [string]$OfflinePlaceholder,
    [string[]]$ProviderProcessName,
    [switch]$RunWarmQueryProfile,
    [string]$EvidenceDirectory
)

$ErrorActionPreference = 'Stop'

function Add-MatrixEntry(
    [string]$Gate,
    [string]$Status,
    [string]$Evidence,
    [string]$Boundary
) {
    $script:matrix.Add([ordered]@{
        gate = $Gate
        status = $Status
        evidence = $Evidence
        boundary = $Boundary
    })
}

function Invoke-LoggedNative(
    [string]$Name,
    [string]$FilePath,
    [string[]]$Arguments
) {
    $logPath = Join-Path $script:evidenceRoot "$Name.log"
    $started = Get-Date
    & $FilePath @Arguments 2>&1 | Tee-Object -LiteralPath $logPath | Out-Host
    $exitCode = $LASTEXITCODE
    $elapsed = (Get-Date) - $started
    $result = [ordered]@{
        name = $Name
        command = "$FilePath $($Arguments -join ' ')"
        exitCode = $exitCode
        elapsedMilliseconds = [math]::Round($elapsed.TotalMilliseconds, 2)
        log = $logPath
    }
    $script:commands.Add($result)
    return $result
}

function Assert-Executed([object]$Result) {
    if ($Result.exitCode -ne 0) {
        throw "$($Result.name) failed with exit code $($Result.exitCode). See $($Result.log)."
    }
}

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$project = Join-Path $repoRoot 'apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj'
if (-not (Test-Path -LiteralPath $project -PathType Leaf)) {
    throw "Infrastructure test project was not found: $project"
}

if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $EvidenceDirectory = Join-Path $repoRoot "artifacts/windows-recycle-bin-acceptance/$stamp"
}
$evidenceRoot = [IO.Path]::GetFullPath($EvidenceDirectory)
$repoBoundary = $repoRoot.TrimEnd('\') + '\'
if (-not $evidenceRoot.StartsWith($repoBoundary, [StringComparison]::OrdinalIgnoreCase)) {
    throw "EvidenceDirectory must stay inside the repository: $evidenceRoot"
}
$null = New-Item -ItemType Directory -Path $evidenceRoot -Force

$matrix = [Collections.Generic.List[object]]::new()
$commands = [Collections.Generic.List[object]]::new()
$measurements = [Collections.Generic.List[object]]::new()
$failure = $null
$gitHead = (& git -c safe.directory=C:/Users/gary/workspace/super-duper rev-parse HEAD).Trim()
$os = [ordered]@{
    caption = [Runtime.InteropServices.RuntimeInformation]::OSDescription
    version = [Environment]::OSVersion.Version.ToString()
    build = [Environment]::OSVersion.Version.Build
}
$computer = [ordered]@{
    model = $null
    logicalProcessors = [Environment]::ProcessorCount
    totalPhysicalMemory = $null
}
$video = @()
try {
    $cimOs = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
    $cimComputer = Get-CimInstance Win32_ComputerSystem -ErrorAction Stop
    $os = [ordered]@{ caption = $cimOs.Caption; version = $cimOs.Version; build = $cimOs.BuildNumber }
    $computer = [ordered]@{
        model = $cimComputer.Model
        logicalProcessors = $cimComputer.NumberOfLogicalProcessors
        totalPhysicalMemory = $cimComputer.TotalPhysicalMemory
    }
    $video = @(Get-CimInstance Win32_VideoController -ErrorAction Stop | ForEach-Object {
        [ordered]@{ name = $_.Name; currentHorizontalResolution = $_.CurrentHorizontalResolution; currentVerticalResolution = $_.CurrentVerticalResolution }
    })
}
catch {
    Write-Warning "CIM host detail is unavailable; recording process-visible fallbacks: $($_.Exception.Message)"
}

Push-Location $repoRoot
try {
    if (-not $SkipBuild) {
        $build = Invoke-LoggedNative 'infrastructure-build' 'dotnet' @(
            'build', $project, '-c', $Configuration, '--no-restore'
        )
        Assert-Executed $build
    }

    $contract = Invoke-LoggedNative 'deterministic-executor-contract' 'dotnet' @(
        'test', $project, '-c', $Configuration, '--no-build', '--no-restore',
        '--filter', 'FullyQualifiedName~WindowsRecycleOperationExecutorTests&TestCategory!=RealRecycleBin',
        '--logger', "trx;LogFileName=deterministic-executor-contract.trx",
        '--results-directory', $evidenceRoot
    )
    Assert-Executed $contract
    Add-MatrixEntry 'Deterministic callback/HRESULT/flag contract' 'passed' `
        'Focused Infrastructure tests and TRX evidence.' `
        'Stable mapping coverage is not a real Shell/provider outcome.'

    if ($ConfirmRecycleBinMutation) {
        $mutation = Invoke-LoggedNative 'real-recycle-bin' 'pwsh' @(
            '-NoProfile', '-File', (Join-Path $PSScriptRoot 'Invoke-WindowsRecycleBinSmoke.ps1'),
            '-Configuration', $Configuration, '-ConfirmRecycleBinMutation', '-SkipBuild'
        )
        Assert-Executed $mutation
        Add-MatrixEntry 'Local success and callback/abort evidence' 'passed' `
            'Disposable hard-link and exact-folder items produced real Shell results.' `
            'Fixtures remain recoverable in the current user Recycle Bin.'
        Add-MatrixEntry 'Post-start cancellation' 'passed' `
            'Real PreDeleteItem cancellation kept its source unchanged.' `
            'Does not prove cancellation during provider work.'
        Add-MatrixEntry 'Locked-file mapping' 'passed' `
            'Real locked source remained byte-identical and returned structured sharing violation.' `
            'Host evidence is not a provider-wide contract.'
    }
    else {
        Add-MatrixEntry 'Local success and callback/abort evidence' 'not_run' `
            'Mutation was not explicitly authorized for this run.' `
            'Use -ConfirmRecycleBinMutation; successful fixtures remain recoverable and are not permanently cleaned.'
        Add-MatrixEntry 'Post-start cancellation' 'not_run' `
            'Mutation was not explicitly authorized for this run.' `
            'Existing automated seams do not replace a real Shell pass.'
        Add-MatrixEntry 'Locked-file mapping' 'not_run' `
            'Mutation was not explicitly authorized for this run.' `
            'Deterministic HRESULT mapping alone does not close this gate.'
    }

    if ($RunProviderNoHydration) {
        foreach ($required in @(
            @{ name = 'CloudRoot'; value = $CloudRoot },
            @{ name = 'LocallyAvailableFile'; value = $LocallyAvailableFile },
            @{ name = 'OfflinePlaceholder'; value = $OfflinePlaceholder }
        )) {
            if ([string]::IsNullOrWhiteSpace($required.value)) {
                throw "$($required.name) is required with -RunProviderNoHydration. Fixture discovery is intentionally not automatic."
            }
        }
        if ($null -eq $ProviderProcessName -or $ProviderProcessName.Count -eq 0) {
            throw 'ProviderProcessName is required with -RunProviderNoHydration.'
        }
        $env:SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS = '1'
        $env:SUPER_DUPER_RECYCLE_CLOUD_ROOT = $CloudRoot
        $env:SUPER_DUPER_RECYCLE_LOCAL_FILE = $LocallyAvailableFile
        $env:SUPER_DUPER_RECYCLE_OFFLINE_FILE = $OfflinePlaceholder
        $env:SUPER_DUPER_RECYCLE_PROVIDER_PROCESSES = $ProviderProcessName -join ';'
        $provider = Invoke-LoggedNative 'provider-no-hydration' 'dotnet' @(
            'test', $project, '-c', $Configuration, '--no-build', '--no-restore',
            '--filter', 'TestCategory=RealRecycleBinProvider',
            '--logger', "trx;LogFileName=provider-no-hydration.trx",
            '--results-directory', $evidenceRoot,
            '--logger', 'console;verbosity=detailed'
        )
        Assert-Executed $provider
        Add-MatrixEntry 'Registered-provider no hydration' 'passed' `
            'Explicit local/offline fixtures retained attributes, allocation, timestamps, and stable provider transfer counters.' `
            'Metadata-only executor inspection; excluded content was never opened or read.'
    }
    else {
        Add-MatrixEntry 'Registered-provider no hydration' 'not_run' `
            'No explicit registered-root fixtures were supplied.' `
            'The script never auto-discovers fixtures or opens excluded content.'
    }

    if ($RunWarmQueryProfile) {
        $profile = Invoke-LoggedNative 'representative-review-warm-query' 'cargo' @(
            'test', '-p', 'super-duper-core', '--release', '--test', 'storage_tests',
            'representative_review_workspace_profile', '--', '--ignored', '--exact', '--nocapture', '--test-threads=1'
        )
        $profileLine = Get-Content -LiteralPath (Join-Path $evidenceRoot 'representative-review-warm-query.log') |
            Where-Object { $_ -like 'review-profile samples=*' } |
            Select-Object -Last 1
        if ($profileLine -match 'samples=(?<samples>\d+) groups-p50=(?<groupsP50>[\d.]+)ms groups-p95=(?<groupsP95>[\d.]+)ms groups-p99=(?<groupsP99>[\d.]+)ms root-facets-p95=(?<rootP95>[\d.]+)ms drive-facets-p95=(?<driveP95>[\d.]+)ms review-plan-p95=(?<planP95>[\d.]+)ms review-groups-p95=(?<reviewP95>[\d.]+)ms private-growth=(?<growth>\d+) bytes') {
            $measurements.Add([ordered]@{
                name = 'representative_review_workspace_profile'
                samples = [int]$Matches.samples
                groupsP50Milliseconds = [double]$Matches.groupsP50
                groupsP95Milliseconds = [double]$Matches.groupsP95
                groupsP99Milliseconds = [double]$Matches.groupsP99
                rootFacetsP95Milliseconds = [double]$Matches.rootP95
                driveFacetsP95Milliseconds = [double]$Matches.driveP95
                reviewPlanP95Milliseconds = [double]$Matches.planP95
                reviewGroupsP95Milliseconds = [double]$Matches.reviewP95
                privateGrowthBytes = [long]$Matches.growth
                testPassed = $profile.exitCode -eq 0
                acceptedAsRepresentative = $false
            })
        }
        if ($profile.exitCode -eq 0) {
            Add-MatrixEntry 'Representative review warm queries' 'measured' `
                'The existing 100-sample Release profile completed; timings are in the evidence log and JSON measurement.' `
                'This is an independent Milestone 8 gate, not large-plan Shell-operation evidence.'
        }
        else {
            Add-MatrixEntry 'Representative review warm queries' 'failed' `
                'The 100-sample Release profile exceeded a numeric ceiling; observed metrics remain captured in the log and JSON measurement.' `
                'The gate remains open and this failure is not large-plan Shell-operation evidence.'
            Assert-Executed $profile
        }
    }
    else {
        Add-MatrixEntry 'Representative review warm queries' 'not_run' `
            'The long-running explicit profile was not requested.' `
            'Use -RunWarmQueryProfile on representative hardware.'
    }
}
catch {
    $failure = $_.Exception.Message
}
finally {
    Remove-Item Env:SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_RECYCLE_CLOUD_ROOT -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_RECYCLE_LOCAL_FILE -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_RECYCLE_OFFLINE_FILE -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_RECYCLE_PROVIDER_PROCESSES -ErrorAction SilentlyContinue
    Pop-Location
}

Add-MatrixEntry 'Access-denied Shell outcome' 'open' `
    'Stable Win32/copy-engine mappings are automated.' `
    'A real disposable ACL/elevation pass has not been captured.'
Add-MatrixEntry 'Root disconnection' 'open' `
    'Stable Win32/copy-engine mappings are automated.' `
    'Requires real removable/mapped/provider media and controlled disconnect.'
Add-MatrixEntry 'Recycle Bin capacity/oversized item' 'open' `
    'Stable copy-engine capacity mappings are automated.' `
    'No test may permanently delete or silently fall back when capacity is unavailable.'
Add-MatrixEntry 'Provider-specific Shell HRESULTs' 'open' `
    'Provider unavailable/failure/paused mappings are automated.' `
    'Real provider outcomes remain required; no placeholder may be hydrated to induce them.'
Add-MatrixEntry 'Residual Shell TOCTOU and ambiguous-start recovery' 'open' `
    'Admission, durable-start, and non-retry seams are automated.' `
    'A controlled real path-swap/process-loss campaign remains required.'
Add-MatrixEntry 'Representative large-plan operation performance' 'open' `
    'No qualifying large disposable operation fixture was executed.' `
    'Small local Shell passes and review-query profiles cannot close this gate.'
Add-MatrixEntry 'Five-minute/60-second/30-second/32-entry constants' 'provisional' `
    'Expiry and bound enforcement are automated.' `
    'Usability and safety decisions require the large-plan operator evidence.'
Add-MatrixEntry 'FOFX_ADDUNDORECORD' 'omitted_pending_decision' `
    'The exact flag regression asserts that 0x20000000 is absent.' `
    'Do not add it until separately reviewed operator evidence decides Windows Undo behavior.'
Add-MatrixEntry 'Narrator/NVDA, OS high contrast, multi-monitor DPI' 'open' `
    'Automated UI Automation/layout contracts remain separate.' `
    'Requires physical listening, theme, and per-monitor transition evidence.'
Add-MatrixEntry 'Production wiring' 'disabled' `
    'App composition, WPF submission, and worker executorEnabled remain disabled.' `
    'This matrix never authorizes production execution or Milestone 11 completion.'

$evidence = [ordered]@{
    schemaVersion = 1
    capturedAt = (Get-Date).ToUniversalTime().ToString('O')
    repository = $repoRoot
    gitHead = $gitHead
    configuration = $Configuration
    host = [ordered]@{
        computerModel = $computer.model
        logicalProcessors = $computer.logicalProcessors
        totalPhysicalMemory = $computer.totalPhysicalMemory
        osCaption = $os.caption
        osVersion = $os.version
        osBuild = $os.build
        videoControllers = $video
    }
    commands = $commands
    measurements = $measurements
    matrix = $matrix
    productionEnabled = $false
    milestone11Complete = $false
    failure = $failure
}
$jsonPath = Join-Path $evidenceRoot 'acceptance-evidence.json'
$evidence | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $jsonPath -Encoding utf8

$report = [Collections.Generic.List[string]]::new()
$report.Add('# Windows Recycle Bin acceptance evidence')
$report.Add('')
$report.Add("Captured: $($evidence.capturedAt)")
$report.Add("")
$report.Add("Git HEAD: ``$gitHead``")
$report.Add('')
$report.Add('| Gate | Status | Evidence | Boundary |')
$report.Add('|---|---|---|---|')
foreach ($entry in $matrix) {
    $report.Add("| $($entry.gate -replace '\|', '\|') | ``$($entry.status)`` | $($entry.evidence -replace '\|', '\|') | $($entry.boundary -replace '\|', '\|') |")
}
$report.Add('')
$report.Add('Production wiring remains disabled and this evidence does not claim Milestone 11 complete.')
if ($null -ne $failure) {
    $report.Add('')
    $report.Add("Run failure: $failure")
}
$reportPath = Join-Path $evidenceRoot 'acceptance-report.md'
$report | Set-Content -LiteralPath $reportPath -Encoding utf8

Write-Output "ACCEPTANCE_EVIDENCE=$jsonPath"
Write-Output "ACCEPTANCE_REPORT=$reportPath"
Write-Output 'PRODUCTION_EXECUTOR_ENABLED=false'
Write-Output 'MILESTONE_11_COMPLETE=false'
if ($null -ne $failure) {
    throw $failure
}
