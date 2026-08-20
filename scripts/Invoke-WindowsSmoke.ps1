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
        if ($index -eq 10) {
            [IO.File]::WriteAllBytes((Join-Path $results ("group{0:D3}-c.JPG" -f $index)), $bytes)
        }
    }
    [IO.File]::WriteAllText((Join-Path $results 'no-extension-a'), 'no extension smoke')
    [IO.File]::WriteAllText((Join-Path $results 'no-extension-b'), 'no extension smoke')

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

function Wait-PreflightTerminal($Connection, [long]$PreflightId, [int]$TimeoutSeconds = 120) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $result = Send-WorkerRequest $Connection 'preflight.get' @{ preflightId = $PreflightId }
        $preflight = $result.preflight
        if ($preflight.status -in @('completed', 'cancelled', 'interrupted', 'failed')) {
            return $preflight
        }
        Start-Sleep -Milliseconds 100
    }
    throw "Timed out waiting for preflight $PreflightId to finish."
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
    Add-Type -AssemblyName System.Windows.Forms
    if ($null -eq ('SmokeMouseInput' -as [type])) {
        Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SmokeMouseInput
{
    [DllImport("user32.dll")]
    public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")]
    public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extraInfo);
}
'@
    }
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
    $automationFailure = $null
    $preflightInvoker = $null
    $preflightInvokeAsync = $null
    try {
        for ($attempt = 0; $attempt -lt 80 -and $process.MainWindowHandle -eq 0; $attempt++) {
            Start-Sleep -Milliseconds 250
            $process.Refresh()
        }
        Assert-True ($process.MainWindowHandle -ne 0) 'WPF main window did not appear.'
        $window = [Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
        $automationShell = New-Object -ComObject WScript.Shell
        function Activate-SmokeWindow {
            for ($attempt = 0; $attempt -lt 20; $attempt++) {
                if ($automationShell.AppActivate($process.Id)) {
                    Start-Sleep -Milliseconds 100
                    return
                }
                Start-Sleep -Milliseconds 100
            }
            throw "The disposable WPF process $($process.Id) could not be activated for keyboard automation."
        }
        Activate-SmokeWindow

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

        function Get-AutomationCount($Element) {
            $digits = $Element.Current.Name -replace '[^0-9]', ''
            if ([string]::IsNullOrEmpty($digits)) {
                throw "UI Automation element AutomationId=$($Element.Current.AutomationId) did not expose a count in its accessible name."
            }
            return [long]::Parse($digits, [Globalization.CultureInfo]::InvariantCulture)
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

        function Find-FirstListItem($Container, [int]$Attempts = 40) {
            for ($attempt = 0; $attempt -lt $Attempts; $attempt++) {
                $item = $Container.FindFirst(
                    [Windows.Automation.TreeScope]::Descendants,
                    [Windows.Automation.PropertyCondition]::new(
                        [Windows.Automation.AutomationElement]::ControlTypeProperty,
                        [Windows.Automation.ControlType]::ListItem))
                if ($null -ne $item) { return $item }
                Start-Sleep -Milliseconds 250
            }
            throw "UI Automation list item was not found in $($Container.Current.AutomationId)."
        }

        function Find-FacetOption($Combo, [string]$AllOptionName) {
            $container = $Combo.GetCurrentPattern([Windows.Automation.ItemContainerPattern]::Pattern)
            $item = $null
            $names = [Collections.Generic.List[string]]::new()
            for ($index = 0; $index -lt 512; $index++) {
                $item = $container.FindItemByProperty($item, $null, $null)
                if ($null -eq $item) { break }
                $names.Add($item.Current.Name)
                if ($item.Current.Name -ne $AllOptionName -and
                    $item.Current.Name.Contains('set', [StringComparison]::OrdinalIgnoreCase)) {
                    return $item
                }
            }
            throw "Facet $($Combo.Current.AutomationId) did not expose a bounded worker-owned value and count. Items: $($names -join ' | ')"
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

        function Find-DescendantButtonByNameFragment($Container, [string]$Fragment, [int]$Attempts = 40) {
            for ($attempt = 0; $attempt -lt $Attempts; $attempt++) {
                $buttons = $Container.FindAll(
                    [Windows.Automation.TreeScope]::Descendants,
                    [Windows.Automation.PropertyCondition]::new(
                        [Windows.Automation.AutomationElement]::ControlTypeProperty,
                        [Windows.Automation.ControlType]::Button))
                $button = $buttons | Where-Object {
                    $_.Current.Name.Contains($Fragment, [StringComparison]::OrdinalIgnoreCase)
                } | Select-Object -First 1
                if ($null -ne $button) { return $button }
                Start-Sleep -Milliseconds 250
            }
            throw "UI Automation Button containing Name=$Fragment was not found in $($Container.Current.AutomationId)."
        }

        function Find-DescendantByHelpTextPrefix($Container, [string]$Prefix, [int]$Attempts = 40) {
            for ($attempt = 0; $attempt -lt $Attempts; $attempt++) {
                $matches = $Container.FindAll(
                    [Windows.Automation.TreeScope]::Descendants,
                    [Windows.Automation.Condition]::TrueCondition)
                $match = $matches | Where-Object {
                    $_.Current.HelpText.StartsWith($Prefix, [StringComparison]::Ordinal)
                } | Select-Object -First 1
                if ($null -ne $match) { return $match }
                Start-Sleep -Milliseconds 250
            }
            throw "UI Automation descendant HelpText prefix=$Prefix was not found in $($Container.Current.AutomationId)."
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

        function Test-IsAutomationDescendant($Ancestor, $Element) {
            $current = $Element
            while ($null -ne $current) {
                if ([Windows.Automation.Automation]::Compare($Ancestor, $current)) { return $true }
                try {
                    $current = [Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($current)
                }
                catch {
                    return $false
                }
            }
            return $false
        }

        Select-Element (Find-Element Name 'Milestone 6 Smoke')
        Select-Element (Find-Element AutomationId 'SetupTab')
        $null = Find-Element AutomationId 'CloudPolicyName'
        $null = Find-Element AutomationId 'CloudPolicyDescription'
        $null = Find-Element AutomationId 'ManualCloudLocationExclusions'
        $cloudStatus = Find-Element AutomationId 'CloudDetectionStatus'
        $refreshCloud = Find-Element AutomationId 'RefreshCloudLocations'
        Invoke-Element $refreshCloud
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            $cloudStatus = Find-Element AutomationId 'CloudDetectionStatus' 1
            $refreshCloud = Find-Element AutomationId 'RefreshCloudLocations' 1
            if ($refreshCloud.Current.IsEnabled -and
                -not $cloudStatus.Current.Name.Contains('Checking', [StringComparison]::OrdinalIgnoreCase)) {
                break
            }
            Start-Sleep -Milliseconds 250
        }
        Assert-True ($refreshCloud.Current.IsEnabled) 'Cloud registration refresh did not complete responsively.'
        Assert-True (-not $cloudStatus.Current.Name.Contains('unavailable', [StringComparison]::OrdinalIgnoreCase)) 'Cloud registration discovery remained unavailable in the normal WPF smoke.'
        Assert-True ((Find-Element AutomationId 'StartScanButton').Current.IsEnabled) 'Start scan did not become enabled after successful cloud registration discovery.'
        Select-Element (Find-Element AutomationId 'DuplicateFilesTab')
        $oneGigabyteOrLarger = Find-Element AutomationId 'FileOneGigabyteOrLarger'
        Assert-True ($oneGigabyteOrLarger.Current.Name -eq 'Show only duplicate sets whose one-copy size is at least 1 GB, 1,073,741,824 bytes') 'One-gigabyte size preset was not accessible.'
        $threeOrMoreCopies = Find-Element AutomationId 'FileThreeOrMoreCopies'
        Assert-True ($threeOrMoreCopies.Current.Name -eq 'Show only duplicate sets with three or more copies') 'Minimum-copy-count filter was not accessible.'
        $acrossDrives = Find-Element AutomationId 'FileAcrossDrives'
        Assert-True ($acrossDrives.Current.Name -eq 'Show only duplicate sets across multiple drives') 'Across-drives filter was not accessible.'
        $exactPathMatch = Find-Element AutomationId 'FileExactPathMatch'
        Assert-True ($exactPathMatch.Current.Name -eq 'Match the complete canonical member path') 'Exact-path filter was not accessible.'
        Assert-True ($exactPathMatch.Current.HelpText.Contains('Unicode case normalization', [StringComparison]::OrdinalIgnoreCase)) 'Exact-path filter did not explain its case normalization.'
        $extension = Find-Element AutomationId 'FileExtension'
        Assert-True ($extension.Current.Name -eq 'Filename extension without the dot') 'Extension filter was not accessible.'
        Assert-True ($extension.Current.HelpText.Contains('any immutable member', [StringComparison]::OrdinalIgnoreCase)) 'Extension filter did not explain its any-member semantics.'
        $withoutExtension = Find-Element AutomationId 'FileWithoutExtension'
        Assert-True ($withoutExtension.Current.Name.Contains('extension filter value', [StringComparison]::OrdinalIgnoreCase)) 'No-extension filter was not accessible.'
        Assert-True ($withoutExtension.Current.HelpText.Contains('terminal dot', [StringComparison]::OrdinalIgnoreCase)) 'No-extension filter did not explain terminal-dot handling.'
        $allExtensionsMatch = Find-Element AutomationId 'FileAllExtensionsMatch'
        Assert-True ($allExtensionsMatch.Current.Name.Contains('every copy', [StringComparison]::OrdinalIgnoreCase)) 'All-member extension filter was not accessible.'
        Assert-True ($allExtensionsMatch.Current.HelpText.Contains('distinct from file type', [StringComparison]::OrdinalIgnoreCase)) 'All-member extension filter did not distinguish extension from file type.'
        $rootFacet = Find-Element AutomationId 'FileSelectedRootFacet'
        Assert-True ($rootFacet.Current.Name.Contains('Selected root facet', [StringComparison]::OrdinalIgnoreCase)) 'Selected-root facet was not accessible.'
        $driveFacet = Find-Element AutomationId 'FileDriveFacet'
        Assert-True ($driveFacet.Current.Name.Contains('Drive facet', [StringComparison]::OrdinalIgnoreCase)) 'Drive facet was not accessible.'
        $null = Find-Element AutomationId 'FilePreviousDriveFacets'
        $null = Find-Element AutomationId 'FileNextDriveFacets'
        Invoke-Element (Find-Element AutomationId 'FileRootFacetNameSort')
        try {
            $rootFacet.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
        }
        catch {
            throw "Selected-root facet did not expose the ExpandCollapse pattern: $($_.Exception.Message)"
        }
        Start-Sleep -Milliseconds 250
        $rootOption = Find-FacetOption $rootFacet 'All selected roots'
        Assert-True $rootFacet.Current.IsKeyboardFocusable 'Selected-root facet was not keyboard focusable.'
        Activate-SmokeWindow
        $rootFacet.SetFocus()
        $rootOption.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select()
        Start-Sleep -Milliseconds 400
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        $selectedRootFilterText = Find-Element AutomationId 'FileSelectedRootFilterText'
        Assert-True ($selectedRootFilterText.Current.Name.Contains('Filtering sets represented under', [StringComparison]::OrdinalIgnoreCase)) 'Selected-root facet selection did not become active.'
        Invoke-Element (Find-Element AutomationId 'FileClearFilters')
        Invoke-Element (Find-Element AutomationId 'FileDriveFacetNameSort')
        $driveFacet = Find-Element AutomationId 'FileDriveFacet'
        try {
            $driveFacet.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
        }
        catch {
            throw "Drive facet did not expose the ExpandCollapse pattern: $($_.Exception.Message)"
        }
        Start-Sleep -Milliseconds 250
        $driveOption = Find-FacetOption $driveFacet 'All drives'
        Assert-True $driveFacet.Current.IsKeyboardFocusable 'Drive facet was not keyboard focusable.'
        Activate-SmokeWindow
        $driveFacet.SetFocus()
        $driveOption.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Select()
        Start-Sleep -Milliseconds 400
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        $selectedDriveFilterText = Find-Element AutomationId 'FileSelectedDriveFilterText'
        Assert-True ($selectedDriveFilterText.Current.Name.Contains('Filtering sets represented on', [StringComparison]::OrdinalIgnoreCase)) 'Drive facet selection did not become active.'
        Invoke-Element (Find-Element AutomationId 'FileClearFilters')
        $oneGigabyteToggle = $oneGigabyteOrLarger.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
        if ($oneGigabyteToggle.Current.ToggleState -ne [Windows.Automation.ToggleState]::Off) {
            $oneGigabyteToggle.Toggle()
        }
        $oneGigabyteToggle.Toggle()
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            if ((Find-Element AutomationId 'FileApplyFilters' 1).Current.IsEnabled) { break }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ((Find-Element AutomationId 'FileApplyFilters').Current.IsEnabled) 'One-gigabyte size preset did not complete responsively.'
        Assert-True ((Get-AutomationCount (Find-Element AutomationId 'FileSummaryMatchingSets')) -eq 0) 'One-gigabyte size preset did not exclude the small smoke sets.'
        $oneGigabyteToggle.Toggle()
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            if ((Find-Element AutomationId 'FileApplyFilters' 1).Current.IsEnabled) { break }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ((Get-AutomationCount (Find-Element AutomationId 'FileSummaryMatchingSets')) -gt 0) 'Clearing the one-gigabyte size preset did not restore the smoke sets.'
        $threeOrMoreCopiesToggle = $threeOrMoreCopies.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
        if ($threeOrMoreCopiesToggle.Current.ToggleState -ne [Windows.Automation.ToggleState]::Off) {
            $threeOrMoreCopiesToggle.Toggle()
        }
        $threeOrMoreCopiesToggle.Toggle()
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            if ((Find-Element AutomationId 'FileApplyFilters' 1).Current.IsEnabled) { break }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ((Find-Element AutomationId 'FileApplyFilters').Current.IsEnabled) 'Minimum-copy-count filter did not complete responsively.'
        $minimumCopySummary = Find-Element AutomationId 'FileSummaryMatchingCopies'
        Assert-True ((Get-AutomationCount $minimumCopySummary) -ge 3) 'Minimum-copy-count filter did not return a three-copy set.'
        $threeOrMoreCopiesToggle.Toggle()
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            if ((Find-Element AutomationId 'FileApplyFilters' 1).Current.IsEnabled) { break }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ((Find-Element AutomationId 'FileApplyFilters').Current.IsEnabled) 'Clearing the minimum-copy-count filter did not complete responsively.'
        $acrossDrivesToggle = $acrossDrives.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
        if ($acrossDrivesToggle.Current.ToggleState -ne [Windows.Automation.ToggleState]::Off) {
            $acrossDrivesToggle.Toggle()
        }
        $acrossDrivesToggle.Toggle()
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            if ((Find-Element AutomationId 'FileApplyFilters' 1).Current.IsEnabled) { break }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ((Find-Element AutomationId 'FileApplyFilters').Current.IsEnabled) 'Across-drives filter did not complete responsively.'
        $acrossDrivesToggle.Toggle()
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            if ((Find-Element AutomationId 'FileApplyFilters' 1).Current.IsEnabled) { break }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ((Find-Element AutomationId 'FileApplyFilters').Current.IsEnabled) 'Clearing the across-drives filter did not complete responsively.'
        $extension.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('JPG')
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            if ((Find-Element AutomationId 'FileApplyFilters' 1).Current.IsEnabled) { break }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ((Get-AutomationCount (Find-Element AutomationId 'FileSummaryMatchingSets')) -eq 1) 'Any-member extension filtering did not isolate the mixed-extension smoke set.'
        $allExtensionsMatchToggle = $allExtensionsMatch.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
        if ($allExtensionsMatchToggle.Current.ToggleState -ne [Windows.Automation.ToggleState]::Off) {
            $allExtensionsMatchToggle.Toggle()
        }
        $allExtensionsMatchToggle.Toggle()
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            if ((Find-Element AutomationId 'FileApplyFilters' 1).Current.IsEnabled) { break }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ((Get-AutomationCount (Find-Element AutomationId 'FileSummaryMatchingSets')) -eq 0) 'All-member extension filtering did not exclude the mixed-extension smoke set.'
        $extension.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('')
        $withoutExtensionToggle = $withoutExtension.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
        if ($withoutExtensionToggle.Current.ToggleState -ne [Windows.Automation.ToggleState]::Off) {
            $withoutExtensionToggle.Toggle()
        }
        $withoutExtensionToggle.Toggle()
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            if ((Find-Element AutomationId 'FileApplyFilters' 1).Current.IsEnabled) { break }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ((Get-AutomationCount (Find-Element AutomationId 'FileSummaryMatchingSets')) -eq 1) 'All-member no-extension filtering did not isolate the extensionless smoke set.'
        Invoke-Element (Find-Element AutomationId 'FileClearFilters')
        Invoke-Element (Find-Element Name 'Group size')
        Invoke-Element (Find-Element AutomationId 'FileNextGroupPage')
        $fileGrid = Find-Element AutomationId 'FileGroupsGrid'
        $fileRow = Find-FirstDataItem $fileGrid
        Select-Element $fileRow
        $selectedSetName = Find-Element AutomationId 'FileSelectedSetName'
        $beforeNextSet = $selectedSetName.Current.Name
        $previousSet = Find-Element AutomationId 'FilePreviousSet'
        $nextSet = Find-Element AutomationId 'FileNextSet'
        Assert-True ($previousSet.Current.Name.Contains('focus returns', [StringComparison]::OrdinalIgnoreCase)) 'Previous-set focus behavior was not accessible.'
        Assert-True ($nextSet.Current.Name.Contains('focus returns', [StringComparison]::OrdinalIgnoreCase)) 'Next-set focus behavior was not accessible.'
        Activate-SmokeWindow
        Invoke-Element $nextSet
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            $selectedSetName = Find-Element AutomationId 'FileSelectedSetName' 1
            $focused = [Windows.Automation.AutomationElement]::FocusedElement
            if ($selectedSetName.Current.Name -ne $beforeNextSet -and
                (Test-IsAutomationDescendant $fileGrid $focused)) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ($selectedSetName.Current.Name -ne $beforeNextSet) 'Next set did not advance the selected duplicate set.'
        $focusedAfterNextSet = [Windows.Automation.AutomationElement]::FocusedElement
        Assert-True (Test-IsAutomationDescendant $fileGrid $focusedAfterNextSet) "Next set did not restore keyboard focus to the selected group row. Focus remained on Name=$($focusedAfterNextSet.Current.Name) AutomationId=$($focusedAfterNextSet.Current.AutomationId) ControlType=$($focusedAfterNextSet.Current.ControlType.ProgrammaticName)."
        Activate-SmokeWindow
        Invoke-Element $previousSet
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            $selectedSetName = Find-Element AutomationId 'FileSelectedSetName' 1
            $focused = [Windows.Automation.AutomationElement]::FocusedElement
            if ($selectedSetName.Current.Name -eq $beforeNextSet -and
                (Test-IsAutomationDescendant $fileGrid $focused)) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ($selectedSetName.Current.Name -eq $beforeNextSet) 'Previous set did not restore the prior duplicate set.'
        Assert-True (Test-IsAutomationDescendant $fileGrid ([Windows.Automation.AutomationElement]::FocusedElement)) 'Previous set did not restore keyboard focus to the selected group row.'
        $search = Find-Element AutomationId 'FileSearch'
        $search.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('group010')
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        $fileMembers = Find-Element AutomationId 'FileMembersGrid'
        $memberRow = Find-FirstDataItem $fileMembers
        $summarySets = Find-Element AutomationId 'FileSummaryMatchingSets'
        $summaryRecoverable = Find-Element AutomationId 'FileSummaryRecoverable'
        $locationSummary = Find-Element AutomationId 'FileLocationSummaryText'
        $selectedSetExplanation = Find-Element AutomationId 'FileSelectedSetExplanation'
        $selectedSetLocations = Find-Element AutomationId 'FileSelectedSetLocations'
        Assert-True ((Get-AutomationCount $summarySets) -ge 1) 'Filtered review summary did not expose matching sets.'
        Assert-True (-not [string]::IsNullOrWhiteSpace($summaryRecoverable.Current.Name)) 'Filtered review summary did not expose recoverable bytes.'
        Assert-True ($locationSummary.Current.Name.Contains('selected root', [StringComparison]::OrdinalIgnoreCase)) 'Filtered location summary did not expose selected-root coverage.'
        Assert-True ($locationSummary.Current.Name.Contains('drive', [StringComparison]::OrdinalIgnoreCase)) 'Filtered location summary did not expose drive coverage.'
        Assert-True ($selectedSetExplanation.Current.Name.Contains('not identify an original', [StringComparison]::OrdinalIgnoreCase)) 'Selected-set explanation was not accessible.'
        Assert-True ($selectedSetLocations.Current.Name.Contains('selected root', [StringComparison]::OrdinalIgnoreCase)) 'Selected-set location span was not accessible.'
        $pathCell = Find-DescendantByHelpTextPrefix $memberRow 'Complete path: '
        $exactPath = $pathCell.Current.HelpText.Substring('Complete path: '.Length)
        Invoke-Element (Find-DescendantButtonByNameFragment $fileMembers 'records intent only and does not delete')
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            $reviewPlanSummary = Find-Element AutomationId 'FileReviewPlanSummary' 1
            $selectedReviewSummary = Find-Element AutomationId 'FileSelectedSetReviewSummary' 1
            if ($reviewPlanSummary.Current.Name.Contains('1 remove', [StringComparison]::OrdinalIgnoreCase) -and
                $selectedReviewSummary.Current.Name.Contains('1 remove', [StringComparison]::OrdinalIgnoreCase)) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ($reviewPlanSummary.Current.Name.Contains('1 remove', [StringComparison]::OrdinalIgnoreCase)) 'The durable review-plan summary did not refresh after a Remove decision.'
        Assert-True ($selectedReviewSummary.Current.Name.Contains('1 remove', [StringComparison]::OrdinalIgnoreCase)) 'The selected-set review summary did not refresh after a Remove decision.'
        Assert-True ([IO.File]::Exists($exactPath)) 'Recording a review decision unexpectedly removed the disposable fixture file.'
        $preferenceExpander = Find-Element AutomationId 'PreferredRootPreviewExpander'
        $preferenceExpander.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
        $preferenceScope = Find-Element AutomationId 'PreferencePreviewScope'
        Assert-True $preferenceScope.Current.IsKeyboardFocusable 'The preferred-root scope selector was not keyboard focusable.'
        try {
            $preferenceScope.GetCurrentPattern(
                [Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
            Start-Sleep -Milliseconds 250
            $scopeItemContainer = $preferenceScope.GetCurrentPattern(
                [Windows.Automation.ItemContainerPattern]::Pattern)
            $completedRunScope = $null
            $scopeItem = $null
            $scopeItemNames = [Collections.Generic.List[string]]::new()
            for ($scopeIndex = 0; $scopeIndex -lt 10; $scopeIndex++) {
                $scopeItem = $scopeItemContainer.FindItemByProperty($scopeItem, $null, $null)
                if ($null -eq $scopeItem) { break }
                $scopeItemNames.Add($scopeItem.Current.Name)
                if ($scopeItem.Current.Name.Contains('DisplayName = Completed run', [StringComparison]::Ordinal) -or $scopeIndex -eq 2) {
                    $completedRunScope = $scopeItem
                }
            }
            Assert-True ($null -ne $completedRunScope) "The expanded preferred-root scope selector did not expose three options. Items: $($scopeItemNames -join ', ')"
            $completedRunScope.GetCurrentPattern(
                [Windows.Automation.SelectionItemPattern]::Pattern).Select()
            Start-Sleep -Milliseconds 400
        }
        catch {
            throw "Preferred-root scope selection did not expose its item-container contract: $($_.Exception.Message)"
        }
        $selectedPreferenceScope = $preferenceScope.GetCurrentPattern(
            [Windows.Automation.SelectionPattern]::Pattern).Current.GetSelection()
        $selectedPreferenceScopeNames = @($selectedPreferenceScope | ForEach-Object { $_.Current.Name }) -join ', '
        Assert-True ($selectedPreferenceScope.Count -eq 1 -and $selectedPreferenceScope[0].Current.Name.Contains('DisplayName = Completed run', [StringComparison]::Ordinal)) "Scope selection did not choose Completed run. Items: $($scopeItemNames -join ', '). Selected: $selectedPreferenceScopeNames"
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            $preferencePreviewButton = Find-Element AutomationId 'PreferenceRunPreview' 1
            if ($preferencePreviewButton.Current.IsEnabled) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ($preferencePreviewButton.Current.IsEnabled) 'The saved preferred-root rule did not become available to the WPF preview.'
        Invoke-Element $preferencePreviewButton
        for ($attempt = 0; $attempt -lt 60; $attempt++) {
            $preferenceStatus = Find-Element AutomationId 'PreferencePreviewStatus' 1
            if ($preferenceStatus.Current.Name.Contains('preview complete', [StringComparison]::OrdinalIgnoreCase)) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ($preferenceStatus.Current.Name.Contains('preview complete', [StringComparison]::OrdinalIgnoreCase)) 'The accessible preferred-root preview did not complete.'
        Assert-True ($preferenceStatus.Current.Name.Contains('nothing was applied or deleted', [StringComparison]::OrdinalIgnoreCase)) 'The preferred-root preview did not announce its non-applying boundary.'
        Assert-True ([IO.File]::Exists($exactPath)) 'The WPF preferred-root preview unexpectedly removed the disposable fixture file.'
        $preferenceApplyButton = Find-Element AutomationId 'PreferenceApplyRule'
        Assert-True ($preferenceApplyButton.Current.IsEnabled) 'The exact completed-run preferred-root preview did not enable review-state application.'
        Invoke-Element $preferenceApplyButton
        $applicationHeading = Find-Element AutomationId 'PreferenceApplicationConfirmationHeading'
        for ($attempt = 0; $attempt -lt 40 -and -not $applicationHeading.Current.HasKeyboardFocus; $attempt++) {
            Start-Sleep -Milliseconds 100
        }
        Assert-True $applicationHeading.Current.HasKeyboardFocus 'Opening rule application confirmation did not move keyboard focus to its heading.'
        Invoke-Element (Find-Element AutomationId 'PreferenceConfirmApplication')
        for ($attempt = 0; $attempt -lt 80; $attempt++) {
            $preferenceStatus = Find-Element AutomationId 'PreferencePreviewStatus' 1
            if ($preferenceStatus.Current.Name.Contains('Applied ', [StringComparison]::OrdinalIgnoreCase) -and
                $preferenceStatus.Current.Name.Contains('Nothing was deleted', [StringComparison]::OrdinalIgnoreCase)) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ($preferenceStatus.Current.Name.Contains('Applied ', [StringComparison]::OrdinalIgnoreCase)) 'The WPF preferred-root application did not announce completion.'
        Assert-True ($preferenceStatus.Current.Name.Contains('Nothing was deleted', [StringComparison]::OrdinalIgnoreCase)) 'The WPF preferred-root application did not announce its non-deleting boundary.'
        Assert-True ([IO.File]::Exists($exactPath)) 'The WPF preferred-root application unexpectedly removed the disposable fixture file.'
        $preferenceReverseButton = Find-Element AutomationId 'PreferenceReverseApplication'
        Assert-True ($preferenceReverseButton.Current.IsEnabled) 'The new preferred-root application was not available for isolated reversal.'
        Invoke-Element $preferenceReverseButton
        $reversalHeading = Find-Element AutomationId 'PreferenceReversalConfirmationHeading'
        for ($attempt = 0; $attempt -lt 40 -and -not $reversalHeading.Current.HasKeyboardFocus; $attempt++) {
            Start-Sleep -Milliseconds 100
        }
        Assert-True $reversalHeading.Current.HasKeyboardFocus 'Opening rule-application reversal confirmation did not move keyboard focus to its heading.'
        Invoke-Element (Find-Element AutomationId 'PreferenceConfirmReversal')
        for ($attempt = 0; $attempt -lt 80; $attempt++) {
            $preferenceStatus = Find-Element AutomationId 'PreferencePreviewStatus' 1
            if ($preferenceStatus.Current.Name.Contains('Reversed application', [StringComparison]::OrdinalIgnoreCase) -and
                $preferenceStatus.Current.Name.Contains('Manual choices were preserved', [StringComparison]::OrdinalIgnoreCase)) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ($preferenceStatus.Current.Name.Contains('Reversed application', [StringComparison]::OrdinalIgnoreCase)) 'The WPF preferred-root reversal did not announce completion.'
        Assert-True ($preferenceStatus.Current.Name.Contains('Manual choices were preserved', [StringComparison]::OrdinalIgnoreCase)) 'The WPF preferred-root reversal did not announce manual-choice preservation.'
        Assert-True ([IO.File]::Exists($exactPath)) 'The WPF preferred-root reversal unexpectedly removed the disposable fixture file.'
        Invoke-Element (Find-DescendantButtonByNameFragment $fileMembers 'in Explorer')
        Assert-NoVisibleDetailError 'FileDetailError'

        $exactPathToggle = $exactPathMatch.GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
        if ($exactPathToggle.Current.ToggleState -ne [Windows.Automation.ToggleState]::Off) {
            $exactPathToggle.Toggle()
        }
        $exactPathToggle.Toggle()
        $search.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($exactPath)
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        Assert-True ((Get-AutomationCount (Find-Element AutomationId 'FileSummaryMatchingSets')) -eq 1) 'Exact member-path filtering did not isolate one duplicate set.'
        $exactPathToggle.Toggle()

        $search.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('long-a.txt')
        Invoke-Element (Find-Element AutomationId 'FileApplyFilters')
        $fileMembers = Find-Element AutomationId 'FileMembersGrid'
        $null = Find-FirstDataItem $fileMembers
        Invoke-Element (Find-DescendantButtonByNameFragment $fileMembers 'in Explorer')
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
        $keepFolderButton = Find-DescendantButtonByNameFragment $folderMembers 'Keep folder copy '
        $keptFolderPath = $keepFolderButton.Current.Name.Substring('Keep folder copy '.Length)
        Invoke-Element $keepFolderButton
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            $folderReviewSummary = Find-Element AutomationId 'FolderSelectedReviewSummary' 1
            if ($folderReviewSummary.Current.Name.Contains('1 keep', [StringComparison]::OrdinalIgnoreCase)) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ($folderReviewSummary.Current.Name.Contains('1 keep', [StringComparison]::OrdinalIgnoreCase)) 'The durable exact-folder review summary did not refresh after a Keep decision.'
        Assert-True ([IO.Directory]::Exists($keptFolderPath)) 'Recording an exact-folder review decision unexpectedly removed the disposable fixture directory.'
        Invoke-Element (Find-DescendantByName $folderMembers 'Show in Explorer')
        Assert-NoVisibleDetailError 'FolderDetailError'

        Select-Element (Find-Element AutomationId 'PreflightTab')
        $preflightPlan = Find-Element AutomationId 'PreflightPlanSummary'
        Assert-True ($preflightPlan.Current.Name.Contains('logical removal', [StringComparison]::OrdinalIgnoreCase)) 'Preflight did not expose its reviewed-plan snapshot summary.'
        $startPreflight = Find-Element AutomationId 'StartPreflightButton'
        Assert-True ($startPreflight.Current.Name.Contains('no files will be deleted', [StringComparison]::OrdinalIgnoreCase)) 'Preflight start did not expose its non-deleting automation name.'
        Assert-True $startPreflight.Current.IsEnabled 'The current reviewed plan did not enable WPF preflight.'
        Activate-SmokeWindow
        $preflightInvoker = [PowerShell]::Create()
        $null = $preflightInvoker.AddScript(@'
param([long]$windowHandle)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$window = [Windows.Automation.AutomationElement]::FromHandle([IntPtr]::new($windowHandle))
$button = $window.FindFirst(
    [Windows.Automation.TreeScope]::Descendants,
    [Windows.Automation.PropertyCondition]::new(
        [Windows.Automation.AutomationElement]::AutomationIdProperty,
        'StartPreflightButton'))
if ($null -eq $button) { throw 'Background invoker could not find StartPreflightButton.' }
$button.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
'@).AddArgument([long]$process.MainWindowHandle)
        $preflightInvokeAsync = $preflightInvoker.BeginInvoke()
        $desktop = [Windows.Automation.AutomationElement]::RootElement
        $confirmation = $null
        for ($attempt = 0; $attempt -lt 40 -and $null -eq $confirmation; $attempt++) {
            $confirmation = $desktop.FindFirst(
                [Windows.Automation.TreeScope]::Descendants,
                [Windows.Automation.AndCondition]::new(
                    [Windows.Automation.PropertyCondition]::new(
                        [Windows.Automation.AutomationElement]::ProcessIdProperty,
                        $process.Id),
                    [Windows.Automation.PropertyCondition]::new(
                        [Windows.Automation.AutomationElement]::NameProperty,
                        'Run preflight validation?')))
            if ($null -eq $confirmation) { Start-Sleep -Milliseconds 100 }
        }
        if ($null -eq $confirmation) {
            $topLevelNames = @($desktop.FindAll(
                [Windows.Automation.TreeScope]::Children,
                [Windows.Automation.PropertyCondition]::new(
                    [Windows.Automation.AutomationElement]::ProcessIdProperty,
                    $process.Id)) | ForEach-Object { $_.Current.Name }) -join ', '
            if ($preflightInvokeAsync.IsCompleted) {
                try { $null = $preflightInvoker.EndInvoke($preflightInvokeAsync) }
                catch { throw "WPF preflight invocation failed before confirmation: $($_.Exception.Message). Top-level windows: $topLevelNames" }
            }
            throw "WPF preflight confirmation did not appear. Top-level windows: $topLevelNames"
        }
        $confirmationText = @($confirmation.Current.Name) + @($confirmation.FindAll(
            [Windows.Automation.TreeScope]::Descendants,
            [Windows.Automation.Condition]::TrueCondition) | ForEach-Object { $_.Current.Name }) -join ' '
        Assert-True ($confirmationText.Contains('complete file content', [StringComparison]::OrdinalIgnoreCase)) 'Preflight confirmation did not explain complete-content hashing.'
        Assert-True ($confirmationText.Contains('will not be opened', [StringComparison]::OrdinalIgnoreCase)) 'Preflight confirmation did not explain excluded and placeholder handling.'
        Assert-True ($confirmationText.Contains('No files will be deleted', [StringComparison]::OrdinalIgnoreCase)) 'Preflight confirmation did not explain its non-deleting boundary.'
        $yesLabel = $confirmation.FindFirst(
            [Windows.Automation.TreeScope]::Descendants,
            [Windows.Automation.PropertyCondition]::new(
                [Windows.Automation.AutomationElement]::NameProperty,
                'Yes'))
        Assert-True ($null -ne $yesLabel) 'Preflight confirmation did not expose a keyboard Yes action.'
        Activate-SmokeWindow
        [Windows.Forms.SendKeys]::SendWait('%Y')
        Assert-True ($preflightInvokeAsync.AsyncWaitHandle.WaitOne(10000)) 'The preflight start invocation did not return after confirmation.'
        $null = $preflightInvoker.EndInvoke($preflightInvokeAsync)
        $preflightInvoker.Dispose()
        $preflightInvoker = $null
        $preflightInvokeAsync = $null
        $preflightStatus = $null
        for ($attempt = 0; $attempt -lt 120; $attempt++) {
            $preflightStatus = Find-Element AutomationId 'PreflightStatusSummary' 1
            if ($preflightStatus.Current.Name.Contains('Completed', [StringComparison]::OrdinalIgnoreCase)) {
                break
            }
            Start-Sleep -Milliseconds 100
        }
        Assert-True ($preflightStatus.Current.Name.Contains('Completed', [StringComparison]::OrdinalIgnoreCase)) 'WPF preflight did not announce a completed summary.'
        $preflightHeading = Find-Element AutomationId 'PreflightSummaryHeading'
        for ($attempt = 0; $attempt -lt 40 -and -not $preflightHeading.Current.HasKeyboardFocus; $attempt++) {
            Start-Sleep -Milliseconds 100
        }
        Assert-True $preflightHeading.Current.HasKeyboardFocus 'Completed preflight did not move keyboard focus to its summary heading.'
        Assert-True ($preflightStatus.Current.Name.Contains('changed 0', [StringComparison]::OrdinalIgnoreCase) -and
            $preflightStatus.Current.Name.Contains('conflicts 0', [StringComparison]::OrdinalIgnoreCase)) 'Unchanged WPF preflight did not expose a clean structured summary.'
        Assert-True ($null -ne (Find-FirstListItem (Find-Element AutomationId 'PreflightItemsList'))) 'WPF preflight did not expose its bounded observation details.'
        Assert-True ([IO.File]::Exists($exactPath)) 'WPF preflight unexpectedly removed a disposable fixture file.'
        Write-Output "WPF automation passed for restored run $RunId, including durable non-deleting file Remove and exact-folder Keep review decisions, completed-run preferred-root preview/application/isolated reversal with confirmation focus and manual-choice preservation, bounded preflight confirmation/validation/summary focus with unchanged fixtures, exact member-path, any/all-member extension/no-extension, 1 GB-or-larger, and minimum-copy-count entry points, selected-root and drive facet filtering, next/previous-set focus restoration, and completed ordinary, long-path, and folder Explorer reveal commands."
    }
    catch {
        $automationFailure = $_
        throw
    }
    finally {
        try {
            if ($null -ne $preflightInvoker) {
                try { $preflightInvoker.Stop() } catch { }
                $preflightInvoker.Dispose()
                $preflightInvoker = $null
            }
            if (-not $process.HasExited) {
                $null = $process.CloseMainWindow()
                if (-not $process.WaitForExit(10000)) {
                    $process.Kill($true)
                    $process.WaitForExit(5000) | Out-Null
                    if ($null -eq $automationFailure) {
                        throw 'WPF app did not complete graceful shutdown within 10 seconds.'
                    }
                }
            }
            if ($null -eq $automationFailure) {
                Assert-True ($process.ExitCode -eq 0) "WPF app exited with code $($process.ExitCode)."
            }
            if ($null -ne $ownedWorkerId -and $null -eq $automationFailure) {
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

function Assert-WpfCloudFailClosedScenario([string]$DatabasePath) {
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    $knownWorkerIds = @(Get-Process -Name 'super-duper-worker' -ErrorAction SilentlyContinue | ForEach-Object Id)
    $ownedWorkerId = $null
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $app
    $start.WorkingDirectory = Split-Path $app
    $start.UseShellExecute = $false
    $start.Environment['SUPER_DUPER_WORKER_PATH'] = $worker
    $start.Environment['SUPER_DUPER_DB_PATH'] = $DatabasePath
    $start.Environment['HASH_CACHE_PATH'] = $cache
    $start.Environment['SUPER_DUPER_DISABLE_CLOUD_REGISTRATION_DISCOVERY'] = '1'
    $process = [Diagnostics.Process]::Start($start)
    try {
        for ($attempt = 0; $attempt -lt 80 -and $process.MainWindowHandle -eq 0; $attempt++) {
            Start-Sleep -Milliseconds 250
            $process.Refresh()
        }
        Assert-True ($process.MainWindowHandle -ne 0) 'Fail-closed WPF main window did not appear.'
        $window = [Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)

        for ($attempt = 0; $attempt -lt 40 -and $null -eq $ownedWorkerId; $attempt++) {
            $owned = Get-Process -Name 'super-duper-worker' -ErrorAction SilentlyContinue |
                Where-Object { $_.Id -notin $knownWorkerIds } |
                Select-Object -First 1
            if ($null -ne $owned) { $ownedWorkerId = $owned.Id; break }
            Start-Sleep -Milliseconds 250
        }
        Assert-True ($null -ne $ownedWorkerId) 'Fail-closed WPF scenario did not start an owned worker.'

        function Find-CloudElement([string]$Property, [string]$Value, [int]$Attempts = 40) {
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
            throw "Fail-closed UI Automation element $Property=$Value was not found."
        }

        function Select-CloudElement($Element) {
            $pattern = $Element.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)
            $pattern.Select()
            Start-Sleep -Milliseconds 400
        }

        Select-CloudElement (Find-CloudElement Name 'Milestone 6 Smoke')
        Select-CloudElement (Find-CloudElement AutomationId 'SetupTab')
        $null = Find-CloudElement AutomationId 'CloudPolicyDescription'
        $null = Find-CloudElement AutomationId 'ManualCloudLocationExclusions'
        $status = Find-CloudElement AutomationId 'CloudDetectionStatus'
        for ($attempt = 0; $attempt -lt 40 -and
            -not $status.Current.Name.Contains('unavailable', [StringComparison]::OrdinalIgnoreCase); $attempt++) {
            Start-Sleep -Milliseconds 250
            $status = Find-CloudElement AutomationId 'CloudDetectionStatus' 1
        }
        Assert-True ($status.Current.Name.Contains('unavailable', [StringComparison]::OrdinalIgnoreCase)) 'Fail-closed cloud detection status was not visible.'
        $startScan = Find-CloudElement AutomationId 'StartScanButton'
        Assert-True (-not $startScan.Current.IsEnabled) 'Start scan was enabled while cloud registration discovery was unavailable.'

        $refresh = Find-CloudElement AutomationId 'RefreshCloudLocations'
        $refresh.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
        for ($attempt = 0; $attempt -lt 40; $attempt++) {
            Start-Sleep -Milliseconds 250
            $refresh = Find-CloudElement AutomationId 'RefreshCloudLocations' 1
            $status = Find-CloudElement AutomationId 'CloudDetectionStatus' 1
            if ($refresh.Current.IsEnabled -and
                $status.Current.Name.Contains('unavailable', [StringComparison]::OrdinalIgnoreCase)) {
                break
            }
        }
        Assert-True ($refresh.Current.IsEnabled) 'Fail-closed cloud refresh did not complete responsively.'
        Assert-True (-not (Find-CloudElement AutomationId 'StartScanButton').Current.IsEnabled) 'Refresh incorrectly enabled Start scan while discovery remained unavailable.'
        Write-Output 'WPF cloud setup automation passed, including deterministic provider-unavailable fail-closed start behavior.'
    }
    finally {
        try {
            if (-not $process.HasExited) {
                $null = $process.CloseMainWindow()
                if (-not $process.WaitForExit(10000)) {
                    $process.Kill($true)
                    $process.WaitForExit(5000) | Out-Null
                    throw 'Fail-closed WPF app did not complete graceful shutdown within 10 seconds.'
                }
            }
            Assert-True ($process.ExitCode -eq 0) "Fail-closed WPF app exited with code $($process.ExitCode)."
            if ($null -ne $ownedWorkerId) {
                for ($attempt = 0; $attempt -lt 20 -and $null -ne (Get-Process -Id $ownedWorkerId -ErrorAction SilentlyContinue); $attempt++) {
                    Start-Sleep -Milliseconds 250
                }
                Assert-True ($null -eq (Get-Process -Id $ownedWorkerId -ErrorAction SilentlyContinue)) "Fail-closed owned worker $ownedWorkerId survived WPF shutdown."
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
        cloudPolicy = 'exclude_registered_roots'
        manualLocationExclusions = @()
        registeredCloudLocations = @()
        cloudDetectionStatus = 'complete'
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
        cloudPolicy = 'exclude_registered_roots'
        manualLocationExclusions = @()
        registeredCloudLocations = @()
        cloudDetectionStatus = 'complete'
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
    Assert-True ($filteredFiles.summary.matchingGroupCount -eq $filteredFiles.total) 'Filtered summary set count diverged from the result total.'
    Assert-True ($filteredFiles.summary.matchingCopyCount -ge 2) 'Filtered summary did not count duplicate copies.'
    Assert-True ([uint64]$filteredFiles.summary.potentialRecoverableBytes -gt 0) 'Filtered summary did not report recoverable bytes.'
    Assert-True ($filteredFiles.summary.distinctSelectedRootCount -eq 1) 'Filtered summary did not aggregate selected-root coverage.'
    Assert-True ($filteredFiles.summary.distinctDriveCount -eq 1) 'Filtered summary did not aggregate drive coverage.'
    Assert-True ($filteredFiles.summary.acrossDriveGroupCount -eq 0) 'Single-drive filtered summary reported a cross-drive set.'
    Assert-True ($filteredFiles.groups[0].distinctSelectedRootCount -eq 1) 'Duplicate-file group did not report its selected-root span.'
    Assert-True ($filteredFiles.groups[0].distinctDriveCount -eq 1) 'Duplicate-file group did not report its drive span.'
    $largeFileGroups = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'groupSize'; direction = 'ascending' }
        filter = @{ search = ''; minimumSize = '1200' }; cursor = $null
    }
    Assert-True ($largeFileGroups.total -gt 0 -and $largeFileGroups.total -lt $filePage.total) 'Minimum-size filter did not isolate a bounded subset of smoke sets.'
    Assert-True ($largeFileGroups.summary.matchingGroupCount -eq $largeFileGroups.total) 'Minimum-size summary diverged from its result total.'
    Assert-True (@($largeFileGroups.groups | Where-Object { [uint64]$_.groupSize -lt 1200 }).Count -eq 0) 'Minimum-size filter returned a set below its one-copy-size threshold.'
    $largeRootFacets = Send-WorkerRequest $restored 'duplicate_file_selected_root_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ search = ''; minimumSize = '1200'; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($largeRootFacets.facets[0].matchingGroupCount -eq $largeFileGroups.total) 'Selected-root facet did not apply the minimum-size filter.'
    $largeDriveFacets = Send-WorkerRequest $restored 'duplicate_file_drive_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ search = ''; minimumSize = '1200'; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($largeDriveFacets.facets[0].matchingGroupCount -eq $largeFileGroups.total) 'Drive facet did not apply the minimum-size filter.'
    $selectedRootFacets = Send-WorkerRequest $restored 'duplicate_file_selected_root_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ search = 'group010'; minimumSize = '0'; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($selectedRootFacets.total -ge 1) 'Selected-root facet returned no worker-owned values.'
    Assert-True ($selectedRootFacets.facets[0].matchingGroupCount -ge 1) 'Selected-root facet did not report a matching-set count.'
    $rootFilteredFiles = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'representativeName'; direction = 'ascending' }
        filter = @{
            search = 'group010'; minimumSize = '0'; acrossDrives = $false
            selectedRoot = $selectedRootFacets.facets[0].value
        }; cursor = $null
    }
    Assert-True ($rootFilteredFiles.total -ge 1) 'Selected-root filter returned no matching smoke result.'
    Assert-True ($rootFilteredFiles.summary.matchingGroupCount -eq $rootFilteredFiles.total) 'Selected-root filter summary diverged from its result total.'
    $driveFacets = Send-WorkerRequest $restored 'duplicate_file_drive_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{
            search = 'group010'; minimumSize = '0'; acrossDrives = $false
            selectedRoot = $selectedRootFacets.facets[0].value
        }; cursor = $null
    }
    Assert-True ($driveFacets.total -ge 1) 'Drive facet returned no worker-owned values.'
    Assert-True ($driveFacets.facets[0].matchingGroupCount -ge 1) 'Drive facet did not report a matching-set count.'
    $driveFilteredFiles = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'representativeName'; direction = 'ascending' }
        filter = @{
            search = 'group010'; minimumSize = '0'; acrossDrives = $false
            selectedDrive = $driveFacets.facets[0].value
        }; cursor = $null
    }
    Assert-True ($driveFilteredFiles.total -ge 1) 'Drive filter returned no matching smoke result.'
    Assert-True ($driveFilteredFiles.summary.matchingGroupCount -eq $driveFilteredFiles.total) 'Drive filter summary diverged from its result total.'
    $driveScopedRootFacets = Send-WorkerRequest $restored 'duplicate_file_selected_root_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{
            search = 'group010'; minimumSize = '0'; acrossDrives = $false
            selectedDrive = $driveFacets.facets[0].value
        }; cursor = $null
    }
    Assert-True ($driveScopedRootFacets.total -ge 1) 'Selected-root facet did not compose with the drive filter.'
    $acrossDriveFiles = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'recoverableBytes'; direction = 'descending' }
        filter = @{ search = ''; minimumSize = '0'; acrossDrives = $true }; cursor = $null
    }
    Assert-True ($acrossDriveFiles.summary.matchingGroupCount -eq $acrossDriveFiles.total) 'Across-drives summary diverged from the filtered result total.'
    Assert-True ($acrossDriveFiles.summary.acrossDriveGroupCount -eq $acrossDriveFiles.total) 'Across-drives location summary diverged from the filtered result total.'
    Assert-True (@($acrossDriveFiles.groups | Where-Object { $_.distinctDriveCount -le 1 }).Count -eq 0) 'Across-drives filter returned a set confined to one drive.'
    $threeCopyFiles = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'copyCount'; direction = 'descending' }
        filter = @{ search = ''; minimumSize = '0'; minimumCopyCount = 3 }; cursor = $null
    }
    Assert-True ($threeCopyFiles.total -eq 1) 'Minimum-copy-count filter did not isolate the three-copy smoke set.'
    Assert-True ($threeCopyFiles.summary.matchingGroupCount -eq $threeCopyFiles.total) 'Minimum-copy-count summary diverged from its result total.'
    Assert-True ($threeCopyFiles.summary.matchingCopyCount -eq 3) 'Minimum-copy-count summary did not report the three matching copies.'
    Assert-True (@($threeCopyFiles.groups | Where-Object { $_.copyCount -lt 3 }).Count -eq 0) 'Minimum-copy-count filter returned a set below its threshold.'
    $threeCopyRootFacets = Send-WorkerRequest $restored 'duplicate_file_selected_root_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ search = ''; minimumSize = '0'; minimumCopyCount = 3; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($threeCopyRootFacets.facets[0].matchingGroupCount -eq 1) 'Selected-root facet did not apply the minimum-copy-count filter.'
    $threeCopyDriveFacets = Send-WorkerRequest $restored 'duplicate_file_drive_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ search = ''; minimumSize = '0'; minimumCopyCount = 3; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($threeCopyDriveFacets.facets[0].matchingGroupCount -eq 1) 'Drive facet did not apply the minimum-copy-count filter.'
    $extensionFiles = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'recoverableBytes'; direction = 'descending' }
        filter = @{ extension = 'JPG'; minimumSize = '0' }; cursor = $null
    }
    Assert-True ($extensionFiles.total -eq 1) 'Any-member extension filter did not isolate the mixed-extension smoke set.'
    Assert-True ($extensionFiles.summary.matchingGroupCount -eq 1) 'Extension-filtered summary diverged from its result total.'
    $extensionRootFacets = Send-WorkerRequest $restored 'duplicate_file_selected_root_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ extension = 'JPG'; minimumSize = '0'; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($extensionRootFacets.facets[0].matchingGroupCount -eq 1) 'Selected-root facet did not apply the any-member extension filter.'
    $extensionDriveFacets = Send-WorkerRequest $restored 'duplicate_file_drive_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ extension = 'JPG'; minimumSize = '0'; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($extensionDriveFacets.facets[0].matchingGroupCount -eq 1) 'Drive facet did not apply the any-member extension filter.'
    $allExtensionFiles = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'recoverableBytes'; direction = 'descending' }
        filter = @{ extension = 'JPG'; extensionMatch = 'all'; minimumSize = '0' }; cursor = $null
    }
    Assert-True ($allExtensionFiles.total -eq 0) 'All-member extension filter did not exclude the mixed-extension smoke set.'
    Assert-True ($allExtensionFiles.summary.matchingGroupCount -eq 0) 'All-member extension summary diverged from its result total.'
    $allExtensionRootFacets = Send-WorkerRequest $restored 'duplicate_file_selected_root_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ extension = 'JPG'; extensionMatch = 'all'; minimumSize = '0'; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($allExtensionRootFacets.total -eq 0) 'Selected-root facet did not apply the all-member extension filter.'
    $allExtensionDriveFacets = Send-WorkerRequest $restored 'duplicate_file_drive_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ extension = 'JPG'; extensionMatch = 'all'; minimumSize = '0'; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($allExtensionDriveFacets.total -eq 0) 'Drive facet did not apply the all-member extension filter.'
    $noExtensionFiles = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'recoverableBytes'; direction = 'descending' }
        filter = @{ extension = ''; minimumSize = '0' }; cursor = $null
    }
    Assert-True ($noExtensionFiles.total -eq 1) 'Explicit no-extension filter did not isolate the extensionless smoke set.'
    $allNoExtensionFiles = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'recoverableBytes'; direction = 'descending' }
        filter = @{ extension = ''; extensionMatch = 'all'; minimumSize = '0' }; cursor = $null
    }
    Assert-True ($allNoExtensionFiles.total -eq 1) 'All-member no-extension filter did not isolate the extensionless smoke set.'
    $fileMembers = Send-WorkerRequest $restored 'duplicate_file_group.members' @{
        runId = $run.id; groupId = $filteredFiles.groups[0].id; pageSize = 25
        sort = @{ field = 'path'; direction = 'ascending' }
        filter = @{ search = '' }; cursor = $null
    }
    Assert-True ($fileMembers.total -ge 2) 'Duplicate-file member browsing did not return the copies.'
    Assert-True (($fileMembers.members | Where-Object { -not [string]::IsNullOrWhiteSpace($_.rootPath) }).Count -eq $fileMembers.members.Count) 'Duplicate-file members did not include selected-root context.'
    Assert-True (($fileMembers.members | Where-Object { -not [string]::IsNullOrWhiteSpace($_.relativePath) }).Count -eq $fileMembers.members.Count) 'Duplicate-file members did not include relative-path context.'
    $exactMemberPath = $fileMembers.members[0].path.ToUpperInvariant()
    $exactPathFiles = Send-WorkerRequest $restored 'duplicate_file_group.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'recoverableBytes'; direction = 'descending' }
        filter = @{ search = $exactMemberPath; pathMatch = 'exact'; minimumSize = '0' }; cursor = $null
    }
    Assert-True ($exactPathFiles.total -eq 1) 'Exact member-path filter did not isolate one smoke set.'
    Assert-True ($exactPathFiles.summary.matchingGroupCount -eq 1) 'Exact member-path summary diverged from its result total.'
    $exactPathRootFacets = Send-WorkerRequest $restored 'duplicate_file_selected_root_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ search = $exactMemberPath; pathMatch = 'exact'; minimumSize = '0'; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($exactPathRootFacets.facets[0].matchingGroupCount -eq 1) 'Selected-root facet did not apply the exact member-path filter.'
    $exactPathDriveFacets = Send-WorkerRequest $restored 'duplicate_file_drive_facet.page' @{
        runId = $run.id; pageSize = 25
        sort = @{ field = 'matchingGroupCount'; direction = 'descending' }
        filter = @{ search = $exactMemberPath; pathMatch = 'exact'; minimumSize = '0'; acrossDrives = $false }; cursor = $null
    }
    Assert-True ($exactPathDriveFacets.facets[0].matchingGroupCount -eq 1) 'Drive facet did not apply the exact member-path filter.'

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
    $reviewPlan = Send-WorkerRequest $restored 'review_plan.get' @{ runId = $run.id }
    $folderReviewPage = Send-WorkerRequest $restored 'review_folder_group.page' @{
        runId = $run.id; pageSize = 25; cursor = $null
    }
    Assert-True ($folderReviewPage.total -ge 1) 'Bounded exact-folder review paging returned no smoke result.'
    $folderDecision = Send-WorkerRequest $restored 'review_folder_decision.set' @{
        operationId = [Guid]::NewGuid().ToString('N')
        runId = $run.id
        folderGroupId = $folderPage.groups[0].id
        folderMemberId = $folderMembers.members[0].id
        decision = 'keep'
        expectedRevision = $reviewPlan.plan.revision
    }
    Assert-True ($folderDecision.decision -eq 'keep') 'Exact-folder Keep review decision was not recorded.'
    Assert-True (-not $folderDecision.replayed) 'The first exact-folder smoke mutation was unexpectedly replayed.'
    $preferenceRule = Send-WorkerRequest $restored 'preference_rule.save' @{
        operationId = [Guid]::NewGuid().ToString('N')
        name = 'Smoke preferred root'
        roots = @($fileMembers.members[0].rootPath)
        expectedRevision = 0
    }
    Assert-True ($preferenceRule.rule.revision -eq 1) 'Preferred-root rule configuration was not saved.'
    $preferencePreview = Send-WorkerRequest $restored 'preference_rule.preview' @{
        runId = $run.id
        ruleId = $preferenceRule.rule.id
        ruleRevision = $preferenceRule.rule.revision
        reviewRevision = $folderDecision.appliedRevision
        pageSize = 25
        scope = @{ kind = 'completed_run' }
        cursor = $null
    }
    Assert-True ($preferencePreview.total -ge 1) 'Read-only preferred-root preview returned no affected smoke set.'
    Assert-True ($preferencePreview.summary.scopedGroupCount -ge $preferencePreview.total) 'Preferred-root preview summary diverged from its bounded rows.'
    Assert-True ([IO.File]::Exists($fileMembers.members[0].path)) 'Preferred-root preview unexpectedly removed a disposable fixture file.'
    $preferenceApplication = Send-WorkerRequest $restored 'preference_rule.apply' @{
        operationId = [Guid]::NewGuid().ToString('N')
        runId = $run.id
        ruleId = $preferenceRule.rule.id
        ruleRevision = $preferenceRule.rule.revision
        sourceReviewRevision = $folderDecision.appliedRevision
        previewSignature = $preferencePreview.previewSignature
        scope = @{ kind = 'completed_run' }
    }
    Assert-True (-not $preferenceApplication.replayed) 'The first preferred-root rule application was unexpectedly replayed.'
    Assert-True ($preferenceApplication.application.summary.ruleKeepPathCount -ge 1) 'Preferred-root rule application recorded no rule Keep decisions.'
    $previewPageForProvenance = $preferencePreview
    $applicablePreviewGroup = $null
    while ($null -eq $applicablePreviewGroup) {
        $applicablePreviewGroup = $previewPageForProvenance.groups |
            Where-Object { $_.status -eq 'applicable' } |
            Select-Object -First 1
        if ($null -ne $applicablePreviewGroup -or $null -eq $previewPageForProvenance.nextCursor) {
            break
        }
        $previewPageForProvenance = Send-WorkerRequest $restored 'preference_rule.preview' @{
            runId = $run.id
            ruleId = $preferenceRule.rule.id
            ruleRevision = $preferenceRule.rule.revision
            reviewRevision = $folderDecision.appliedRevision
            pageSize = 25
            scope = @{ kind = 'completed_run' }
            cursor = $previewPageForProvenance.nextCursor
        }
    }
    Assert-True ($null -ne $applicablePreviewGroup) 'The bounded preferred-root preview contained no applicable set for provenance verification.'
    $appliedRuleMembers = Send-WorkerRequest $restored 'duplicate_file_group.members' @{
        runId = $run.id; groupId = $applicablePreviewGroup.groupId; pageSize = 25
        sort = @{ field = 'path'; direction = 'ascending' }
        filter = @{ search = '' }; cursor = $null
    }
    $ruleOwnedMember = $appliedRuleMembers.members |
        Where-Object { $_.decisionApplicationId -eq $preferenceApplication.application.id } |
        Select-Object -First 1
    Assert-True ($null -ne $ruleOwnedMember) 'Applied preferred-root rule provenance was not visible on a member row.'
    $manualRuleOverride = Send-WorkerRequest $restored 'review_decision.set' @{
        operationId = [Guid]::NewGuid().ToString('N')
        runId = $run.id
        groupId = $ruleOwnedMember.groupId
        fileId = $ruleOwnedMember.id
        decision = 'undecided'
        expectedRevision = $preferenceApplication.application.appliedRevision
    }
    Assert-True ($manualRuleOverride.decision -eq 'undecided') 'A later manual Undecided did not override the rule-produced decision.'
    Assert-True ([IO.File]::Exists($ruleOwnedMember.path)) 'Rule application or manual override unexpectedly removed a disposable fixture file.'
    $firstQueryDiagnostics = Stop-SmokeWorker $restored
    $restored = $null

    $restored = Start-SmokeWorker
    $null = Send-WorkerRequest $restored 'hello' @{
        protocolVersions = @(1)
        client = @{ name = 'windows-smoke-review-restart'; version = '1.0.0' }
    }
    $persistedFolderMembers = Send-WorkerRequest $restored 'duplicate_folder_group.members' @{
        runId = $run.id; groupId = $folderPage.groups[0].id; pageSize = 25
        sort = @{ field = 'path'; direction = 'ascending' }
        filter = @{ search = '' }; cursor = $null
    }
    $persistedFolderMember = $persistedFolderMembers.members |
        Where-Object { $_.id -eq $folderMembers.members[0].id } |
        Select-Object -First 1
    Assert-True ($persistedFolderMember.decision -eq 'keep') 'Exact-folder review decision did not survive a worker restart.'
    Assert-True ([IO.Directory]::Exists($persistedFolderMember.path)) 'Recording an exact-folder review decision unexpectedly removed the disposable fixture directory.'
    $persistedRules = Send-WorkerRequest $restored 'preference_rule.list' @{ offset = 0; limit = 200 }
    Assert-True ($persistedRules.total -eq 1) 'Named preferred-root rule did not survive a worker restart.'
    $persistedRule = Send-WorkerRequest $restored 'preference_rule.get' @{ ruleId = $preferenceRule.rule.id }
    Assert-True ($persistedRule.rule.revision -eq $preferenceRule.rule.revision) 'Preferred-root rule revision changed across restart.'
    $persistedApplications = Send-WorkerRequest $restored 'preference_rule.application.page' @{
        runId = $run.id; ruleId = $persistedRule.rule.id; state = 'active'; pageSize = 25; cursor = $null
    }
    Assert-True ($persistedApplications.total -eq 1) 'Active preferred-root application provenance did not survive restart.'
    $persistedRuleMembers = Send-WorkerRequest $restored 'duplicate_file_group.members' @{
        runId = $run.id; groupId = $ruleOwnedMember.groupId; pageSize = 25
        sort = @{ field = 'path'; direction = 'ascending' }
        filter = @{ search = '' }; cursor = $null
    }
    $persistedManualOverride = $persistedRuleMembers.members |
        Where-Object { $_.id -eq $ruleOwnedMember.id } |
        Select-Object -First 1
    Assert-True ($persistedManualOverride.decision -eq 'undecided' -and $persistedManualOverride.decisionProvenance -eq 'manual') 'Later manual override did not survive restart as manual provenance.'
    $preferenceReversal = Send-WorkerRequest $restored 'preference_rule.application.reverse' @{
        operationId = [Guid]::NewGuid().ToString('N')
        runId = $run.id
        applicationId = $preferenceApplication.application.id
        expectedRevision = $persistedApplications.revision
    }
    Assert-True ($preferenceReversal.state -eq 'reversed') 'Preferred-root application reversal did not complete.'
    Assert-True ($preferenceReversal.removedRuleKeepCount -ge 1) 'Preferred-root reversal did not clear its rule Keep decisions.'
    $persistedPreview = Send-WorkerRequest $restored 'preference_rule.preview' @{
        runId = $run.id
        ruleId = $persistedRule.rule.id
        ruleRevision = $persistedRule.rule.revision
        reviewRevision = $preferenceReversal.appliedRevision
        pageSize = 25
        scope = @{ kind = 'completed_run' }
        cursor = $null
    }
    Assert-True ($persistedPreview.total -eq $preferencePreview.total) 'Preferred-root preview did not reconstruct consistently after restart.'
    Assert-True ([IO.File]::Exists($fileMembers.members[0].path)) 'Restarted preferred-root preview unexpectedly removed a disposable fixture file.'
    $manualRemoval = Send-WorkerRequest $restored 'review_decision.set' @{
        operationId = [Guid]::NewGuid().ToString('N')
        runId = $run.id
        groupId = $fileMembers.members[0].groupId
        fileId = $fileMembers.members[0].id
        decision = 'remove'
        expectedRevision = $preferenceReversal.appliedRevision
    }
    Assert-True ($manualRemoval.decision -eq 'remove') 'The preflight fixture Remove decision was not recorded.'
    $preflightOperation = [Guid]::NewGuid().ToString('N')
    $preflightStart = Send-WorkerRequest $restored 'preflight.start' @{
        operationId = $preflightOperation
        runId = $run.id
        expectedReviewRevision = $manualRemoval.appliedRevision
    }
    Assert-True (-not $preflightStart.replayed) 'The first preflight start was unexpectedly replayed.'
    Assert-True ($preflightStart.preflight.reviewRevision -eq $manualRemoval.appliedRevision) 'Preflight did not freeze the exact review revision.'
    $preflightReplay = Send-WorkerRequest $restored 'preflight.start' @{
        operationId = $preflightOperation
        runId = $run.id
        expectedReviewRevision = $manualRemoval.appliedRevision
    }
    Assert-True $preflightReplay.replayed 'Exact active preflight replay did not return the original generation.'
    Assert-True ($preflightReplay.preflight.id -eq $preflightStart.preflight.id) 'Preflight replay returned a different generation.'
    $completedPreflight = Wait-PreflightTerminal $restored $preflightStart.preflight.id
    Assert-True ($completedPreflight.status -eq 'completed') "Preflight ended in status $($completedPreflight.status)."
    Assert-True ($completedPreflight.processedItemCount -eq $completedPreflight.totalItemCount) 'Preflight did not commit every bounded observation.'
    Assert-True ($completedPreflight.readyCount -eq $completedPreflight.totalItemCount) 'Unchanged disposable preflight targets did not all validate ready.'
    Assert-True ($completedPreflight.changedCount -eq 0 -and $completedPreflight.missingCount -eq 0 -and $completedPreflight.unavailableCount -eq 0 -and $completedPreflight.conflictCount -eq 0) 'Unchanged disposable preflight produced a changed, missing, unavailable, or conflict outcome.'
    $preflightPage = Send-WorkerRequest $restored 'preflight.item.page' @{
        preflightId = $completedPreflight.id; pageSize = 1; outcome = $null; cursor = $null
    }
    Assert-True ($preflightPage.total -eq $completedPreflight.totalItemCount) 'Preflight detail total diverged from its frozen item count.'
    Assert-True ($preflightPage.items.Count -eq 1 -and $preflightPage.items[0].outcome -eq 'ready') 'Preflight detail paging did not return a ready observation.'
    Assert-True ([IO.File]::Exists($fileMembers.members[0].path)) 'Preflight unexpectedly removed a disposable fixture file.'
    $queryDiagnostics = $firstQueryDiagnostics + (Stop-SmokeWorker $restored)
    $restored = $null

    $restored = Start-SmokeWorker
    $null = Send-WorkerRequest $restored 'hello' @{
        protocolVersions = @(1)
        client = @{ name = 'windows-smoke-preflight-restart'; version = '1.0.0' }
    }
    $persistedPreflightResult = Send-WorkerRequest $restored 'preflight.get' @{ runId = $run.id }
    $persistedPreflight = $persistedPreflightResult.preflight
    Assert-True ($persistedPreflight.id -eq $completedPreflight.id -and $persistedPreflight.status -eq 'completed') 'Completed preflight did not survive worker restart.'
    Assert-True $persistedPreflight.isCurrent 'Restarted preflight no longer matched the unchanged review revision.'
    $persistedPreflightPage = Send-WorkerRequest $restored 'preflight.item.page' @{
        preflightId = $persistedPreflight.id; pageSize = 200; outcome = 'ready'; cursor = $null
    }
    Assert-True ($persistedPreflightPage.total -eq $persistedPreflight.totalItemCount) 'Restarted preflight details did not reconstruct from durable observations.'
    Assert-True ([IO.File]::Exists($fileMembers.members[0].path)) 'Restarted preflight browsing unexpectedly removed a disposable fixture file.'
    $queryDiagnostics += Stop-SmokeWorker $restored
    $restored = $null

    foreach ($phase in @('discovering', 'hashing', 'persisting', 'analyzing_folders', 'finalizing')) {
        Assert-True ($scanDiagnostics.Contains("kind=scan_phase run_id=$($run.id) phase=$phase")) "Missing $phase timing."
    }
    foreach ($method in @(
        'duplicate_file_group.page', 'duplicate_file_group.members',
        'duplicate_file_selected_root_facet.page',
        'duplicate_file_drive_facet.page',
        'duplicate_folder_group.page', 'duplicate_folder_group.members',
        'review_plan.get', 'review_folder_group.page',
        'preference_rule.preview', 'preflight.item.page')) {
        Assert-True ($queryDiagnostics.Contains("kind=result_query method=$method")) "Missing $method timing."
    }

    if (-not $SkipWpf) {
        Invoke-WpfAutomation $run.id
        Assert-WpfCloudFailClosedScenario $database
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
