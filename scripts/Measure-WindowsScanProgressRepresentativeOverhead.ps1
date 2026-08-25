[CmdletBinding()]
param([switch]$PreflightOnly)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$gitSafeRepo = $repo.Replace('\', '/')
$controlRevision = '0a3c1c1'
$treatmentRevision = 'f803cbd'
$evidencePath = Join-Path $repo 'docs/evidence/scan-progress-representative-overhead-20260825.json'
$shortEvidencePath = Join-Path $repo 'docs/evidence/scan-progress-overhead-20260825.json'
$tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
$profileRoot = Join-Path $tempParent ('super-duper-sop2-representative-' + [guid]::NewGuid().ToString('N'))
$campaignStartedAt = [DateTimeOffset]::UtcNow
$campaignWatchdog = [TimeSpan]::FromHours(2)
$campaignStartedTimestamp = [Diagnostics.Stopwatch]::GetTimestamp()
$campaignDeadlineTimestamp = $campaignStartedTimestamp +
    [long]([Diagnostics.Stopwatch]::Frequency * $campaignWatchdog.TotalSeconds)
$minimumRunNanos = 60000000000L
$maximumRunNanos = 600000000000L
$expectedFiles = 600008L
$expectedBytes = 4605870080L
$smallFiles = 600000L
$smallFileBytes = 4096L
$generatorSeed = '0x534F503246524550'
$runs = [Collections.Generic.List[object]]::new()
$timingBegan = $false
$evidenceWritten = $false
$thresholdFailed = $false

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using System.Text;

public sealed class Sop2FixtureFacts
{
    public long FileCount { get; set; }
    public long LogicalBytes { get; set; }
    public string ManifestSha256 { get; set; } = "";
    public string[] LargePairSha256 { get; set; } = Array.Empty<string>();
}

public sealed class Sop2ConditioningFacts
{
    public long FileCount { get; set; }
    public long LogicalBytes { get; set; }
    public string ContentSha256 { get; set; } = "";
}

public static class Sop2RepresentativeFixture
{
    public const int SmallDirectoryCount = 600;
    public const int SmallFilesPerDirectory = 1000;
    public const int SmallFileLength = 4096;
    public const ulong GeneratorSeed = 0x534F503246524550UL;
    public static readonly long[] LargePairLengths = new long[] {
        268435456L, 268500992L, 268566528L, 268632064L
    };

    private static ulong Next(ref ulong state)
    {
        state += 0x9E3779B97F4A7C15UL;
        ulong value = state;
        value = (value ^ (value >> 30)) * 0xBF58476D1CE4E5B9UL;
        value = (value ^ (value >> 27)) * 0x94D049BB133111EBUL;
        return value ^ (value >> 31);
    }

    private static void PutUInt64(byte[] buffer, int offset, ulong value)
    {
        for (int index = 0; index < 8 && offset + index < buffer.Length; index++)
            buffer[offset + index] = (byte)(value >> (index * 8));
    }

    private static void FillSmall(byte[] buffer, ulong globalIndex)
    {
        Array.Clear(buffer, 0, buffer.Length);
        PutUInt64(buffer, 0, globalIndex);
        ulong state = GeneratorSeed ^ (globalIndex * 0xD6E8FEB86659FD93UL);
        for (int offset = 8; offset < buffer.Length; offset += 8)
            PutUInt64(buffer, offset, Next(ref state));
    }

    private static void FillStreamBuffer(byte[] buffer, ref ulong state)
    {
        for (int offset = 0; offset < buffer.Length; offset += 8)
            PutUInt64(buffer, offset, Next(ref state));
    }

    private static string SmallRelativePath(int directory, long globalIndex)
    {
        return String.Format(CultureInfo.InvariantCulture,
            "small/{0:D3}/file-{1:D6}.bin", directory, globalIndex);
    }

    private static string LargeRelativePath(int pair, char member)
    {
        return String.Format(CultureInfo.InvariantCulture,
            "large/pair-{0}-{1}.bin", pair, member);
    }

    private static string NativePath(string root, string relative)
    {
        return Path.Combine(root, relative.Replace('/', Path.DirectorySeparatorChar));
    }

    private static void AppendManifest(IncrementalHash hash, string relative, long length)
    {
        byte[] path = Encoding.UTF8.GetBytes(relative);
        hash.AppendData(path);
        hash.AppendData(new byte[] { 0 });
        byte[] bytes = new byte[8];
        PutUInt64(bytes, 0, unchecked((ulong)length));
        hash.AppendData(bytes);
    }

    private static void AssertNormalFile(string path, long expectedLength)
    {
        FileInfo info = new FileInfo(path);
        if (!info.Exists || info.Length != expectedLength)
            throw new InvalidDataException("Unexpected fixture file or length: " + path);
        FileAttributes attributes = info.Attributes;
        FileAttributes forbidden = FileAttributes.SparseFile | FileAttributes.Compressed |
            FileAttributes.ReparsePoint | FileAttributes.Directory;
        if ((attributes & forbidden) != 0)
            throw new InvalidDataException("Fixture file is sparse, compressed, or a reparse point: " + path);
    }

    private static void WriteLarge(string path, long length, int pair)
    {
        byte[] buffer = new byte[1024 * 1024];
        ulong state = GeneratorSeed ^ (unchecked((ulong)pair + 1UL) * 0xA0761D6478BD642FUL);
        using (FileStream output = new FileStream(path, FileMode.CreateNew, FileAccess.Write,
            FileShare.None, buffer.Length, FileOptions.SequentialScan))
        {
            long remaining = length;
            while (remaining > 0)
            {
                FillStreamBuffer(buffer, ref state);
                int count = (int)Math.Min(buffer.Length, remaining);
                output.Write(buffer, 0, count);
                remaining -= count;
            }
            output.Flush(true);
        }
    }

    public static void Create(string root)
    {
        Directory.CreateDirectory(root);
        string smallRoot = Path.Combine(root, "small");
        Directory.CreateDirectory(smallRoot);
        byte[] buffer = new byte[SmallFileLength];
        long globalIndex = 0;
        for (int directory = 0; directory < SmallDirectoryCount; directory++)
        {
            string directoryPath = Path.Combine(smallRoot,
                directory.ToString("D3", CultureInfo.InvariantCulture));
            Directory.CreateDirectory(directoryPath);
            for (int member = 0; member < SmallFilesPerDirectory; member++)
            {
                FillSmall(buffer, unchecked((ulong)globalIndex));
                string relative = SmallRelativePath(directory, globalIndex);
                string path = NativePath(root, relative);
                using (FileStream output = new FileStream(path, FileMode.CreateNew, FileAccess.Write,
                    FileShare.None, SmallFileLength, FileOptions.SequentialScan))
                    output.Write(buffer, 0, buffer.Length);
                globalIndex++;
            }
        }

        Directory.CreateDirectory(Path.Combine(root, "large"));
        for (int pair = 0; pair < LargePairLengths.Length; pair++)
        {
            string first = NativePath(root, LargeRelativePath(pair, 'a'));
            string second = NativePath(root, LargeRelativePath(pair, 'b'));
            WriteLarge(first, LargePairLengths[pair], pair);
            File.Copy(first, second, false);
        }
    }

    private static byte[] HashFile(string path)
    {
        using (SHA256 sha = SHA256.Create())
        using (FileStream input = new FileStream(path, FileMode.Open, FileAccess.Read,
            FileShare.Read, 1024 * 1024, FileOptions.SequentialScan))
            return sha.ComputeHash(input);
    }

    public static Sop2FixtureFacts Validate(string root)
    {
        long files = 0;
        long bytes = 0;
        using (IncrementalHash manifest = IncrementalHash.CreateHash(HashAlgorithmName.SHA256))
        {
            long globalIndex = 0;
            for (int directory = 0; directory < SmallDirectoryCount; directory++)
            {
                for (int member = 0; member < SmallFilesPerDirectory; member++)
                {
                    string relative = SmallRelativePath(directory, globalIndex);
                    string path = NativePath(root, relative);
                    AssertNormalFile(path, SmallFileLength);
                    AppendManifest(manifest, relative, SmallFileLength);
                    files++;
                    bytes += SmallFileLength;
                    globalIndex++;
                }
            }

            byte[] previousPairHash = null;
            string[] pairHashes = new string[LargePairLengths.Length];
            for (int pair = 0; pair < LargePairLengths.Length; pair++)
            {
                string firstRelative = LargeRelativePath(pair, 'a');
                string secondRelative = LargeRelativePath(pair, 'b');
                string first = NativePath(root, firstRelative);
                string second = NativePath(root, secondRelative);
                AssertNormalFile(first, LargePairLengths[pair]);
                AssertNormalFile(second, LargePairLengths[pair]);
                byte[] firstHash = HashFile(first);
                byte[] secondHash = HashFile(second);
                if (!CryptographicOperations.FixedTimeEquals(firstHash, secondHash))
                    throw new InvalidDataException("Large fixture pair content differs: " + pair);
                if (previousPairHash != null && CryptographicOperations.FixedTimeEquals(previousPairHash, firstHash))
                    throw new InvalidDataException("Adjacent large fixture pair content unexpectedly matches: " + pair);
                previousPairHash = firstHash;
                pairHashes[pair] = Convert.ToHexString(firstHash).ToLowerInvariant();
                AppendManifest(manifest, firstRelative, LargePairLengths[pair]);
                AppendManifest(manifest, secondRelative, LargePairLengths[pair]);
                files += 2;
                bytes += LargePairLengths[pair] * 2;
            }

            long enumerated = 0;
            foreach (string ignored in Directory.EnumerateFiles(root, "*", SearchOption.AllDirectories))
                enumerated++;
            if (enumerated != files)
                throw new InvalidDataException("Fixture contains an unexpected file count.");

            return new Sop2FixtureFacts {
                FileCount = files,
                LogicalBytes = bytes,
                ManifestSha256 = Convert.ToHexString(manifest.GetHashAndReset()).ToLowerInvariant(),
                LargePairSha256 = pairHashes
            };
        }
    }

    private static void AppendFileContent(IncrementalHash hash, string path, byte[] buffer, ref long bytes)
    {
        using (FileStream input = new FileStream(path, FileMode.Open, FileAccess.Read,
            FileShare.Read, buffer.Length, FileOptions.SequentialScan))
        {
            int read;
            while ((read = input.Read(buffer, 0, buffer.Length)) > 0)
            {
                hash.AppendData(buffer, 0, read);
                bytes += read;
            }
        }
    }

    public static Sop2ConditioningFacts Condition(string root)
    {
        byte[] buffer = new byte[1024 * 1024];
        long files = 0;
        long bytes = 0;
        using (IncrementalHash content = IncrementalHash.CreateHash(HashAlgorithmName.SHA256))
        {
            long globalIndex = 0;
            for (int directory = 0; directory < SmallDirectoryCount; directory++)
            {
                for (int member = 0; member < SmallFilesPerDirectory; member++)
                {
                    AppendFileContent(content,
                        NativePath(root, SmallRelativePath(directory, globalIndex)), buffer, ref bytes);
                    files++;
                    globalIndex++;
                }
            }
            for (int pair = 0; pair < LargePairLengths.Length; pair++)
            {
                AppendFileContent(content, NativePath(root, LargeRelativePath(pair, 'a')), buffer, ref bytes);
                AppendFileContent(content, NativePath(root, LargeRelativePath(pair, 'b')), buffer, ref bytes);
                files += 2;
            }
            return new Sop2ConditioningFacts {
                FileCount = files,
                LogicalBytes = bytes,
                ContentSha256 = Convert.ToHexString(content.GetHashAndReset()).ToLowerInvariant()
            };
        }
    }
}
'@

function Invoke-Checked([scriptblock]$Command, [string]$Failure) {
    & $Command | Out-Host
    if ($LASTEXITCODE -ne 0) { throw $Failure }
}

function Assert-Watchdog {
    if ([Diagnostics.Stopwatch]::GetTimestamp() -ge $campaignDeadlineTimestamp) {
        throw 'The predeclared two-hour representative campaign watchdog expired.'
    }
}

function Get-FreeBytes {
    $drive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($profileRoot))
    $drive.AvailableFreeSpace
}

