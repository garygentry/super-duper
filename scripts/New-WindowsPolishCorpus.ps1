[CmdletBinding()]
param([ValidateRange(201, 1000)][int]$DistinctGroups = 230)
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$destination = Join-Path $repository ('artifacts/ui-dev-session/polish-data-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $destination | Out-Null
# Only tracked, actual files are copied. Hash selection guarantees content-distinct groups.
$sourcePaths = @(& git -C $repository ls-files docs apps/windows/src crates plans)
if ($LASTEXITCODE -ne 0) { throw 'Could not enumerate tracked source files.' }
$hashes = [Collections.Generic.HashSet[string]]::new()
$selected = [Collections.Generic.List[string]]::new()
$manifest = [Collections.Generic.List[object]]::new()
function Copy-Source([string]$relative, [string]$copy) {
    $source = Join-Path $repository $relative
    $target = Join-Path $destination $copy
    New-Item -ItemType Directory -Force -Path (Split-Path $target -Parent) | Out-Null
    Copy-Item -LiteralPath $source -Destination $target
    $manifest.Add([pscustomobject]@{ Source = $relative; Copy = $copy; Bytes = (Get-Item -LiteralPath $target).Length; SHA256 = (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash })
}
# Select images first so the mixed corpus cannot accidentally become text-only.
$orderedPaths = @($sourcePaths | Where-Object { $_ -match '\.(png|ico|svg|jpg|jpeg|gif|zip)$' }) + @($sourcePaths | Where-Object { $_ -notmatch '\.(png|ico|svg|jpg|jpeg|gif|zip)$' })
foreach ($relative in $orderedPaths) {
    $source = Join-Path $repository $relative
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { continue }
    if ((Get-Item -LiteralPath $source).Length -eq 0) { continue }
    $hash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
    if (-not $hashes.Add($hash)) { continue }
    $selected.Add($relative)
    foreach ($collection in @('Working library', 'Backup archive')) { Copy-Source $relative "$collection/$relative" }
    if ($selected.Count -ge $DistinctGroups) { break }
}
if ($selected.Count -lt $DistinctGroups) { throw "Only $($selected.Count) distinct tracked contents were available." }
Copy-Source 'Cargo.toml' 'Working library/workspace-manifest.toml'
Copy-Source 'README.md' 'Working library/Project overview.md'
Copy-Source 'README.md' 'Backup archive/Read me — archived copy.md'
# A real document with >200 members exercises the separately bounded comparison page.
foreach ($index in 1..205) { Copy-Source 'README.md' ('Backup archive/Many copies/Overview-{0:D3}.md' -f $index) }
Copy-Source 'README.md' 'Working library/資料 — café/Archive of project documentation with a descriptive folder name/Nested folder for long Unicode path coverage/Project overview — résumé.md'
# A genuine archive of actual tracked documents adds archive bytes to the scan, with entry provenance.
$archivePath = Join-Path $destination 'source-documents.zip'
$archiveSources = @('README.md', 'Cargo.toml')
Compress-Archive -LiteralPath @($archiveSources | ForEach-Object { Join-Path $repository $_ }) -DestinationPath $archivePath
$archiveEntries = @($archiveSources | ForEach-Object { [pscustomobject]@{ Source = $_; SHA256 = (Get-FileHash -LiteralPath (Join-Path $repository $_) -Algorithm SHA256).Hash } })
foreach ($collection in @('Working library', 'Backup archive')) {
    $copy = "$collection/Actual project documents.zip"
    $target = Join-Path $destination $copy
    Copy-Item -LiteralPath $archivePath -Destination $target
    $manifest.Add([pscustomobject]@{ Source = 'generated:source-documents.zip'; Copy = $copy; Bytes = (Get-Item -LiteralPath $target).Length; SHA256 = (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash; ArchiveEntries = $archiveEntries })
}
$typeCounts = @($manifest | Group-Object { [IO.Path]::GetExtension($_.Copy).ToLowerInvariant() } | Sort-Object Name | ForEach-Object { [pscustomobject]@{ Extension = $_.Name; Copies = $_.Count } })
New-Item -ItemType Directory -Path (Join-Path $destination 'Empty location') | Out-Null
$manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $destination 'source-manifest.json') -Encoding utf8
[pscustomobject]@{ CreatedUtc = [datetime]::UtcNow; DistinctPairedSources = $selected.Count; UniqueSources = @($manifest.Source | Select-Object -Unique).Count; TypeCounts = $typeCounts; ArchiveEntries = $archiveEntries; Copies = $manifest.Count; Bytes = ($manifest | Measure-Object Bytes -Sum).Sum; SourceCommit = (& git -C $repository rev-parse HEAD); Roots = @('Working library', 'Backup archive'); Scope = 'Disposable copies of tracked real files; no performance claim; no source mutation' } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $destination 'corpus.json') -Encoding utf8
Write-Output "CORPUS=$destination"
Write-Output "MANIFEST_COPIES=$($manifest.Count)"
Write-Output "MANIFEST_BYTES=$(($manifest | Measure-Object Bytes -Sum).Sum)"
Write-Output 'Scan Working library and Backup archive; manifests remain outside selected roots.'
