[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Release',
    [switch]$ConfirmControlledProcessLoss,
    [switch]$SkipBuild,
    [string]$EvidenceDirectory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (-not $ConfirmControlledProcessLoss) {
    throw 'This gate intentionally terminates only a disposable test host after durable shell_started. Rerun with -ConfirmControlledProcessLoss.'
}

$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$hostProject = Join-Path $repo 'apps/windows/tools/SuperDuper.Windows.AmbiguousStartHost/SuperDuper.Windows.AmbiguousStartHost.csproj'
$solution = Join-Path $repo 'apps/windows/SuperDuper.Windows.sln'
$cargoProfile = if ($Configuration -eq 'Release') { 'release' } else { 'debug' }
$framework = 'net10.0-windows10.0.22000.0'
$worker = Join-Path $repo "target/$cargoProfile/super-duper-worker.exe"
$hostExecutable = Join-Path $repo "apps/windows/tools/SuperDuper.Windows.AmbiguousStartHost/bin/$Configuration/$framework/SuperDuper.Windows.AmbiguousStartHost.exe"
$appExecutable = Join-Path $repo "apps/windows/src/SuperDuper.Windows/bin/$Configuration/$framework/win-x64/SuperDuper.Windows.exe"

if ([string]::IsNullOrWhiteSpace($EvidenceDirectory)) {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
    $EvidenceDirectory = Join-Path $repo "artifacts/windows-ambiguous-start/$stamp"
}
$evidenceRoot = [IO.Path]::GetFullPath($EvidenceDirectory)
$repoBoundary = $repo.TrimEnd('\') + '\'
if (-not $evidenceRoot.StartsWith($repoBoundary, [StringComparison]::OrdinalIgnoreCase)) {
    throw "EvidenceDirectory must stay inside the repository: $evidenceRoot"
}
$cursor = $evidenceRoot
while (-not $cursor.Equals($repo, [StringComparison]::OrdinalIgnoreCase)) {
    if (Test-Path -LiteralPath $cursor) {
        $item = Get-Item -LiteralPath $cursor -Force
        if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "EvidenceDirectory must contain only ordinary directories: $cursor"
        }
    }
    $parent = [IO.Path]::GetDirectoryName($cursor)
    if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $cursor) {
        throw "EvidenceDirectory parent validation failed: $evidenceRoot"
    }
    $cursor = $parent
}
if (Test-Path -LiteralPath $evidenceRoot) {
    if (@(Get-ChildItem -LiteralPath $evidenceRoot -Force).Count -ne 0) {
        throw "EvidenceDirectory must be new or empty: $evidenceRoot"
    }
}
else {
    $null = New-Item -ItemType Directory -Path $evidenceRoot
}

$failure = $null
$hostProcess = $null
$appProcess = $null
$database = Join-Path $evidenceRoot 'campaign.db'
$hashCache = Join-Path $evidenceRoot 'hash-cache'
$fixtureRoot = Join-Path $evidenceRoot 'fixtures'
$marker = Join-Path $evidenceRoot 'durable-shell-start.json'
$defaultWorkerLog = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)) 'SuperDuper/logs/worker.log'

function Save-Failure([string]$Stage, [string]$Message) {
    [ordered]@{
        schemaVersion = 1
        gate = 'WPM11-ambiguous-start'
        stage = $Stage
        capturedAtUtc = (Get-Date).ToUniversalTime().ToString('O')
        message = $Message
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $evidenceRoot "failure-$Stage.json") -Encoding utf8
}

