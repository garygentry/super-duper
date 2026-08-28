Set-StrictMode -Version Latest

function Get-Sop9PersistenceStateFingerprint([string]$ProductDb) {
    $parts = foreach ($path in @($ProductDb, "$ProductDb-wal")) {
        try {
            if (Test-Path -LiteralPath $path -PathType Leaf) {
                $item = Get-Item -LiteralPath $path -Force
                "$($item.Name):$($item.Length):$($item.LastWriteTimeUtc.Ticks)"
            }
            else {
                "$(Split-Path -Leaf $path):absent"
            }
        }
        catch {
            "$(Split-Path -Leaf $path):unavailable"
        }
    }
    $parts -join '|'
}

function Get-Sop9PersistenceWatchdogDecision(
    [DateTimeOffset]$Now,
    [DateTimeOffset]$PhaseStarted,
    [DateTimeOffset]$LastActivity,
    [bool]$StateChanged,
    [int]$IdleTimeoutSeconds,
    [int]$AbsoluteTimeoutSeconds) {
    if (($Now - $PhaseStarted).TotalSeconds -ge $AbsoluteTimeoutSeconds) {
        return 'timeout_absolute'
    }
    if ($StateChanged) { return 'continue_activity' }
    if (($Now - $LastActivity).TotalSeconds -ge $IdleTimeoutSeconds) {
        return 'timeout_idle'
    }
    'continue_waiting'
}

function Test-Sop9PersistenceWatchdogController {
    $started = [DateTimeOffset]::Parse('2026-08-28T00:00:00Z')
    $activity = $started.AddMinutes(30)
    $waiting = Get-Sop9PersistenceWatchdogDecision $activity.AddSeconds(899) $started $activity `
        $false 900 86400
    $idle = Get-Sop9PersistenceWatchdogDecision $activity.AddSeconds(900) $started $activity `
        $false 900 86400
    $absolute = Get-Sop9PersistenceWatchdogDecision $started.AddSeconds(86400) $started `
        $started.AddSeconds(86399) $false 900 86400
    $changed = Get-Sop9PersistenceWatchdogDecision $activity.AddSeconds(900) $started $activity `
        $true 900 86400
    if ($waiting -ne 'continue_waiting' -or $idle -ne 'timeout_idle' -or
        $absolute -ne 'timeout_absolute' -or $changed -ne 'continue_activity') {
        throw 'The deterministic persistence-watchdog boundary test failed.'
    }
    [pscustomobject]@{
        status = 'passed'
        beforeIdleBoundary = $waiting
        atIdleBoundary = $idle
        atAbsoluteBoundary = $absolute
        activityPrecedesIdleEvaluation = $changed
    }
}
