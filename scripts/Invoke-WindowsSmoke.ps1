[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Debug',
    [switch]$SkipBuild,
    [switch]$SkipWpf,
    [switch]$KeepArtifacts,
    [string[]]$AdditionalRoot = @()
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$solution = Join-Path $repo 'apps/windows/SuperDuper.Windows.sln'
$profile = if ($Configuration -eq 'Release') { 'release' } else { 'debug' }
$worker = Join-Path $repo "target/$profile/super-duper-worker.exe"
$app = Join-Path $repo "apps/windows/src/SuperDuper.Windows/bin/$Configuration/net10.0-windows10.0.22000.0/win-x64/SuperDuper.Windows.exe"
$smokeRoot = Join-Path ([IO.Path]::GetTempPath()) ("super-duper-windows-smoke-" + [guid]::NewGuid().ToString('N'))
$database = Join-Path $smokeRoot 'smoke.db'
$cache = Join-Path $smokeRoot 'hash-cache'
$allFrames = [Collections.Generic.List[object]]::new()
$fixture = $null
$connection = $null
$restored = $null

function Invoke-Checked([scriptblock]$Command, [string]$Description) {
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

function New-SmokeFixture([string]$Root) {
    $results = Join-Path $Root 'results'
    $cancel = Join-Path $Root 'cancel'
    [IO.Directory]::CreateDirectory($results) | Out-Null
    [IO.Directory]::CreateDirectory($cancel) | Out-Null

    for ($index = 0; $index -lt 230; $index++) {
        $bytes = [byte[]]::new(1024 + $index)
        [Array]::Fill($bytes, [byte](($index % 251) + 1))
        [IO.File]::WriteAllBytes((Join-Path $results ("group{0:D3}-a.bin" -f $index)), $bytes)
        [IO.File]::WriteAllBytes((Join-Path $results ("group{0:D3}-b.bin" -f $index)), $bytes)
    }

    foreach ($folder in @('original-set', 'renamed-set')) {
        $folderRoot = Join-Path $results "folders/$folder"
        [IO.Directory]::CreateDirectory((Join-Path $folderRoot 'nested')) | Out-Null
        [IO.File]::WriteAllText((Join-Path $folderRoot 'readme.txt'), 'exact folder smoke')
        [IO.File]::WriteAllBytes((Join-Path $folderRoot 'nested/data.bin'), [byte[]](1..64))
    }

    $longRoot = Join-Path $results 'long-path'
    while ($longRoot.Length -lt 280) {
        $longRoot = Join-Path $longRoot 'segment-0123456789abcdef'
    }
    [IO.Directory]::CreateDirectory($longRoot) | Out-Null
    [IO.File]::WriteAllText((Join-Path $longRoot 'long-a.txt'), 'long path smoke')
    [IO.File]::WriteAllText((Join-Path $longRoot 'long-b.txt'), 'long path smoke')

    $outside = Join-Path $Root 'junction-target'
    [IO.Directory]::CreateDirectory($outside) | Out-Null
    [IO.File]::WriteAllText((Join-Path $outside 'must-not-scan.txt'), 'reparse target')
    $junction = Join-Path $results 'junction-must-be-skipped'
    try {
        New-Item -ItemType Junction -Path $junction -Target $outside | Out-Null
    }
    catch {
        Write-Warning "Junction fixture could not be created: $($_.Exception.Message)"
    }

    for ($index = 0; $index -lt 1500; $index++) {
        $bytes = [byte[]]::new(1024 + ($index % 16))
        [Array]::Fill($bytes, [byte](($index % 251) + 1))
        [IO.File]::WriteAllBytes((Join-Path $cancel ("cancel-{0:D4}.bin" -f $index)), $bytes)
    }

    [pscustomobject]@{ Results = $results; Cancel = $cancel; Junction = $junction }
}

function Start-SmokeWorker {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $worker
    $start.WorkingDirectory = Split-Path $worker
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.Environment['SUPER_DUPER_DB_PATH'] = $database
    $start.Environment['HASH_CACHE_PATH'] = $cache
    $start.Environment['SUPER_DUPER_LOG'] = 'super_duper_core=info,super_duper_worker=info'
    $process = [Diagnostics.Process]::Start($start)
    if ($null -eq $process) { throw 'Windows did not start the smoke worker.' }
    [pscustomobject]@{
        Process = $process
        Stderr = $process.StandardError.ReadToEndAsync()
        NextId = 0
        Stopped = $false
    }
}

function Read-WorkerFrame($Connection, [int]$TimeoutSeconds = 30) {
    $read = $Connection.Process.StandardOutput.ReadLineAsync()
    if (-not $read.Wait([TimeSpan]::FromSeconds($TimeoutSeconds))) {
        throw 'Timed out waiting for a worker protocol frame.'
    }
    $line = $read.Result
    if ($null -eq $line) { throw 'Worker stdout closed unexpectedly.' }
    try { $frame = $line | ConvertFrom-Json -Depth 30 }
    catch { throw "Worker stdout was not protocol JSON: $line" }
    $allFrames.Add($frame)
    $frame
}

function Send-WorkerRequest($Connection, [string]$Method, $Parameters) {
    $Connection.NextId++
    $id = $Connection.NextId.ToString([Globalization.CultureInfo]::InvariantCulture)
    $request = @{ type = 'request'; id = $id; method = $Method; params = $Parameters } |
        ConvertTo-Json -Compress -Depth 30
    $Connection.Process.StandardInput.WriteLine($request)
    $Connection.Process.StandardInput.Flush()
    while ($true) {
        $frame = Read-WorkerFrame $Connection
        if ($frame.type -eq 'response' -and $frame.id -eq $id) {
            if (-not $frame.ok) {
                throw "$Method failed: $($frame.error.code): $($frame.error.message)"
            }
            return $frame.result
        }
    }
}

function Wait-RunTerminal($Connection, [long]$RunId, [int]$TimeoutSeconds = 120) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $frame = Read-WorkerFrame $Connection 30
        if ($frame.type -eq 'event' -and
            $frame.event -in @('run.completed', 'run.cancelled', 'run.failed') -and
            [long]$frame.data.run.id -eq $RunId) {
            return $frame.data.run
        }
    }
    throw "Timed out waiting for run $RunId to finish."
}