function Assert-FreeBytes([long]$Minimum, [string]$Stage) {
    [long]$free = Get-FreeBytes
    if ($free -lt $Minimum) {
        throw "$Stage requires at least $Minimum free bytes; observed $free."
    }
    $free
}

function Assert-NoProductProcesses {
    $running = @(Get-Process -Name 'SuperDuper.Windows','super-duper-worker' -ErrorAction SilentlyContinue)
    if ($running.Count -ne 0) {
        $details = $running | ForEach-Object { "$($_.ProcessName):$($_.Id)" }
        throw "Another Super Duper app or worker is running: $($details -join ', ')."
    }
}

function Expand-Revision([string]$Revision, [string]$Destination) {
    $archive = "$Destination.tar"
    [IO.Directory]::CreateDirectory($Destination) | Out-Null
    Invoke-Checked { git -C $repo -c "safe.directory=$gitSafeRepo" archive --format=tar --output=$archive $Revision } `
        "Could not archive revision $Revision."
    Invoke-Checked { tar -xf $archive -C $Destination } "Could not expand revision $Revision."
    [IO.File]::Delete($archive)
}

function Build-Worker([string]$Source, [string]$Target) {
    $previousTarget = $env:CARGO_TARGET_DIR
    try {
        $env:CARGO_TARGET_DIR = $Target
        Push-Location $Source
        try {
            Invoke-Checked { cargo build --release --locked -p super-duper-worker } `
                "Release worker build failed for $Source."
        }
        finally { Pop-Location }
    }
    finally {
        if ($null -eq $previousTarget) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
        else { $env:CARGO_TARGET_DIR = $previousTarget }
    }
    $worker = Join-Path $Target 'release/super-duper-worker.exe'
    if (-not (Test-Path -LiteralPath $worker -PathType Leaf)) {
        throw "Release worker not found: $worker"
    }
    $worker
}

