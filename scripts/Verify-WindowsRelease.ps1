[CmdletBinding()]
param(
    [switch]$SkipSmoke,
    [switch]$SkipWpfSmoke
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$solution = Join-Path $repo 'apps/windows/SuperDuper.Windows.sln'
$project = Join-Path $repo 'apps/windows/src/SuperDuper.Windows/SuperDuper.Windows.csproj'
$publish = Join-Path $repo 'artifacts/windows-x64'

if (-not $IsWindows -or -not [Environment]::Is64BitOperatingSystem) {
    throw 'Windows 11 x64 is required for the release verification.'
}
$osVersion = [Environment]::OSVersion.Version
if ($osVersion.Major -lt 10 -or $osVersion.Build -lt 22000) {
    throw "Windows 11 build 22000 or newer is required; found $osVersion."
}

# One release version, declared in two places that must agree.
[xml]$props = Get-Content -Raw -LiteralPath (Join-Path $repo 'apps/windows/Directory.Build.props')
$version = ($props.Project.PropertyGroup | Where-Object { $_.PSObject.Properties['Version'] } | Select-Object -First 1).Version
$cargoVersion = (Select-String -LiteralPath (Join-Path $repo 'Cargo.toml') -Pattern '^version = "([^"]+)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $version -or $version -ne $cargoVersion) {
    throw "Directory.Build.props version '$version' does not match Cargo.toml workspace version '$cargoVersion'."
}
$packageName = "super-duper-$version-win-x64"
$zip = Join-Path $repo "artifacts/$packageName.zip"

Push-Location $repo
try {
    cargo test --workspace --release
    if ($LASTEXITCODE -ne 0) { throw 'cargo test --workspace --release failed.' }
    cargo build --workspace --release
    if ($LASTEXITCODE -ne 0) { throw 'cargo build --workspace --release failed.' }
    dotnet build $solution --configuration Release
    if ($LASTEXITCODE -ne 0) { throw 'Release solution build failed.' }
    # Keep the UI STA suite isolated from the loaded Infrastructure project. Concurrent solution
    # test hosts can starve WPF dispatcher startup long enough to produce a false timeout.
    dotnet test $solution --configuration Release --no-build -m:1
    if ($LASTEXITCODE -ne 0) { throw 'Release solution tests failed.' }

    $expectedPublish = [IO.Path]::GetFullPath((Join-Path $repo 'artifacts/windows-x64'))
    $resolvedPublish = [IO.Path]::GetFullPath($publish)
    if (-not $resolvedPublish.Equals($expectedPublish, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to clean unexpected publish path: $resolvedPublish"
    }
    if (Test-Path -LiteralPath $resolvedPublish) {
        $publishItem = Get-Item -LiteralPath $resolvedPublish -Force
        if (-not $publishItem.PSIsContainer -or
            ($publishItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Refusing to clean a non-directory or reparse-point publish path: $resolvedPublish"
        }
        Remove-Item -LiteralPath $resolvedPublish -Recurse -Force
    }
    # Self-contained: the zip runs without an installed .NET runtime.
    dotnet publish $project --configuration Release --runtime win-x64 --self-contained true --output $publish
    if ($LASTEXITCODE -ne 0) { throw 'Windows x64 publish failed.' }

    foreach ($required in @('SuperDuper.Windows.exe', 'super-duper-worker.exe', 'SuperDuper.Windows.dll', 'coreclr.dll')) {
        $path = Join-Path $publish $required
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Release output is missing $required at $path."
        }
    }
    $appVersion = (Get-Item -LiteralPath (Join-Path $publish 'SuperDuper.Windows.exe')).VersionInfo.ProductVersion
    if (-not $appVersion.StartsWith($version, [StringComparison]::Ordinal)) {
        throw "SuperDuper.Windows.exe reports product version '$appVersion', expected $version."
    }

    & (Join-Path $PSScriptRoot 'New-ThirdPartyNotices.ps1') -PublishDirectory $publish `
        -OutputPath (Join-Path $publish 'THIRD-PARTY-NOTICES.txt')
    Copy-Item -LiteralPath (Join-Path $repo 'LICENSE') -Destination (Join-Path $publish 'LICENSE.txt')
    Copy-Item -LiteralPath (Join-Path $repo 'CHANGELOG.md') -Destination (Join-Path $publish 'CHANGELOG.md')

    if (-not $SkipSmoke) {
        # Smoke the published app itself, not the bin output it was built from.
        & (Join-Path $PSScriptRoot 'Invoke-WindowsSmoke.ps1') `
            -Configuration Release `
            -SkipBuild `
            -SkipWpf:$SkipWpfSmoke `
            -AppPath (Join-Path $publish 'SuperDuper.Windows.exe')
        if ($LASTEXITCODE -ne 0) { throw 'Release smoke workflow failed.' }
    }

    # The zip holds one top-level folder named after the package.
    if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
    $staging = Join-Path ([IO.Path]::GetTempPath()) ("super-duper-package-" + [guid]::NewGuid().ToString('N'))
    try {
        $packageRoot = Join-Path $staging $packageName
        [IO.Directory]::CreateDirectory($packageRoot) | Out-Null
        Copy-Item -Path (Join-Path $publish '*') -Destination $packageRoot -Recurse
        Compress-Archive -Path $packageRoot -DestinationPath $zip -CompressionLevel Optimal
    } finally {
        Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
    }
    $hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath "$zip.sha256" -Value "$hash  $packageName.zip" -Encoding ascii
}
finally {
    Pop-Location
}

Write-Output "Windows 11 x64 Release verification passed."
Write-Output "Publish output: $publish"
Write-Output "Package: $zip"
Write-Output "SHA-256: $hash"