Push-Location $repo
try {
    if (-not $SkipBuild) {
        & cargo build -p super-duper-worker $(if ($Configuration -eq 'Release') { '--release' }) 2>&1 |
            Tee-Object -LiteralPath (Join-Path $evidenceRoot 'worker-build.log') | Out-Host
        if ($LASTEXITCODE -ne 0) { throw 'Rust worker build failed.' }
        & dotnet build $hostProject -c $Configuration 2>&1 |
            Tee-Object -LiteralPath (Join-Path $evidenceRoot 'campaign-host-build.log') | Out-Host
        if ($LASTEXITCODE -ne 0) { throw 'Disposable campaign host build failed.' }
        & dotnet build $solution -c $Configuration -m:1 2>&1 |
            Tee-Object -LiteralPath (Join-Path $evidenceRoot 'wpf-build.log') | Out-Host
        if ($LASTEXITCODE -ne 0) { throw 'WPF solution build failed.' }
    }
    foreach ($required in @($worker, $hostExecutable, $appExecutable)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
            throw "Required campaign executable was not found: $required"
        }
    }

    [ordered]@{
        schemaVersion = 1
        gate = 'WPM11-ambiguous-start'
        startedAtUtc = (Get-Date).ToUniversalTime().ToString('O')
        gitHead = (& git -c safe.directory=C:/Users/gary/workspace/super-duper rev-parse HEAD).Trim()
        configuration = $Configuration
        repository = $repo
        evidenceRoot = $evidenceRoot
        worker = $worker
        hostExecutable = $hostExecutable
        appExecutable = $appExecutable
        productionEnabled = $false
        milestone11Complete = $false
        automaticLiveInspection = $false
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $evidenceRoot 'campaign-run.json') -Encoding utf8

    $arguments = @(
        '--mode', 'prepare',
        '--worker', $worker,
        '--database', $database,
        '--hash-cache', $hashCache,
        '--fixture-root', $fixtureRoot,
        '--evidence-root', $evidenceRoot
    )
    $hostProcess = Start-Process -FilePath $hostExecutable -ArgumentList $arguments `
        -WorkingDirectory (Split-Path -Parent $hostExecutable) -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $evidenceRoot 'campaign-host.stdout.log') `
        -RedirectStandardError (Join-Path $evidenceRoot 'campaign-host.stderr.log')

    $deadline = (Get-Date).AddMinutes(3)
    while (-not (Test-Path -LiteralPath $marker -PathType Leaf)) {
        if ($hostProcess.HasExited) {
            throw "Disposable host exited before durable shell_started (exit $($hostProcess.ExitCode))."
        }
        if ((Get-Date) -ge $deadline) {
            throw 'Timed out waiting for durable shell_started.'
        }
        Start-Sleep -Milliseconds 100
        $hostProcess.Refresh()
    }

    $durable = Get-Content -Raw -LiteralPath $marker | ConvertFrom-Json
    if ($durable.hostProcessId -ne $hostProcess.Id -or
        $durable.operationStatus -ne 'executing' -or
        $durable.executorEnabled -ne $false -or
        $durable.performOperationsCalled -ne $false) {
        throw 'Durable-start marker did not match the controlled pre-PerformOperations boundary.'
    }
    $childWorkers = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($hostProcess.Id)" |
        Where-Object { [IO.Path]::GetFullPath($_.ExecutablePath).Equals($worker, [StringComparison]::OrdinalIgnoreCase) })
    [ordered]@{
        schemaVersion = 1
        capturedAtUtc = (Get-Date).ToUniversalTime().ToString('O')
        terminatedProcess = [ordered]@{ id = $hostProcess.Id; path = $hostExecutable }
        observedWorkerChildren = @($childWorkers | ForEach-Object { [ordered]@{ id = $_.ProcessId; path = $_.ExecutablePath } })
        explorerTerminated = $false
        providerTerminated = $false
        workerTerminatedByCampaign = $false
        databaseRemoved = $false
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $evidenceRoot 'process-loss-boundary.json') -Encoding utf8

    Stop-Process -Id $hostProcess.Id -Force
    $hostProcess.WaitForExit()
    $workerDeadline = (Get-Date).AddSeconds(20)
    foreach ($workerProcess in $childWorkers) {
        while ((Get-Process -Id $workerProcess.ProcessId -ErrorAction SilentlyContinue) -and (Get-Date) -lt $workerDeadline) {
            Start-Sleep -Milliseconds 100
        }
        if (Get-Process -Id $workerProcess.ProcessId -ErrorAction SilentlyContinue) {
            throw "Owned worker $($workerProcess.ProcessId) did not exit naturally after its stdin owner was lost; it was not terminated."
        }
    }
    if (-not (Test-Path -LiteralPath $database -PathType Leaf)) {
        throw 'The durable worker database was not preserved after process loss.'
    }
    if (Test-Path -LiteralPath $defaultWorkerLog -PathType Leaf) {
        Copy-Item -LiteralPath $defaultWorkerLog -Destination (Join-Path $evidenceRoot 'worker-after-host-loss.log')
    }

    $fixture = Get-Content -Raw -LiteralPath (Join-Path $evidenceRoot 'fixture-description.json') | ConvertFrom-Json
    & cargo run --quiet -p super-duper-core --example windows_ambiguous_start_evidence -- `
        $database $fixture.recycleOperationId (Join-Path $evidenceRoot 'recovered-source.json') 2>&1 |
        Tee-Object -LiteralPath (Join-Path $evidenceRoot 'recovered-source-command.log') | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Restart/source snapshot failed.' }

    & $hostExecutable --mode verify --worker $worker --database $database --hash-cache $hashCache `
        --operation-id $fixture.recycleOperationId --output (Join-Path $evidenceRoot 'recovered-protocol.json') 2>&1 |
        Tee-Object -LiteralPath (Join-Path $evidenceRoot 'recovered-protocol-command.log') | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Restart/protocol reconstruction failed.' }

    $env:SUPER_DUPER_DB_PATH = $database
    $env:HASH_CACHE_PATH = $hashCache
    $env:SUPER_DUPER_WORKER_PATH = $worker
    $env:SUPER_DUPER_DISABLE_CLOUD_REGISTRATION_DISCOVERY = '1'
    $appProcess = Start-Process -FilePath $appExecutable -WorkingDirectory (Split-Path -Parent $appExecutable) -PassThru
    [ordered]@{
        schemaVersion = 1
        state = 'awaiting_operator_review'
        updatedAtUtc = (Get-Date).ToUniversalTime().ToString('O')
        appProcessId = $appProcess.Id
        appExecutable = $appExecutable
        recycleOperationId = $fixture.recycleOperationId
        unknownItemIds = @($fixture.itemIds)
        requiredManualActions = @(
            'Open the preserved completed run and confirm recovery_required.',
            'Record a deferred_unresolved observation for one item before inspection.',
            'Inspect the source folder and Recycle Bin manually without app inference.',
            'Correct the deferred record to observed_at_source with a correction reason.',
            'Record observed_at_source for the other item and confirm review_complete_with_unresolved_evidence.'
        )
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $evidenceRoot 'campaign-state.json') -Encoding utf8
    Write-Output "EVIDENCE_ROOT=$evidenceRoot"
    Write-Output "APP_PID=$($appProcess.Id)"
    Write-Output 'Controlled host loss is complete. The WPF app is ready for the manual Option A checklist.'
}
catch {
    $failure = $_
    Save-Failure 'prepare' $_.Exception.Message
    throw
}
finally {
    Remove-Item Env:SUPER_DUPER_DB_PATH -ErrorAction SilentlyContinue
    Remove-Item Env:HASH_CACHE_PATH -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_WORKER_PATH -ErrorAction SilentlyContinue
    Remove-Item Env:SUPER_DUPER_DISABLE_CLOUD_REGISTRATION_DISCOVERY -ErrorAction SilentlyContinue
    Pop-Location
}
