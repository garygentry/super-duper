[CmdletBinding()]
param([Parameter(Mandatory)][string]$CorpusDirectory)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$allowed = [IO.Path]::GetFullPath((Join-Path $repository 'artifacts/ui-dev-session')) + [IO.Path]::DirectorySeparatorChar
$corpus = [IO.Path]::GetFullPath($CorpusDirectory)
if (-not $corpus.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) { throw 'Corpus must be inside the isolated UI development tree.' }
$state = Join-Path $allowed ('polish-baseline-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $state | Out-Null
$start = [Diagnostics.ProcessStartInfo]::new()
$start.FileName = Join-Path $repository 'target/debug/super-duper-worker.exe'
$start.WorkingDirectory = $repository
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.RedirectStandardInput = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
$start.Environment['SUPER_DUPER_DB_PATH'] = Join-Path $state 'super_duper.db'
$start.Environment['SUPER_DUPER_STATUS_DB_PATH'] = Join-Path $state 'scan_status.db'
$start.Environment['HASH_CACHE_PATH'] = Join-Path $state 'hash-cache'
$start.Environment['LOG_FILE_PATH'] = Join-Path $state 'app.log'
$script:requestId = 0
$frames = [Collections.Generic.List[object]]::new()
function Send-Request([string]$Method, $Parameters) {
    $script:requestId++
    $id = [string]$script:requestId
    $process.StandardInput.WriteLine((@{type='request';id=$id;method=$Method;params=$Parameters} | ConvertTo-Json -Depth 30 -Compress))
    $process.StandardInput.Flush()
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ([DateTime]::UtcNow -lt $deadline) {
        $line = $process.StandardOutput.ReadLineAsync()
        if (-not $line.Wait([TimeSpan]::FromSeconds(30))) { throw "Timeout: $Method" }
        if ($null -eq $line.Result) { throw 'Worker exited unexpectedly.' }
        $frame = $line.Result | ConvertFrom-Json -Depth 50
        $frames.Add($frame)
        if ($frame.type -eq 'response' -and $frame.id -eq $id) {
            if (-not $frame.ok) { throw ($frame.error | ConvertTo-Json -Compress) }
            return $frame.result
        }
    }
    throw "Deadline exceeded: $Method"
}
$process = [Diagnostics.Process]::Start($start)
$stderr = $process.StandardError.ReadToEndAsync()
try {
    $null = Send-Request 'hello' @{protocolVersions=@(1);client=@{name='polish-baseline';version='1'}}
    $session = (Send-Request 'session.create' @{
        name='Working library and backup'; roots=@((Join-Path $corpus 'Working library'), (Join-Path $corpus 'Backup archive'))
        ignorePatterns=@(); cloudPolicy='exclude_registered_roots'; manualLocationExclusions=@()
        registeredCloudLocations=@(); cloudDetectionStatus='complete'
    }).session
    $runs = @()
    foreach ($iteration in 1..2) {
        $run = (Send-Request 'run.start' @{sessionId=$session.id;repeatCachePolicy='reuse_verified'}).run
        $deadline = [DateTime]::UtcNow.AddMinutes(2)
        do {
            $run = (Send-Request 'run.get' @{runId=$run.id}).run
            if ($run.status -in @('completed','failed','cancelled','interrupted')) { break }
            Start-Sleep -Milliseconds 150
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($run.status -ne 'completed') { throw "Run did not complete: $($run.status)" }
        $runs += $run
    }
    $page = Send-Request 'duplicate_file_group.page' @{
        runId=$run.id;pageSize=25;sort=@{field='groupSize';direction='ascending'}
        filter=@{search='';minimumSize='0'};cursor=$null
    }
    if ($page.total -lt 2 -or -not $page.nextCursor) { throw 'Expected multiple real-document duplicate groups and a second page.' }
    $next = Send-Request 'duplicate_file_group.page' @{
        runId=$run.id;pageSize=25;sort=@{field='groupSize';direction='ascending'}
        filter=@{search='';minimumSize='0'};cursor=$page.nextCursor
    }
    @{runs=$runs;filePage=$page;nextPage=$next;corpus=$corpus;state=$state} | ConvertTo-Json -Depth 50 | Set-Content (Join-Path $state 'baseline.json') -Encoding utf8
    Write-Output "STATE_DIRECTORY=$state"
    Write-Output "RUNS=$($runs.id -join ',') FILE_GROUPS=$($page.total) COPIES=$($page.summary.matchingCopyCount)"
}
finally {
    $process.StandardInput.Close()
    if (-not $process.WaitForExit(10000)) { $process.Kill(); $process.WaitForExit(); Write-Warning 'Owned baseline worker required forced shutdown.' }
    $stderr.GetAwaiter().GetResult() | Set-Content (Join-Path $state 'worker-stderr.log')
    $frames | ConvertTo-Json -Depth 50 | Set-Content (Join-Path $state 'frames.json') -Encoding utf8
    $process.Dispose()
}