function Build-StatusProbe([string]$Source, [string]$Target) {
    $probeRoot = Join-Path $Source 'sop2-status-probe'
    [IO.Directory]::CreateDirectory((Join-Path $probeRoot 'src')) | Out-Null
    $corePath = (Join-Path $Source 'crates/super-duper-core').Replace('\', '/')
    $cargoToml = @"
[package]
name = "sop2-status-probe"
version = "0.1.0"
edition = "2021"

[workspace]

[dependencies]
serde_json = "1.0"
super-duper-core = { path = "$corePath" }
"@
    $mainRs = @'
use std::{collections::BTreeMap, env, process};
use super_duper_core::telemetry::StatusDatabase;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 { return Err("usage: sop2-status-probe <db> <product-run-id>".into()); }
    let product_run_id: i64 = args[2].parse()?;
    let database = StatusDatabase::open_connection(&args[1])?;
    let matches: Vec<_> = database.list_runs(None, 100)?.into_iter()
        .filter(|run| run.product_run_id == Some(product_run_id)).collect();
    if matches.len() != 1 { return Err(format!("expected one status run, found {}", matches.len()).into()); }
    let run = &matches[0];
    let counters: BTreeMap<String, u64> = database.get_run_counters(run.id)?.into_iter()
        .map(|counter| (counter.metric, counter.value)).collect();
    println!("{}", serde_json::to_string(&serde_json::json!({
        "run": run,
        "counters": counters
    }))?);
    Ok(())
}

fn main() {
    if let Err(error) = run() { eprintln!("{error}"); process::exit(1); }
}
'@
    [IO.File]::WriteAllText((Join-Path $probeRoot 'Cargo.toml'), $cargoToml, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $probeRoot 'src/main.rs'), $mainRs, [Text.UTF8Encoding]::new($false))
    $previousTarget = $env:CARGO_TARGET_DIR
    try {
        $env:CARGO_TARGET_DIR = $Target
        Push-Location $probeRoot
        try {
            Invoke-Checked { cargo generate-lockfile --offline } 'Status-probe lockfile generation failed.'
            Invoke-Checked { cargo build --release --locked --offline } 'Release status-probe build failed.'
        }
        finally { Pop-Location }
    }
    finally {
        if ($null -eq $previousTarget) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
        else { $env:CARGO_TARGET_DIR = $previousTarget }
    }
    $probe = Join-Path $Target 'release/sop2-status-probe.exe'
    if (-not (Test-Path -LiteralPath $probe -PathType Leaf)) { throw "Status probe not found: $probe" }
    $probe
}

function Update-ProgressState($Connection, $Frame, [string]$Line) {
    if ($Connection.TerminalSeen) { throw 'Progress arrived after the matching terminal event.' }
    [long]$progressRunId = $Frame.data.runId
    if ($null -eq $Connection.ObservedRunId) { $Connection.ObservedRunId = $progressRunId }
    elseif ($Connection.ObservedRunId -ne $progressRunId) { throw 'Progress changed run ID.' }
    if ($null -ne $Connection.ExpectedRunId -and $Connection.ExpectedRunId -ne $progressRunId) {
        throw 'Progress run ID did not match the started run.'
    }
    [UInt64]$sequence = $Frame.data.sequence
    if ($sequence -le $Connection.LastSequence) { throw 'Progress transport sequence regressed.' }
    $Connection.LastSequence = $sequence
    $Connection.ProgressFrames++
    $Connection.ProgressBytes += [Text.Encoding]::UTF8.GetByteCount($Line) + 1
    $progressProperty = $Frame.data.PSObject.Properties['progress']
    if ($null -eq $progressProperty -or $null -eq $progressProperty.Value) { return }
    $progress = $progressProperty.Value
    $Connection.TypedProgressFrames++
    if ($progress.progressContractVersion -ne 1 -or $progress.metricsContractVersion -ne 2) {
        throw 'Treatment progress contract versions changed.'
    }
    [UInt64]$revision = $progress.revision
    if ($revision -le $Connection.LastRevision) { throw 'Progress source revision regressed.' }
    $Connection.LastRevision = $revision
    [UInt64]$discovered = $progress.counters.discoveredFiles
    [UInt64]$partial = $progress.logical.partialScreenedFiles
    [UInt64]$resolved = $progress.logical.hashPipelineResolvedFiles
    [UInt64]$fullStarted = $progress.counters.fullHashContentReadsStarted
    [UInt64]$fullCompleted = $progress.counters.fullHashContentReadsCompleted
    [UInt64]$fullBytes = [UInt64][string]$progress.counters.fullHashBytesRead
    if ($discovered -lt $Connection.LastDiscovered -or
        $partial -lt $Connection.LastPartial -or
        $resolved -lt $Connection.LastResolved -or
        $fullBytes -lt $Connection.LastFullBytes) {
        throw 'A typed cumulative progress value regressed.'
    }
    if ($partial -gt 0 -and $resolved -lt $partial) { $Connection.SawMidBucket = $true }
    if ($fullStarted -gt $fullCompleted -and $fullCompleted -eq $Connection.LastFullCompleted -and
        $fullBytes -ge $Connection.LastFullBytes + 8MB) { $Connection.SawMidRead = $true }
    $Connection.LastDiscovered = $discovered
    $Connection.LastPartial = $partial
    $Connection.LastResolved = $resolved
    $Connection.LastFullCompleted = $fullCompleted
    $Connection.LastFullBytes = $fullBytes
}

function Convert-ProtocolLine($Connection, [string]$Line) {
    try { $frame = $Line | ConvertFrom-Json -Depth 50 }
    catch { throw "Worker stdout was not protocol JSON: $Line" }
    if ($frame.type -eq 'event' -and $frame.event -eq 'run.progress') {
        Update-ProgressState $Connection $frame $Line
    }
    elseif ($frame.type -eq 'event' -and $frame.event -in @('run.completed','run.cancelled','run.failed')) {
        if ($Connection.TerminalSeen) { throw 'The worker emitted more than one terminal event.' }
        $Connection.TerminalSeen = $true
        $Connection.TerminalRun = $frame.data.run
    }
    $frame
}

function Read-Frame($Connection, [int]$TimeoutSeconds = 700, [long]$DeadlineTimestamp = 0) {
    Assert-Watchdog
    [long]$effectiveDeadline = $campaignDeadlineTimestamp
    if ($DeadlineTimestamp -gt 0 -and $DeadlineTimestamp -lt $effectiveDeadline) {
        $effectiveDeadline = $DeadlineTimestamp
    }
    [long]$remainingTicks = $effectiveDeadline - [Diagnostics.Stopwatch]::GetTimestamp()
    if ($remainingTicks -le 0) { throw 'A profile protocol deadline expired.' }
    [double]$remainingSeconds = $remainingTicks / [double][Diagnostics.Stopwatch]::Frequency
    [TimeSpan]$waitTime = [TimeSpan]::FromSeconds([Math]::Min([double]$TimeoutSeconds, $remainingSeconds))
    $read = $Connection.Process.StandardOutput.ReadLineAsync()
    if (-not $read.Wait($waitTime)) {
        throw 'Timed out waiting for a worker protocol frame.'
    }
    $line = $read.Result
    if ($null -eq $line) { throw 'Worker stdout closed unexpectedly.' }
    Convert-ProtocolLine $Connection $line
}

function Send-Request($Connection, [string]$Method, $Parameters, [long]$DeadlineTimestamp = 0) {
    $Connection.NextId++
    $id = $Connection.NextId.ToString([Globalization.CultureInfo]::InvariantCulture)
    $request = @{ type = 'request'; id = $id; method = $Method; params = $Parameters } |
        ConvertTo-Json -Compress -Depth 50
    $Connection.Process.StandardInput.WriteLine($request)
    $Connection.Process.StandardInput.Flush()
    while ($true) {
        $frame = Read-Frame $Connection 700 $DeadlineTimestamp
        if ($frame.type -eq 'response' -and $frame.id -eq $id) {
            if (-not $frame.ok) {
                throw "$Method failed: $($frame.error.code): $($frame.error.message)"
            }
            return $frame.result
        }
        if ($frame.type -eq 'response') { throw "$Method received an unexpected response ID $($frame.id)." }
    }
}

