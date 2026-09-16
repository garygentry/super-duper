[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$destination = Join-Path $repository ('artifacts/ui-dev-session/polish-data-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $destination | Out-Null
# Copy actual repository documents and assets. Never scan or mutate the source checkout.
$sourcePaths = @(& git -C $repository ls-files docs apps/windows/src/SuperDuper.Windows/Assets)
if ($LASTEXITCODE -ne 0) { throw 'Could not enumerate tracked source files.' }
$manifest = @()
foreach ($relative in $sourcePaths) {
    $source = Join-Path $repository $relative
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { continue }
    foreach ($collection in @('Working library', 'Backup archive')) {
        $target = Join-Path $destination (Join-Path $collection $relative)
        New-Item -ItemType Directory -Force -Path (Split-Path $target -Parent) | Out-Null
        Copy-Item -LiteralPath $source -Destination $target
        $manifest += [pscustomobject]@{ Source = $relative; Copy = [IO.Path]::GetRelativePath($destination, $target); Bytes = (Get-Item -LiteralPath $target).Length; SHA256 = (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash }
    }
}
# A unique real document and a differently named copy distinguish names from content.
Copy-Item -LiteralPath (Join-Path $repository 'Cargo.toml') -Destination (Join-Path $destination 'Working library/workspace-manifest.toml')
Copy-Item -LiteralPath (Join-Path $repository 'README.md') -Destination (Join-Path $destination 'Working library/Project overview.md')
Copy-Item -LiteralPath (Join-Path $repository 'README.md') -Destination (Join-Path $destination 'Backup archive/Read me — archived copy.md')
New-Item -ItemType Directory -Path (Join-Path $destination 'Empty location') | Out-Null
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $destination 'source-manifest.json') -Encoding utf8
Write-Output "CORPUS=$destination"
Write-Output "MANIFEST_COPIES=$($manifest.Count)"
Write-Output "MANIFEST_BYTES=$(($manifest | Measure-Object Bytes -Sum).Sum)"
Write-Output 'Scan Working library and Backup archive; keep source-manifest.json outside selected roots.'