function Stop-SmokeWorker($Connection) {
    if ($Connection.Stopped) { return '' }
    $Connection.Process.StandardInput.Close()
    if (-not $Connection.Process.WaitForExit(10000)) {
        throw 'Worker did not finish graceful EOF shutdown within 10 seconds.'
    }
    $stderr = $Connection.Stderr.Result
    $exitCode = $Connection.Process.ExitCode
    $Connection.Process.Dispose()
    $Connection.Stopped = $true
    if ($exitCode -ne 0) {
        throw "Worker exited with code $exitCode during graceful shutdown."
    }
    $stderr
}

function Stop-SmokeWorkerForCleanup($Connection) {
    if ($null -eq $Connection -or $Connection.Stopped) { return }
    try {
        try { $Connection.Process.StandardInput.Close() } catch { }
        if (-not $Connection.Process.WaitForExit(2000)) {
            $Connection.Process.Kill($true)
            $Connection.Process.WaitForExit(5000) | Out-Null
        }
    }
    catch {
        Write-Warning "Unable to stop smoke worker $($Connection.Process.Id): $($_.Exception.Message)"
    }
    finally {
        $Connection.Process.Dispose()
        $Connection.Stopped = $true
    }
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw "Smoke assertion failed: $Message" }
}

function Invoke-WpfAutomation([long]$RunId) {
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    $knownWorkerIds = @(Get-Process -Name 'super-duper-worker' -ErrorAction SilentlyContinue | ForEach-Object Id)
    $ownedWorkerId = $null
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $app
    $start.WorkingDirectory = Split-Path $app
    $start.UseShellExecute = $false
    $start.Environment['SUPER_DUPER_WORKER_PATH'] = $worker
    $start.Environment['SUPER_DUPER_DB_PATH'] = $database
    $start.Environment['HASH_CACHE_PATH'] = $cache
    $process = [Diagnostics.Process]::Start($start)
    try {
        for ($attempt = 0; $attempt -lt 80 -and $process.MainWindowHandle -eq 0; $attempt++) {
            Start-Sleep -Milliseconds 250
            $process.Refresh()
        }
        Assert-True ($process.MainWindowHandle -ne 0) 'WPF main window did not appear.'
        $window = [Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)

        for ($attempt = 0; $attempt -lt 40 -and $null -eq $ownedWorkerId; $attempt++) {
            $owned = Get-Process -Name 'super-duper-worker' -ErrorAction SilentlyContinue |
                Where-Object { $_.Id -notin $knownWorkerIds } |
                Select-Object -First 1
            if ($null -ne $owned) { $ownedWorkerId = $owned.Id; break }
            Start-Sleep -Milliseconds 250
        }
        Assert-True ($null -ne $ownedWorkerId) 'The WPF app did not start an owned worker process.'

        function Find-Element([string]$Property, [string]$Value, [int]$Attempts = 40) {
            $propertyId = if ($Property -eq 'AutomationId') {
                [Windows.Automation.AutomationElement]::AutomationIdProperty
            } else {
                [Windows.Automation.AutomationElement]::NameProperty
            }
            for ($attempt = 0; $attempt -lt $Attempts; $attempt++) {
                $condition = [Windows.Automation.PropertyCondition]::new($propertyId, $Value)
                $element = $window.FindFirst([Windows.Automation.TreeScope]::Descendants, $condition)
                if ($null -ne $element) { return $element }
                Start-Sleep -Milliseconds 250
            }
            throw "UI Automation element $Property=$Value was not found."
        }

        function Select-Element($Element) {
            $pattern = $Element.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)
            $pattern.Select()
            Start-Sleep -Milliseconds 400
        }

        function Invoke-Element($Element) {
            for ($attempt = 0; $attempt -lt 40 -and -not $Element.Current.IsEnabled; $attempt++) {
                Start-Sleep -Milliseconds 250
            }
            if (-not $Element.Current.IsEnabled) {
                throw "UI Automation element Name=$($Element.Current.Name) AutomationId=$($Element.Current.AutomationId) did not become enabled."
            }
            try {
                $pattern = $Element.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)
                $pattern.Invoke()
            }
            catch {
                throw "UI Automation invoke failed for Name=$($Element.Current.Name) AutomationId=$($Element.Current.AutomationId): $($_.Exception.Message)"
            }
            Start-Sleep -Milliseconds 400
        }


        function Find-FirstDataItem($Container, [int]$Attempts = 40) {
            for ($attempt = 0; $attempt -lt $Attempts; $attempt++) {
                $row = $Container.FindFirst(
                    [Windows.Automation.TreeScope]::Descendants,
                    [Windows.Automation.PropertyCondition]::new(
                        [Windows.Automation.AutomationElement]::ControlTypeProperty,
                        [Windows.Automation.ControlType]::DataItem))
                if ($null -ne $row) { return $row }
                Start-Sleep -Milliseconds 250
            }
            throw "UI Automation data row was not found in $($Container.Current.AutomationId)."
        }

        function Find-DescendantByName($Container, [string]$Name, [int]$Attempts = 40) {
            for ($attempt = 0; $attempt -lt $Attempts; $attempt++) {
                $element = $Container.FindFirst(
                    [Windows.Automation.TreeScope]::Descendants,
                    [Windows.Automation.PropertyCondition]::new(
                        [Windows.Automation.AutomationElement]::NameProperty,
                        $Name))
                if ($null -ne $element) { return $element }
                Start-Sleep -Milliseconds 250
            }
            throw "UI Automation descendant Name=$Name was not found in $($Container.Current.AutomationId)."
        }

        function Assert-NoVisibleDetailError([string]$AutomationId, [int]$Attempts = 20) {
            for ($attempt = 0; $attempt -lt $Attempts; $attempt++) {
                $condition = [Windows.Automation.PropertyCondition]::new(
                    [Windows.Automation.AutomationElement]::AutomationIdProperty,
                    $AutomationId)
                $errorElement = $window.FindFirst([Windows.Automation.TreeScope]::Descendants, $condition)
                if ($null -ne $errorElement -and
                    -not $errorElement.Current.IsOffscreen -and
                    -not [string]::IsNullOrWhiteSpace($errorElement.Current.Name)) {
                    throw "Explorer reveal reported an error: $($errorElement.Current.Name)"
                }
                Start-Sleep -Milliseconds 100
            }
        }

        Select-Element (Find-Element Name 'Milestone 6 Smoke')
        Select-Element (Find-Element AutomationId 'DuplicateFilesTab')
        Invoke-Element (Find-Element Name 'Group size')
        Invoke-Element (Find-Element Name 'Next')
        $fileGrid = Find-Element AutomationId 'FileGroupsGrid'
        $fileRow = Find-FirstDataItem $fileGrid
        Select-Element $fileRow
        $search = Find-Element AutomationId 'FileSearch'
        $search.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('group010')
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        $fileMembers = Find-Element AutomationId 'FileMembersGrid'
        $null = Find-FirstDataItem $fileMembers
        Invoke-Element (Find-DescendantByName $fileMembers 'Show in Explorer')
        Assert-NoVisibleDetailError 'FileDetailError'

        $search.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('long-a.txt')
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        $fileMembers = Find-Element AutomationId 'FileMembersGrid'
        $null = Find-FirstDataItem $fileMembers
        Invoke-Element (Find-DescendantByName $fileMembers 'Show in Explorer')
        Assert-NoVisibleDetailError 'FileDetailError'

        Select-Element (Find-Element AutomationId 'DuplicateFoldersTab')
        Invoke-Element (Find-Element Name 'Representative folder')
        $folderGrid = Find-Element AutomationId 'FolderGroupsGrid'
        $folderRow = Find-FirstDataItem $folderGrid
        Select-Element $folderRow
        $folderSearch = Find-Element AutomationId 'FolderSearch'
        $folderSearch.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('original-set')
        Invoke-Element (Find-Element AutomationId 'FolderApplyFilters')
        $folderMembers = Find-Element AutomationId 'FolderMembersGrid'
        $null = Find-FirstDataItem $folderMembers
        Invoke-Element (Find-DescendantByName $folderMembers 'Show in Explorer')
        Assert-NoVisibleDetailError 'FolderDetailError'
        Write-Output "WPF automation passed for restored run $RunId, including completed ordinary, long-path, and folder Explorer reveal commands."
    }
    finally {
        try {
            if (-not $process.HasExited) {
                $null = $process.CloseMainWindow()
                if (-not $process.WaitForExit(10000)) {
                    $process.Kill($true)
                    $process.WaitForExit(5000) | Out-Null
                    throw 'WPF app did not complete graceful shutdown within 10 seconds.'
                }
            }
            Assert-True ($process.ExitCode -eq 0) "WPF app exited with code $($process.ExitCode)."
            if ($null -ne $ownedWorkerId) {
                for ($attempt = 0; $attempt -lt 20 -and $null -ne (Get-Process -Id $ownedWorkerId -ErrorAction SilentlyContinue); $attempt++) {
                    Start-Sleep -Milliseconds 250
                }
                Assert-True ($null -eq (Get-Process -Id $ownedWorkerId -ErrorAction SilentlyContinue)) "Owned worker $ownedWorkerId survived WPF shutdown."
            }
        }
        finally {
            $process.Dispose()
        }
    }
}