function Wait-Terminal($Connection, [long]$RunId, [long]$DeadlineTimestamp) {
    while ($true) {
        if ($Connection.TerminalSeen) {
            if ([long]$Connection.TerminalRun.id -ne $RunId) { throw 'Terminal run ID did not match.' }
            if ($Connection.TerminalRun.status -ne 'completed') {
                throw "Profile run $RunId ended as $($Connection.TerminalRun.status)."
            }
            return $Connection.TerminalRun
        }
        $null = Read-Frame $Connection 700 $DeadlineTimestamp
    }
}

function Assert-TerminalTruth($Run) {
    if ([long]$Run.filesDiscovered -ne $expectedFiles) { throw 'Terminal discovered-file total is wrong.' }
    if ([UInt64][string]$Run.bytesDiscovered -ne [UInt64]$expectedBytes) { throw 'Terminal discovered-byte total is wrong.' }
    if ([long]$Run.filesHashed -ne $expectedFiles) { throw 'Terminal partial-hash success total is wrong.' }
    if ([long]$Run.duplicateFileGroups -ne 4) { throw 'Terminal duplicate-file-group total is wrong.' }
    if ([long]$Run.duplicateFolderGroups -ne 0) { throw 'Terminal duplicate-folder-group total is wrong.' }
    if ([UInt64][string]$Run.wastedBytes -ne 1074135040UL) { throw 'Terminal recoverable-byte total is wrong.' }
    if ([long]$Run.warningCount -ne 0) { throw 'The representative scan produced warnings.' }
}

