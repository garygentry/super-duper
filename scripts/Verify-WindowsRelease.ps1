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
    throw 'Windows 11 x64 is required for the MVP release verification.'
}
$version = [Environment]::OSVersion.Version
if ($version.Major -lt 10 -or $version.Build -lt 22000) {
    throw "Windows 11 build 22000 or newer is required; found $version."
}

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
    dotnet publish $project --configuration Release --runtime win-x64 --self-contained false --output $publish
    if ($LASTEXITCODE -ne 0) { throw 'Windows x64 publish failed.' }

    foreach ($required in @('SuperDuper.Windows.exe', 'super-duper-worker.exe', 'SuperDuper.Windows.dll')) {
        $path = Join-Path $publish $required
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Release output is missing $required at $path."
        }
    }

    if (-not $SkipSmoke) {
        & (Join-Path $PSScriptRoot 'Invoke-WindowsSmoke.ps1') `
            -Configuration Release `
            -SkipBuild `
            -SkipWpf:$SkipWpfSmoke
        if ($LASTEXITCODE -ne 0) { throw 'Release smoke workflow failed.' }
    }
}
finally {
    Pop-Location
}

Write-Output "Windows 11 x64 Release verification passed. Publish output: $publish"