function Assert-WpfCloseScenario(
    [string]$Name,
    [string]$DatabasePath,
    [string]$WorkerPath,
    [bool]$ExpectRecovery) {
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    $knownWorkerIds = @(Get-Process -Name 'super-duper-worker' -ErrorAction SilentlyContinue | ForEach-Object Id)
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $app
    $start.WorkingDirectory = Split-Path $app
    $start.UseShellExecute = $false
    $start.Environment['SUPER_DUPER_WORKER_PATH'] = $WorkerPath
    $start.Environment['SUPER_DUPER_DB_PATH'] = $DatabasePath
    $start.Environment['HASH_CACHE_PATH'] = Join-Path $smokeRoot ("close-cache-" + [guid]::NewGuid().ToString('N'))
    $process = [Diagnostics.Process]::Start($start)
    try {
        for ($attempt = 0; $attempt -lt 80 -and $process.MainWindowHandle -eq 0; $attempt++) {
            Start-Sleep -Milliseconds 250
            $process.Refresh()
        }
        Assert-True ($process.MainWindowHandle -ne 0) "$Name did not show a WPF window."
        if ($ExpectRecovery) {
            $window = [Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
            $foundRecovery = $false
            for ($attempt = 0; $attempt -lt 60 -and -not $foundRecovery; $attempt++) {
                $condition = [Windows.Automation.PropertyCondition]::new(
                    [Windows.Automation.AutomationElement]::AutomationIdProperty,
                    'RestartWorkerButton')
                $foundRecovery = $null -ne $window.FindFirst([Windows.Automation.TreeScope]::Descendants, $condition)
                if (-not $foundRecovery) { Start-Sleep -Milliseconds 250 }
            }
            Assert-True $foundRecovery "$Name did not reach its recovery screen."
        }

        $null = $process.CloseMainWindow()
        Assert-True ($process.WaitForExit(10000)) "$Name did not exit within 10 seconds."
        Assert-True ($process.ExitCode -eq 0) "$Name exited with code $($process.ExitCode)."

        for ($attempt = 0; $attempt -lt 20; $attempt++) {
            $survivors = @(Get-Process -Name 'super-duper-worker' -ErrorAction SilentlyContinue |
                Where-Object { $_.Id -notin $knownWorkerIds })
            if ($survivors.Count -eq 0) { break }
            Start-Sleep -Milliseconds 250
        }
        $survivors = @(Get-Process -Name 'super-duper-worker' -ErrorAction SilentlyContinue |
            Where-Object { $_.Id -notin $knownWorkerIds })
        Assert-True ($survivors.Count -eq 0) "$Name left an owned worker running."
        Write-Output "WPF shutdown passed: $Name"
    }
    finally {
        if (-not $process.HasExited) {
            $process.Kill($true)
            $process.WaitForExit(5000) | Out-Null
        }
        $process.Dispose()
    }
}

if (-not $SkipBuild) {
    Push-Location $repo
    try {
        if ($Configuration -eq 'Release') {
            Invoke-Checked { cargo build -p super-duper-worker --release } 'Release worker build'
        } else {
            Invoke-Checked { cargo build -p super-duper-worker } 'Debug worker build'
        }
        Invoke-Checked { dotnet build $solution --configuration $Configuration } '.NET Windows build'
    }
    finally { Pop-Location }
}
if (-not (Test-Path -LiteralPath $worker -PathType Leaf)) { throw "Worker not found: $worker" }
if (-not $SkipWpf -and -not (Test-Path -LiteralPath $app -PathType Leaf)) { throw "App not found: $app" }

[IO.Directory]::CreateDirectory($smokeRoot) | Out-Null
$fixture = New-SmokeFixture $smokeRoot
$lockedPath = Join-Path $fixture.Results 'locked-during-scan.bin'
[IO.File]::WriteAllText($lockedPath, 'locked access warning')
$exclusive = [IO.File]::Open($lockedPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None)

try {
    $connection = Start-SmokeWorker
    $hello = Send-WorkerRequest $connection 'hello' @{
        protocolVersions = @(1)
        client = @{ name = 'windows-smoke'; version = '1.0.0' }
    }
    Assert-True ($hello.protocolVersion -eq 1) 'Protocol V1 negotiation failed.'

    $cancelSession = (Send-WorkerRequest $connection 'session.create' @{
        name = 'Milestone 6 Cancellation'
        roots = @($fixture.Cancel)
        ignorePatterns = @()
    }).session
    $cancelRun = (Send-WorkerRequest $connection 'run.start' @{ sessionId = $cancelSession.id }).run
    $cancelling = (Send-WorkerRequest $connection 'run.cancel' @{ runId = $cancelRun.id }).run
    Assert-True ($cancelling.status -eq 'cancelling') 'Cancel did not enter cancelling state.'
    $cancelled = Wait-RunTerminal $connection $cancelRun.id
    Assert-True ($cancelled.status -eq 'cancelled') 'Cancellation did not become durable.'

    $roots = @($fixture.Results) + $AdditionalRoot
    $session = (Send-WorkerRequest $connection 'session.create' @{
        name = 'Milestone 6 Smoke'
        roots = $roots
        ignorePatterns = @('**/*.ignore')
    }).session
    $run = (Send-WorkerRequest $connection 'run.start' @{ sessionId = $session.id }).run
    $completed = Wait-RunTerminal $connection $run.id
    Assert-True ($completed.status -eq 'completed') 'Rerun did not complete.'
    Assert-True ($completed.warningCount -ge 1) 'Locked-file access warning was not counted.'
    $exclusive.Dispose()
    $exclusive = $null
    $scanDiagnostics = Stop-SmokeWorker $connection
    $connection = $null

    $restored = Start-SmokeWorker
    $null = Send-WorkerRequest $restored 'hello' @{
        protocolVersions = @(1)
        client = @{ name = 'windows-smoke-restart'; version = '1.0.0' }
    }
    $history = Send-WorkerRequest $restored 'run.list' @{ sessionId = $session.id; offset = 0; limit = 100 }
    Assert-True (($history.runs | Where-Object id -eq $run.id).status -eq 'completed') 'Completed run was not restored.'

    $filePage = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'groupSize'; direction = 'ascending' }
        filter = @{ search = ''; minimumSize = '0' }; cursor = $null
    }
    Assert-True ($filePage.total -ge 230) 'Duplicate-file fixture did not produce the expected groups.'
    Assert-True ($null -ne $filePage.nextCursor) 'Duplicate-file paging did not produce a next cursor.'
    $null = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'groupSize'; direction = 'ascending' }
        filter = @{ search = ''; minimumSize = '0' }; cursor = $filePage.nextCursor
    }
    $filteredFiles = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'representativeName'; direction = 'ascending' }
        filter = @{ search = 'group010'; minimumSize = '0' }; cursor = $null
    }
    Assert-True ($filteredFiles.total -ge 1) 'Duplicate-file filtering returned no smoke result.'
    $null = Send-WorkerRequest $restored 'duplicate_file_group.members' @{
        runId = $run.id; groupId = $filteredFiles.groups[0].id; pageSize = 25
        sort = @{ field = 'path'; direction = 'ascending' }
        filter = @{ search = '' }; cursor = $null
    }

    $folderPage = Send-WorkerRequest $restored 'duplicate_folder_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'representativePath'; direction = 'ascending' }
        filter = @{ search = 'original-set'; minimumSize = '0' }; cursor = $null
    }
    Assert-True ($folderPage.total -ge 1) 'Exact-folder filtering returned no smoke result.'
    $folderMembers = Send-WorkerRequest $restored 'duplicate_folder_group.members' @{
        runId = $run.id; groupId = $folderPage.groups[0].id; pageSize = 25
        sort = @{ field = 'path'; direction = 'ascending' }
        filter = @{ search = '' }; cursor = $null
    }
    Assert-True ($folderMembers.total -ge 2) 'Exact-folder member browsing did not return both roots.'
    $queryDiagnostics = Stop-SmokeWorker $restored
    $restored = $null

    foreach ($phase in @('discovering', 'hashing', 'persisting', 'analyzing_folders', 'finalizing')) {
        Assert-True ($scanDiagnostics.Contains("kind=scan_phase run_id=$($run.id) phase=$phase")) "Missing $phase timing."
    }
    foreach ($method in @(
        'duplicate_file_group.page', 'duplicate_file_group.members',
        'duplicate_folder_group.page', 'duplicate_folder_group.members')) {
        Assert-True ($queryDiagnostics.Contains("kind=result_query method=$method")) "Missing $method timing."
    }

    if (-not $SkipWpf) {
        Invoke-WpfAutomation $run.id
        $idleDatabase = Join-Path $smokeRoot 'idle-close.db'
        Assert-WpfCloseScenario 'idle connected close 1' $idleDatabase $worker $false
        Assert-WpfCloseScenario 'idle connected close 2' $idleDatabase $worker $false
        Assert-WpfCloseScenario 'worker startup failure close' (Join-Path $smokeRoot 'startup-failure.db') (Join-Path $smokeRoot 'missing-worker.exe') $true
        $databaseFailurePath = Join-Path $smokeRoot 'database-path-is-a-directory'
        [IO.Directory]::CreateDirectory($databaseFailurePath) | Out-Null
        Assert-WpfCloseScenario 'database failure close' $databaseFailurePath $worker $true
    }
    Write-Output "Windows smoke passed. Fixture: $smokeRoot"
}
finally {
    Stop-SmokeWorkerForCleanup $connection
    Stop-SmokeWorkerForCleanup $restored
    if ($null -ne $exclusive) { $exclusive.Dispose() }
    if (-not $KeepArtifacts -and (Test-Path -LiteralPath $smokeRoot)) {
        $resolved = (Resolve-Path -LiteralPath $smokeRoot).Path
        $expectedPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
        if (-not $resolved.StartsWith($expectedPrefix, [StringComparison]::OrdinalIgnoreCase) -or
            -not ([IO.Path]::GetFileName($resolved)).StartsWith('super-duper-windows-smoke-', [StringComparison]::Ordinal)) {
            throw "Refusing to clean unexpected smoke path: $resolved"
        }
        if ($null -ne $fixture -and (Test-Path -LiteralPath $fixture.Junction)) {
            Remove-Item -LiteralPath $fixture.Junction -Force
        }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