function Get-CanonicalResultFacts($Connection, [long]$RunId) {
    $page = Send-Request $Connection 'duplicate_file_group.page' @{
        runId = $RunId; pageSize = 25
        sort = @{ field = 'groupSize'; direction = 'ascending' }
        filter = @{ search = ''; minimumSize = '0' }; cursor = $null
    }
    if ([long]$page.total -ne 4 -or @($page.groups).Count -ne 4 -or $null -ne $page.nextCursor) {
        throw 'Product results did not contain exactly four duplicate-file groups.'
    }
    $groups = [Collections.Generic.List[object]]::new()
    for ($pair = 0; $pair -lt 4; $pair++) {
        $group = @($page.groups)[$pair]
        [UInt64]$expectedSize = [UInt64]$script:fixtureEvidence.largePairBytes[$pair]
        if ([UInt64][string]$group.groupSize -ne $expectedSize -or [long]$group.copyCount -ne 2 -or
            [UInt64][string]$group.recoverableBytes -ne $expectedSize) {
            throw "Duplicate group $pair size/copy facts differ from the fixture contract."
        }
        $members = Send-Request $Connection 'duplicate_file_group.members' @{
            runId = $RunId; groupId = $group.id; pageSize = 25
            sort = @{ field = 'path'; direction = 'ascending' }
            filter = @{ search = '' }; cursor = $null
        }
        $relativePaths = @($members.members | ForEach-Object { ([string]$_.relativePath).Replace('\', '/') } | Sort-Object)
        $expectedPaths = @("large/pair-$pair-a.bin", "large/pair-$pair-b.bin")
        if ([long]$members.total -ne 2 -or $relativePaths.Count -ne 2 -or $null -ne $members.nextCursor -or
            $relativePaths[0] -cne $expectedPaths[0] -or $relativePaths[1] -cne $expectedPaths[1]) {
            throw "Duplicate group $pair membership differs from the fixture contract."
        }
        $groups.Add([ordered]@{
            size = [string]$expectedSize
            fixtureSha256 = [string]$script:fixtureEvidence.largePairSha256[$pair]
            relativePaths = $relativePaths
        })
    }
    $canonical = $groups | ConvertTo-Json -Compress -Depth 10
    $digest = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData(
        [Text.Encoding]::UTF8.GetBytes($canonical))).ToLowerInvariant()
    [ordered]@{ digestSha256 = $digest; groups = @($groups) }
}

function Get-ReconciledStatusFacts([string]$StatusProbe, [string]$StatusPath, [long]$ProductRunId, $Terminal) {
    Assert-Watchdog
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $StatusProbe
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.ArgumentList.Add($StatusPath)
    $start.ArgumentList.Add($ProductRunId.ToString([Globalization.CultureInfo]::InvariantCulture))
    $probe = $null
    try {
        $probe = [Diagnostics.Process]::Start($start)
        if ($null -eq $probe) { throw 'Windows did not start the status probe.' }
        $stdout = $probe.StandardOutput.ReadToEndAsync()
        $stderr = $probe.StandardError.ReadToEndAsync()
        [long]$remainingTicks = $campaignDeadlineTimestamp - [Diagnostics.Stopwatch]::GetTimestamp()
        if ($remainingTicks -le 0) { throw 'Campaign deadline expired before durable reconciliation.' }
        [TimeSpan]$wait = [TimeSpan]::FromSeconds([Math]::Min(
            30.0, $remainingTicks / [double][Diagnostics.Stopwatch]::Frequency))
        if (-not $probe.WaitForExit($wait)) { throw 'Status probe exceeded its bounded 30-second wait.' }
        if (-not $stdout.Wait([TimeSpan]::FromSeconds(5)) -or
            -not $stderr.Wait([TimeSpan]::FromSeconds(5))) {
            throw 'Status-probe output did not finish draining.'
        }
        if ($probe.ExitCode -ne 0) {
            throw "Status probe failed for product run $ProductRunId`: $($stderr.Result)"
        }
        $facts = $stdout.Result | ConvertFrom-Json -Depth 20
    }
    finally {
        if ($null -ne $probe) {
            try {
                if (-not $probe.HasExited) {
                    $probe.Kill($true)
                    if (-not $probe.WaitForExit(5000)) {
                        throw 'Status probe remained alive after forced termination.'
                    }
                }
            }
            finally { $probe.Dispose() }
        }
    }
    if ($facts.run.state -ne 'completed' -or [long]$facts.run.productRunId -ne $ProductRunId -or
        [long]$facts.run.metricsContractVersion -ne 2 -or [long]$facts.run.lastSequence -le 0) {
        throw 'Durable terminal status row did not reconcile with the completed product run.'
    }
    $expected = [ordered]@{
        discovered_files = [UInt64]$expectedFiles
        discovered_bytes = [UInt64]$expectedBytes
        candidate_size_buckets = 5UL
        candidate_files = [UInt64]$expectedFiles
        candidate_bytes = [UInt64]$expectedBytes
        duplicate_candidate_size_buckets = 5UL
        duplicate_candidate_files = [UInt64]$expectedFiles
        duplicate_candidate_bytes = [UInt64]$expectedBytes
        partial_hashes_attempted = [UInt64]$expectedFiles
        partial_hashes_succeeded = [UInt64]$expectedFiles
        partial_hashes_failed = 0UL
        partial_hash_bytes_read = 614408192UL
        partial_collision_buckets = 4UL
        partial_collision_files = 8UL
        partial_collision_bytes = 2148270080UL
        full_hash_requests = 8UL
        full_hash_cache_hits = 0UL
        full_hash_cache_misses = 8UL
        full_hash_cache_errors = 0UL
        full_hash_cache_stores = 8UL
        full_hash_content_reads_started = 8UL
        full_hash_content_reads_completed = 8UL
        full_hash_content_reads_failed = 0UL
        full_hash_bytes_read = 2148270080UL
        confirmed_duplicate_groups = 4UL
        confirmed_logical_copies = 8UL
        confirmed_physical_items = 8UL
        recoverable_bytes = 1074135040UL
        warnings = 0UL
        telemetry_samples_lost = 0UL
        telemetry_flush_errors = 0UL
    }
    foreach ($entry in $expected.GetEnumerator()) {
        $property = $facts.counters.PSObject.Properties[$entry.Key]
        [UInt64]$actual = if ($null -eq $property) { 0UL } else { [UInt64][string]$property.Value }
        if ($actual -ne $entry.Value) {
            throw "Durable status counter $($entry.Key) did not reconcile."
        }
    }
    if ([UInt64][string]$facts.counters.discovered_files -ne [UInt64]$Terminal.filesDiscovered -or
        [UInt64][string]$facts.counters.discovered_bytes -ne [UInt64][string]$Terminal.bytesDiscovered -or
        [UInt64][string]$facts.counters.partial_hashes_succeeded -ne [UInt64]$Terminal.filesHashed -or
        [UInt64][string]$facts.counters.confirmed_duplicate_groups -ne [UInt64]$Terminal.duplicateFileGroups -or
        [UInt64][string]$facts.counters.recoverable_bytes -ne [UInt64][string]$Terminal.wastedBytes -or
        [UInt64][string]$facts.counters.warnings -ne [UInt64]$Terminal.warningCount) {
        throw 'Durable counters and terminal worker truth differ.'
    }
    $counterCanonical = $expected | ConvertTo-Json -Compress
    $counterDigest = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData(
        [Text.Encoding]::UTF8.GetBytes($counterCanonical))).ToLowerInvariant()
    [ordered]@{
        statusRunId = [long]$facts.run.id
        productRunId = [long]$facts.run.productRunId
        state = [string]$facts.run.state
        lastSequence = [UInt64][string]$facts.run.lastSequence
        deterministicCounterDigestSha256 = $counterDigest
        deterministicCounters = $expected
        counters = $facts.counters
    }
}

function Remove-SafeProfileChild([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $resolvedRoot = [IO.Path]::GetFullPath($profileRoot).TrimEnd('\')
    $resolved = [IO.Path]::GetFullPath($Path).TrimEnd('\')
    $prefix = $resolvedRoot + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unsafe profile child cleanup path: $resolved"
    }
    $item = Get-Item -LiteralPath $resolved -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Refusing to clean a reparse-point profile child: $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}

function Measure-Arm(
    [string]$Mode,
    [string]$Worker,
    [string]$StatusProbe,
    [string]$Fixture,
    [string]$ContentSha256,
    [int]$Ordinal,
    [int]$OrderIndex,
    [bool]$Warmup
) {
    Assert-Watchdog
    [long]$freeBefore = Assert-FreeBytes 15GB "Profile arm $Mode/$Ordinal"
    Assert-NoProductProcesses
    $attempt = [pscustomobject][ordered]@{
        mode = $Mode
        revision = if ($Mode -eq 'control') { $controlRevision } else { $treatmentRevision }
        warmup = $Warmup
        ordinal = $Ordinal
        orderIndex = $OrderIndex
        timingStarted = $false
        valid = $false
        failure = $null
        wallNanos = $null
        workerProcessCpuNanos = $null
        conditioningNanos = $null
        conditioningContentSha256 = $null
        progressFrames = 0L
        typedProgressFrames = 0L
        progressBytes = 0L
        lastSequence = '0'
        lastRevision = '0'
        sawMidBucket = $false
        sawMidRead = $false
        terminal = $null
        resultFacts = $null
        status = $null
        statusDatabaseBytes = $null
        processExited = $false
        forcedKill = $false
        stderr = $null
        freeBytesBefore = $freeBefore
        freeBytesAfter = $null
    }
    $runs.Add($attempt)
    $stateRoot = Join-Path $profileRoot ("state-$OrderIndex-$Mode-$Ordinal")
    $process = $null
    $connection = $null
    $wallBefore = 0L
    $cpuBefore = 0L
    try {
        Write-Host "Conditioning $Mode arm $Ordinal (warmup=$Warmup)..."
        $conditioningStarted = [Diagnostics.Stopwatch]::GetTimestamp()
        $conditioning = [Sop2RepresentativeFixture]::Condition($Fixture)
        $conditioningEnded = [Diagnostics.Stopwatch]::GetTimestamp()
        if ($conditioning.FileCount -ne $expectedFiles -or $conditioning.LogicalBytes -ne $expectedBytes -or
            $conditioning.ContentSha256 -ne $ContentSha256) {
            throw 'Revision-neutral conditioning did not reproduce the validated fixture.'
        }
        $attempt.conditioningNanos = [Diagnostics.Stopwatch]::GetElapsedTime(
            $conditioningStarted, $conditioningEnded).Ticks * 100
        $attempt.conditioningContentSha256 = $conditioning.ContentSha256
        Assert-Watchdog
        [IO.Directory]::CreateDirectory($stateRoot) | Out-Null
        $start = [Diagnostics.ProcessStartInfo]::new()
        $start.FileName = $Worker
        $start.WorkingDirectory = Split-Path $Worker
        $start.UseShellExecute = $false
        $start.CreateNoWindow = $true
        $start.RedirectStandardInput = $true
        $start.RedirectStandardOutput = $true
        $start.RedirectStandardError = $true
        $start.Environment['SUPER_DUPER_DB_PATH'] = Join-Path $stateRoot 'product.db'
        $start.Environment['SUPER_DUPER_STATUS_DB_PATH'] = Join-Path $stateRoot 'status.db'
        $start.Environment['HASH_CACHE_PATH'] = Join-Path $stateRoot 'hash-cache'
        $start.Environment['SUPER_DUPER_LOG'] = 'off'
        Assert-NoProductProcesses
        $process = [Diagnostics.Process]::Start($start)
        if ($null -eq $process) { throw "Windows did not start the $Mode worker." }
        $connection = [pscustomobject]@{
            Process = $process
            Stderr = $process.StandardError.ReadToEndAsync()
            NextId = 0
            ProgressFrames = 0L
            ProgressBytes = 0L
            TypedProgressFrames = 0L
            LastSequence = [UInt64]0
            LastRevision = [UInt64]0
            LastDiscovered = [UInt64]0
            LastPartial = [UInt64]0
            LastResolved = [UInt64]0
            LastFullCompleted = [UInt64]0
            LastFullBytes = [UInt64]0
            SawMidBucket = $false
            SawMidRead = $false
            ExpectedRunId = $null
            ObservedRunId = $null
            TerminalSeen = $false
            TerminalRun = $null
        }
        $hello = Send-Request $connection 'hello' @{
            protocolVersions = @(1)
            client = @{ name = 'sop2-representative-overhead'; version = '1.0.0' }
        }
        if ($hello.protocolVersion -ne 1) { throw 'Protocol V1 negotiation failed.' }
        $session = (Send-Request $connection 'session.create' @{
            name = "SOP2 representative $Mode $OrderIndex"
            roots = @($Fixture)
            ignorePatterns = @()
            cloudPolicy = 'exclude_registered_roots'
            manualLocationExclusions = @()
            registeredCloudLocations = @()
            cloudDetectionStatus = 'complete'
        }).session
        Write-Host "Measuring $Mode arm $Ordinal (warmup=$Warmup)..."
        $process.Refresh()
        $cpuBefore = $process.TotalProcessorTime.Ticks
        $wallBefore = [Diagnostics.Stopwatch]::GetTimestamp()
        [long]$deadline = $wallBefore + [long]([Diagnostics.Stopwatch]::Frequency * 600.0)
        $script:timingBegan = $true
        $attempt.timingStarted = $true
        $run = (Send-Request $connection 'run.start' @{ sessionId = $session.id } $deadline).run
        $connection.ExpectedRunId = [long]$run.id
        if ($null -ne $connection.ObservedRunId -and $connection.ObservedRunId -ne $connection.ExpectedRunId) {
            throw 'Progress observed before the start response belonged to another run.'
        }
        $terminal = Wait-Terminal $connection $run.id $deadline
        $wallAfter = [Diagnostics.Stopwatch]::GetTimestamp()
        $process.Refresh()
        $cpuAfter = $process.TotalProcessorTime.Ticks
        $attempt.wallNanos = [Diagnostics.Stopwatch]::GetElapsedTime($wallBefore, $wallAfter).Ticks * 100
        $attempt.workerProcessCpuNanos = ($cpuAfter - $cpuBefore) * 100
        $attempt.terminal = [ordered]@{
            filesDiscovered = [long]$terminal.filesDiscovered
            bytesDiscovered = [string]$terminal.bytesDiscovered
            filesHashed = [long]$terminal.filesHashed
            duplicateFileGroups = [long]$terminal.duplicateFileGroups
            duplicateFolderGroups = [long]$terminal.duplicateFolderGroups
            wastedBytes = [string]$terminal.wastedBytes
            warningCount = [long]$terminal.warningCount
        }
        Assert-TerminalTruth $terminal
        $attempt.resultFacts = Get-CanonicalResultFacts $connection $run.id
        $remainingTask = $process.StandardOutput.ReadToEndAsync()
        $process.StandardInput.Close()
        if (-not $process.WaitForExit(10000)) { throw "$Mode worker did not stop after EOF." }
        if (-not $remainingTask.Wait([TimeSpan]::FromSeconds(10))) {
            throw "$Mode worker stdout did not finish draining after process exit."
        }
        $remaining = $remainingTask.Result
        foreach ($line in @($remaining -split '\r?\n')) {
            if (-not [string]::IsNullOrWhiteSpace($line)) { $null = Convert-ProtocolLine $connection $line }
        }
        if ($process.ExitCode -ne 0) {
            if (-not $connection.Stderr.Wait([TimeSpan]::FromSeconds(10))) {
                throw "$Mode worker exited with code $($process.ExitCode), and stderr did not drain."
            }
            throw "$Mode worker exited with code $($process.ExitCode): $($connection.Stderr.Result)"
        }
        $attempt.processExited = $true
        $statusPath = Join-Path $stateRoot 'status.db'
        if (-not (Test-Path -LiteralPath $statusPath -PathType Leaf)) { throw 'Status database was not created.' }
        [long]$statusBytes = (Get-Item -LiteralPath $statusPath).Length
        if ($statusBytes -le 0) { throw 'Status database is empty.' }
        $attempt.statusDatabaseBytes = $statusBytes
        $attempt.status = Get-ReconciledStatusFacts $StatusProbe $statusPath $run.id $terminal
        [long]$wallNanos = $attempt.wallNanos
        [long]$cpuNanos = $attempt.workerProcessCpuNanos
        if ($wallNanos -lt $minimumRunNanos -or $wallNanos -gt $maximumRunNanos) {
            throw "Profile arm duration $wallNanos ns is outside 60-600 seconds."
        }
        if ($cpuNanos -le 0) { throw 'Worker CPU accounting was not positive.' }
        if ($connection.ProgressFrames -le 0) { throw "$Mode emitted no progress frame." }
        if ($Mode -eq 'treatment') {
            if ($connection.TypedProgressFrames -le 0) { throw 'Treatment emitted no typed progress.' }
            if (-not $connection.SawMidBucket) { throw 'Treatment did not expose a mid-bucket progress state.' }
            if (-not $connection.SawMidRead) { throw 'Treatment did not expose a mid-read progress state.' }
        }
        else {
            if ($connection.TypedProgressFrames -ne 0) { throw 'Control unexpectedly emitted typed progress.' }
        }
        Assert-Watchdog
        $attempt.progressFrames = $connection.ProgressFrames
        $attempt.typedProgressFrames = $connection.TypedProgressFrames
        $attempt.progressBytes = $connection.ProgressBytes
        $attempt.lastSequence = [string]$connection.LastSequence
        $attempt.lastRevision = [string]$connection.LastRevision
        $attempt.sawMidBucket = $connection.SawMidBucket
        $attempt.sawMidRead = $connection.SawMidRead
        $attempt.valid = $true
        Write-Host ("Completed {0} arm {1}: wall={2:N3}s cpu={3:N3}s frames={4}" -f `
            $Mode, $Ordinal, ($wallNanos / 1e9), ($cpuNanos / 1e9), $connection.ProgressFrames)
    }
    catch {
        $attempt.failure = $_.Exception.Message
        if ($null -ne $connection) {
            $attempt.progressFrames = $connection.ProgressFrames
            $attempt.typedProgressFrames = $connection.TypedProgressFrames
            $attempt.progressBytes = $connection.ProgressBytes
            $attempt.lastSequence = [string]$connection.LastSequence
            $attempt.lastRevision = [string]$connection.LastRevision
            $attempt.sawMidBucket = $connection.SawMidBucket
            $attempt.sawMidRead = $connection.SawMidRead
            if ($null -ne $connection.TerminalRun -and $null -eq $attempt.terminal) {
                $attempt.terminal = $connection.TerminalRun
            }
        }
        if ($attempt.timingStarted -and $attempt.wallNanos -eq $null) {
            $attempt.wallNanos = [Diagnostics.Stopwatch]::GetElapsedTime(
                $wallBefore, [Diagnostics.Stopwatch]::GetTimestamp()).Ticks * 100
        }
        if ($attempt.timingStarted -and $null -ne $process) {
            try {
                $process.Refresh()
                $attempt.workerProcessCpuNanos = ($process.TotalProcessorTime.Ticks - $cpuBefore) * 100
            } catch { }
        }
        throw
    }
    finally {
        try {
            if ($null -ne $process) {
                if (-not $process.HasExited) {
                    $attempt.forcedKill = $true
                    try { $process.Kill($true) } catch { }
                    if (-not $process.WaitForExit(5000)) {
                        throw 'Worker remained alive after forced kill.'
                    }
                }
                $attempt.processExited = $process.HasExited
                if ($null -ne $connection -and -not $connection.Stderr.IsCompleted) {
                    if (-not $connection.Stderr.Wait([TimeSpan]::FromSeconds(10))) {
                        throw 'Worker stderr did not finish draining after process exit.'
                    }
                }
                if ($null -ne $connection -and $connection.Stderr.IsCompleted) {
                    $stderrText = [string]$connection.Stderr.Result
                    if ($stderrText.Length -gt 4096) { $stderrText = $stderrText.Substring(0, 4096) }
                    $attempt.stderr = $stderrText
                }
                $process.Dispose()
            }
            try { $attempt.freeBytesAfter = Get-FreeBytes } catch { }
            Remove-SafeProfileChild $stateRoot
        }
        catch {
            $cleanupFailure = $_.Exception.Message
            $attempt.valid = $false
            if ([string]::IsNullOrWhiteSpace([string]$attempt.failure)) {
                $attempt.failure = "Cleanup failure: $cleanupFailure"
            } else {
                $attempt.failure = "$($attempt.failure) Cleanup failure: $cleanupFailure"
            }
            throw
        }
    }
}

function Get-BasisPoints([long]$Control, [long]$Treatment) {
    [long][Math]::Round((($Treatment - $Control) * 10000.0) / $Control, 0, [MidpointRounding]::AwayFromZero)
}

function Write-OnceJson($Value) {
    $json = $Value | ConvertTo-Json -Depth 30
    $encoding = [Text.UTF8Encoding]::new($false)
    $stream = [IO.FileStream]::new($evidencePath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try {
        $writer = [IO.StreamWriter]::new($stream, $encoding)
        try { $writer.WriteLine($json); $writer.Flush(); $stream.Flush($true) }
        finally { $writer.Dispose() }
    }
    finally { $stream.Dispose() }
}

function New-Evidence([string]$Disposition, [string]$Failure) {
    $measuredControl = @($runs | Where-Object { $_.valid -and -not $_.warmup -and $_.mode -eq 'control' })
    $measuredTreatment = @($runs | Where-Object { $_.valid -and -not $_.warmup -and $_.mode -eq 'treatment' })
    [long]$controlWall = [long](($measuredControl | Measure-Object wallNanos -Sum).Sum)
    [long]$treatmentWall = [long](($measuredTreatment | Measure-Object wallNanos -Sum).Sum)
    [long]$controlCpu = [long](($measuredControl | Measure-Object workerProcessCpuNanos -Sum).Sum)
    [long]$treatmentCpu = [long](($measuredTreatment | Measure-Object workerProcessCpuNanos -Sum).Sum)
    $aggregateNote = if ($measuredControl.Count -eq 5 -and $measuredTreatment.Count -eq 5) {
        'All five measured runs per revision contribute to the aggregate; no outlier is excluded.'
    } else {
        'The campaign became invalid before all measured arms completed; every attempted arm is retained.'
    }
    [ordered]@{
        gate = 'SOP2f-progress-acceptance'
        date = '2026-08-25'
        machine = 'designated Windows 11 x64 development machine'
        configuration = 'Release'
        semantics = 'warm_filesystem_cache_cold_application_state'
        controlRevision = $controlRevision
        treatmentRevision = $treatmentRevision
        controlCommit = $script:controlCommit
        treatmentCommit = $script:treatmentCommit
        generator = [ordered]@{ version = 1; algorithm = 'SplitMix64'; seed = $generatorSeed }
        fixture = $script:fixtureEvidence
        protocol = [ordered]@{
            warmupOrder = @('control','treatment')
            measuredOrder = @('control','treatment','treatment','control','control','treatment','treatment','control','control','treatment')
            measuredRunsPerMode = 5
            minimumRunNanos = $minimumRunNanos
            maximumRunNanos = $maximumRunNanos
            aggregateThresholdBasisPointsExclusive = 100
            shortWallCeilingNanos = 100000000
            shortCpuCeilingNanos = 125000000
            freshProductStatusAndHashStatePerArm = $true
            noRetriesOrOutlierRemoval = $true
        }
        shortLeg = [ordered]@{
            evidence = 'scan-progress-overhead-20260825.json'
            wallOverheadNanos = [long]$script:shortEvidence.absoluteDifference.wallNanos
            workerProcessCpuOverheadNanos = [long]$script:shortEvidence.absoluteDifference.cpuNanos
            disposition = 'passed_operator_approved_fixed_caps'
        }
        aggregate = [ordered]@{
            controlWallNanos = $controlWall
            treatmentWallNanos = $treatmentWall
            wallOverheadBasisPoints = if ($controlWall -gt 0 -and $measuredControl.Count -eq 5 -and $measuredTreatment.Count -eq 5) { Get-BasisPoints $controlWall $treatmentWall } else { $null }
            wallStrictlyBelowOnePercent = $null
            controlCpuNanos = $controlCpu
            treatmentCpuNanos = $treatmentCpu
            cpuOverheadBasisPoints = if ($controlCpu -gt 0 -and $measuredControl.Count -eq 5 -and $measuredTreatment.Count -eq 5) { Get-BasisPoints $controlCpu $treatmentCpu } else { $null }
            cpuStrictlyBelowOnePercent = $null
        }
        runs = @($runs)
        host = [ordered]@{
            os = [Environment]::OSVersion.VersionString
            logicalProcessors = [Environment]::ProcessorCount
            freeBytesBefore = $script:freeBytesBefore
            freeBytesAfterBuildAndFixture = $script:freeBytesAfterBuildAndFixture
            rustc = $script:rustcVersion
            cargo = $script:cargoVersion
            volumeDriveType = [string]$script:profileDrive.DriveType
            volumeFormat = [string]$script:profileDrive.DriveFormat
            fixtureStateAndBuildsShareTempVolume = $true
        }
        campaignStartedUtc = $campaignStartedAt.ToString('O')
        evidenceWrittenUtc = [DateTimeOffset]::UtcNow.ToString('O')
        disposition = $Disposition
        failure = if ([string]::IsNullOrWhiteSpace($Failure)) { $null } else { $Failure }
        notes = @(
            'This is the sole approved SOP2 representative-duration campaign; it is never retried or tuned to green.',
            'Fixture creation, validation, conditioning, builds, worker startup, Core, and WPF are outside the measured start-to-terminal boundary; wall time includes PowerShell/pipe scheduling and JSON consumption, while CPU is worker-process only.',
            'Negative aggregate differences mean no detected positive overhead and are not acceleration claims.',
            $aggregateNote
        )
    }
}

if (Test-Path -LiteralPath $evidencePath) {
    throw "Retained representative evidence already exists; refusing a rerun: $evidencePath"
}

$script:fixtureEvidence = $null
$script:freeBytesBefore = $null
$script:freeBytesAfterBuildAndFixture = $null
$script:rustcVersion = (& rustc --version).Trim()
$script:cargoVersion = (& cargo --version).Trim()
$script:profileDrive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($profileRoot))
$script:controlCommit = (& git -C $repo -c "safe.directory=$gitSafeRepo" rev-parse "$controlRevision^{commit}").Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the control revision.' }
$script:treatmentCommit = (& git -C $repo -c "safe.directory=$gitSafeRepo" rev-parse "$treatmentRevision^{commit}").Trim()
if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the treatment revision.' }
$script:shortEvidence = Get-Content -LiteralPath $shortEvidencePath -Raw | ConvertFrom-Json -Depth 30
if ($script:shortEvidence.controlRevision -ne $controlRevision -or
    $script:shortEvidence.treatmentRevision -ne $treatmentRevision -or
    [long]$script:shortEvidence.absoluteDifference.wallNanos -ne 66924300 -or
    [long]$script:shortEvidence.absoluteDifference.cpuNanos -ne -109375000) {
    throw 'Retained short-profile evidence does not match the operator-approved fixed-cost leg.'
}

if ($PreflightOnly) {
    Write-Output 'SOP2 representative harness executable preflight passed without creating campaign state.'
    return
}

[IO.Directory]::CreateDirectory($profileRoot) | Out-Null
try {
    Assert-NoProductProcesses
    $script:freeBytesBefore = Assert-FreeBytes 30GB 'Representative campaign setup'
    $controlSource = Join-Path $profileRoot 'control-source'
    $treatmentSource = Join-Path $profileRoot 'treatment-source'
    Expand-Revision $controlRevision $controlSource
    Expand-Revision $treatmentRevision $treatmentSource
    $controlTarget = Join-Path $profileRoot 'control-target'
    $controlWorker = Build-Worker $controlSource $controlTarget
    $treatmentWorker = Build-Worker $treatmentSource (Join-Path $profileRoot 'treatment-target')
    $statusProbe = Build-StatusProbe $controlSource $controlTarget
    Assert-Watchdog

    $fixture = Join-Path $profileRoot 'fixture'
    Write-Output 'Creating immutable representative fixture...'
    [Sop2RepresentativeFixture]::Create($fixture)
    Write-Output 'Validating fixture manifest, allocation, lengths, and large-pair content...'
    $fixtureFacts = [Sop2RepresentativeFixture]::Validate($fixture)
    if ($fixtureFacts.FileCount -ne $expectedFiles -or $fixtureFacts.LogicalBytes -ne $expectedBytes) {
        throw 'Validated fixture totals do not match the predeclared contract.'
    }
    Write-Output 'Computing revision-neutral fixture content digest...'
    $initialConditioning = [Sop2RepresentativeFixture]::Condition($fixture)
    if ($initialConditioning.FileCount -ne $expectedFiles -or $initialConditioning.LogicalBytes -ne $expectedBytes) {
        throw 'Initial fixture conditioning totals do not match the predeclared contract.'
    }
    $script:fixtureEvidence = [ordered]@{
        files = $fixtureFacts.FileCount
        logicalBytes = $fixtureFacts.LogicalBytes
        smallFiles = $smallFiles
        smallFileBytes = $smallFileBytes
        largePairBytes = @(268435456,268500992,268566528,268632064)
        normalAllocatedRequired = $true
        manifestSha256 = $fixtureFacts.ManifestSha256
        contentSha256 = $initialConditioning.ContentSha256
        largePairSha256 = @($fixtureFacts.LargePairSha256)
    }
    $script:freeBytesAfterBuildAndFixture = Assert-FreeBytes 20GB 'Post-fixture representative campaign'
    Assert-Watchdog

    $orderIndex = 0
    Measure-Arm 'control' $controlWorker $statusProbe $fixture $initialConditioning.ContentSha256 0 $orderIndex $true; $orderIndex++
    Measure-Arm 'treatment' $treatmentWorker $statusProbe $fixture $initialConditioning.ContentSha256 0 $orderIndex $true; $orderIndex++
    $measuredOrder = @('control','treatment','treatment','control','control','treatment','treatment','control','control','treatment')
    $ordinals = @{ control = 0; treatment = 0 }
    foreach ($mode in $measuredOrder) {
        $ordinals[$mode]++
        $worker = if ($mode -eq 'control') { $controlWorker } else { $treatmentWorker }
        Measure-Arm $mode $worker $statusProbe $fixture $initialConditioning.ContentSha256 $ordinals[$mode] $orderIndex $false
        $orderIndex++
    }

    Assert-Watchdog
    $evidence = New-Evidence 'measured' ''
    $validControl = @($runs | Where-Object { $_.valid -and -not $_.warmup -and $_.mode -eq 'control' })
    $validTreatment = @($runs | Where-Object { $_.valid -and -not $_.warmup -and $_.mode -eq 'treatment' })
    if ($validControl.Count -ne 5 -or $validTreatment.Count -ne 5 -or
        [long]$evidence.aggregate.controlWallNanos -le 0 -or [long]$evidence.aggregate.controlCpuNanos -le 0 -or
        [long]$evidence.aggregate.treatmentCpuNanos -le 0 -or
        $null -eq $evidence.aggregate.wallOverheadBasisPoints -or $null -eq $evidence.aggregate.cpuOverheadBasisPoints) {
        throw 'Representative aggregates are incomplete or non-positive.'
    }
    $resultDigests = @($runs | Where-Object valid | ForEach-Object { $_.resultFacts.digestSha256 } | Sort-Object -Unique)
    if ($resultDigests.Count -ne 1) { throw 'Canonical product-result facts differ between profile arms.' }
    $counterDigests = @($runs | Where-Object valid | ForEach-Object {
        $_.status.deterministicCounterDigestSha256
    } | Sort-Object -Unique)
    if ($counterDigests.Count -ne 1) { throw 'Deterministic durable counter facts differ between profile arms.' }
    [long]$wallBp = $evidence.aggregate.wallOverheadBasisPoints
    [long]$cpuBp = $evidence.aggregate.cpuOverheadBasisPoints
    [long]$controlWall = $evidence.aggregate.controlWallNanos
    [long]$treatmentWall = $evidence.aggregate.treatmentWallNanos
    [long]$controlCpu = $evidence.aggregate.controlCpuNanos
    [long]$treatmentCpu = $evidence.aggregate.treatmentCpuNanos
    $passed = $treatmentWall * 100L -lt $controlWall * 101L -and
        $treatmentCpu * 100L -lt $controlCpu * 101L
    $evidence.aggregate.wallStrictlyBelowOnePercent = $treatmentWall * 100L -lt $controlWall * 101L
    $evidence.aggregate.cpuStrictlyBelowOnePercent = $treatmentCpu * 100L -lt $controlCpu * 101L
    $evidence.disposition = if ($passed) { 'passed_two_part_budget' } else { 'failed_representative_threshold' }
    Write-OnceJson $evidence
    $evidenceWritten = $true
    $thresholdFailed = -not $passed
    Write-Output "Representative result retained: wall=$wallBp bp cpu=$cpuBp bp disposition=$($evidence.disposition)"
}
catch {
    $failure = $_.Exception.Message
    if ($timingBegan -and -not $evidenceWritten) {
        try {
            $invalid = New-Evidence 'invalid_campaign' $failure
            Write-OnceJson $invalid
            $evidenceWritten = $true
            Write-Warning "Retained invalid representative evidence: $failure"
        }
        catch {
            Write-Warning "Could not retain invalid evidence; temporary campaign root remains: $profileRoot"
            throw
        }
    }
    throw
}
finally {
    if ($evidenceWritten -or -not $timingBegan) {
        $resolvedProfileRoot = [IO.Path]::GetFullPath($profileRoot).TrimEnd('\')
        $resolvedParent = [IO.Path]::GetDirectoryName($resolvedProfileRoot).TrimEnd('\')
        if (-not $resolvedParent.Equals($tempParent, [StringComparison]::OrdinalIgnoreCase) -or
            -not ([IO.Path]::GetFileName($resolvedProfileRoot)).StartsWith('super-duper-sop2-representative-', [StringComparison]::Ordinal)) {
            throw "Unsafe profile cleanup path: $resolvedProfileRoot"
        }
        if (Test-Path -LiteralPath $resolvedProfileRoot) {
            $item = Get-Item -LiteralPath $resolvedProfileRoot -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Refusing to clean a reparse-point campaign root: $resolvedProfileRoot"
            }
            Remove-Item -LiteralPath $resolvedProfileRoot -Recurse -Force
        }
    }
}

if ($thresholdFailed) {
    throw "The retained representative profile failed the strict less-than-100-bp gate: $evidencePath"
}

Write-Output "SOP2 representative-duration profile passed and was retained at $evidencePath"
