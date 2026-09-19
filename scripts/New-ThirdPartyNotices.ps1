[CmdletBinding()]
param(
    # A published app folder containing SuperDuper.Windows.deps.json.
    [Parameter(Mandatory)]
    [string]$PublishDirectory,
    [Parameter(Mandatory)]
    [string]$OutputPath
)

# Writes THIRD-PARTY-NOTICES for the Windows release: the .NET runtime packs and NuGet packages
# listed in the publish's deps.json, the Rust crates linked into super-duper-worker.exe (via
# cargo-about and about.toml), and the native C/C++ libraries those crates compile in. It fails
# instead of writing an incomplete file when a license cannot be found.

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$rule = '-' * 80
$out = [Text.StringBuilder]::new()
function Add-Line([string]$Text = '') { [void]$out.AppendLine($Text) }

if (-not (Get-Command cargo-about -ErrorAction SilentlyContinue)) {
    throw 'cargo-about is required: cargo install cargo-about --locked --features cli'
}
$depsPath = Join-Path $PublishDirectory 'SuperDuper.Windows.deps.json'
if (-not (Test-Path -LiteralPath $depsPath)) { throw "Missing $depsPath; publish the app first." }
$nugetRoot = if ($env:NUGET_PACKAGES) { $env:NUGET_PACKAGES } else { Join-Path $HOME '.nuget\packages' }

Add-Line 'Super Duper - third-party notices'
Add-Line ''
Add-Line 'Super Duper is licensed under the MIT License (see LICENSE). This release also includes'
Add-Line 'the third-party components below, each under its own license.'
Add-Line ''

# .NET runtime packs and NuGet packages that ship in the publish folder.
Add-Line $rule
Add-Line '.NET RUNTIME AND PACKAGES'
$deps = Get-Content -Raw -LiteralPath $depsPath | ConvertFrom-Json -AsHashtable
foreach ($key in ($deps.libraries.Keys | Sort-Object)) {
    $library = $deps.libraries[$key]
    if ($library.type -notin @('package', 'runtimepack')) { continue }
    $name, $version = ($key -replace '^runtimepack\.', '') -split '/', 2
    $folder = Join-Path $nugetRoot (Join-Path $name.ToLowerInvariant() $version)
    if (-not (Test-Path -LiteralPath $folder)) { throw "Package folder not found for $name $version at $folder." }
    $nuspec = Get-ChildItem -LiteralPath $folder -Filter '*.nuspec' | Select-Object -First 1
    $licenseRef = $null
    if ($nuspec) {
        [xml]$spec = Get-Content -Raw -LiteralPath $nuspec.FullName
        $metadata = $spec.package.metadata
        $licenseRef = if ($metadata.PSObject.Properties['license'] -and $metadata.license.type -eq 'expression') {
            $metadata.license.'#text'
        } elseif ($metadata.PSObject.Properties['licenseUrl']) {
            $metadata.licenseUrl
        }
    }
    $files = @(Get-ChildItem -LiteralPath $folder -File |
        Where-Object { $_.Name -match '^(LICENSE|License|THIRD-PARTY-NOTICES|ThirdPartyNotices)' } |
        Sort-Object Name)
    if (-not $licenseRef -and $files.Count -eq 0) { throw "No license found for $name $version." }
    Add-Line $rule
    Add-Line "$name $version"
    if ($licenseRef) { Add-Line "License: $licenseRef" }
    foreach ($file in $files) {
        Add-Line ''
        Add-Line "[$($file.Name)]"
        Add-Line (Get-Content -Raw -LiteralPath $file.FullName).TrimEnd()
    }
    Add-Line ''
}

# Rust crates linked into the worker.
Add-Line $rule
Add-Line 'RUST CRATES IN super-duper-worker.exe'
$cratesFile = [IO.Path]::GetTempFileName()
try {
    Push-Location $repo
    try {
        cargo about generate --manifest-path crates/super-duper-worker/Cargo.toml `
            --config about.toml about.hbs --output-file $cratesFile
        if ($LASTEXITCODE -ne 0) { throw 'cargo about generate failed; see about.toml for accepted licenses.' }
    } finally { Pop-Location }
    Add-Line (Get-Content -Raw -LiteralPath $cratesFile).TrimEnd()
} finally { Remove-Item -LiteralPath $cratesFile -ErrorAction SilentlyContinue }
Add-Line ''

# Native libraries compiled into the worker by -sys crates. Crate license fields cover the Rust
# bindings; these are the upstream licenses of the bundled C and C++ sources.
Add-Line $rule
Add-Line 'NATIVE LIBRARIES BUNDLED IN super-duper-worker.exe'
$native = [ordered]@{
    'librocksdb-sys' = @(
        @{ Title = 'RocksDB (used under the Apache License 2.0; RocksDB is dual-licensed GPLv2 / Apache-2.0)'; Path = 'rocksdb/LICENSE.Apache' },
        @{ Title = 'RocksDB portions derived from LevelDB'; Path = 'rocksdb/LICENSE.leveldb' },
        @{ Title = 'Snappy'; Path = 'snappy/COPYING' })
    'bzip2-sys' = @(@{ Title = 'bzip2'; Path = 'bzip2-1.0.8/LICENSE' })
    'libz-sys' = @(@{ Title = 'zlib'; Path = 'src/zlib/LICENSE' })
    'lz4-sys' = @(@{ Title = 'LZ4 library'; Path = 'liblz4/lib/LICENSE' })
    'zstd-sys' = @(@{ Title = 'Zstandard'; Path = 'zstd/LICENSE' })
}
Push-Location $repo
try {
    $metadata = cargo metadata --format-version 1 --manifest-path crates/super-duper-worker/Cargo.toml | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
} finally { Pop-Location }
foreach ($crate in $native.Keys) {
    $package = @($metadata.packages | Where-Object { $_.name -eq $crate })
    if ($package.Count -ne 1) { throw "Expected exactly one $crate package in the worker graph; found $($package.Count)." }
    $crateRoot = Split-Path -Parent $package[0].manifest_path
    foreach ($entry in $native[$crate]) {
        $path = Join-Path $crateRoot $entry.Path
        if (-not (Test-Path -LiteralPath $path)) { throw "Missing $($entry.Title) license at $path." }
        Add-Line $rule
        Add-Line "$($entry.Title) (from $crate $($package[0].version))"
        Add-Line ''
        Add-Line (Get-Content -Raw -LiteralPath $path).TrimEnd()
        Add-Line ''
    }
}
Add-Line $rule
Add-Line 'SQLite (from libsqlite3-sys, bundled)'
Add-Line ''
Add-Line 'SQLite is in the public domain: https://www.sqlite.org/copyright.html'
Add-Line ''

$directory = Split-Path -Parent ([IO.Path]::GetFullPath($OutputPath))
[IO.Directory]::CreateDirectory($directory) | Out-Null
[IO.File]::WriteAllText([IO.Path]::GetFullPath($OutputPath), $out.ToString().Replace("`r`n", "`n").Replace("`n", "`r`n"))
Write-Output "Wrote $OutputPath"
