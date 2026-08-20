# Windows Build And Release Verification

The Windows MVP targets Windows 11 x64 and .NET 10. It is an unpackaged, framework-dependent WPF
application. Build Rust before .NET so the selected worker profile can be copied beside the app.

## Prerequisites

- Windows 11 build 22000 or newer on x64 hardware
- Rust stable (pinned by `rust-toolchain.toml`)
- .NET SDK 10.0.303 or a compatible patch (pinned by `global.json`)
- A Windows 11 SDK capable of targeting `10.0.22000.0`

## Developer Build

```powershell
cargo build --workspace
cargo test --workspace
dotnet build apps/windows/SuperDuper.Windows.sln
dotnet test apps/windows/SuperDuper.Windows.sln
```

Run the app with:

```powershell
dotnet run --project apps/windows/src/SuperDuper.Windows/SuperDuper.Windows.csproj
```

The Debug WPF build copies `target/debug/super-duper-worker.exe`; Release copies
`target/release/super-duper-worker.exe`. `SUPER_DUPER_WORKER_PATH` overrides worker discovery.

## Repeatable Release Verification

```powershell
./scripts/Verify-WindowsRelease.ps1
```

The script verifies Windows 11 x64, runs the Rust Release tests/build, builds and tests the .NET
solution in Release, publishes the framework-dependent `win-x64` app, verifies the app/worker
artifacts, and runs the deterministic smoke workflow. The generated
`artifacts/windows-x64/` directory is cleaned before publishing so stale binaries cannot satisfy
artifact checks.

Options:

```powershell
./scripts/Verify-WindowsRelease.ps1 -SkipSmoke
./scripts/Verify-WindowsRelease.ps1 -SkipWpfSmoke
```

`-SkipWpfSmoke` retains the worker/protocol smoke but skips interactive-desktop WPF automation.
Use it only on a headless build agent; a release candidate still needs the real WPF smoke on an
interactive Windows 11 desktop.

## Manual Command Matrix

```powershell
cargo test --workspace
cargo test --workspace --release
cargo build --workspace --release
dotnet build apps/windows/SuperDuper.Windows.sln
dotnet test apps/windows/SuperDuper.Windows.sln
dotnet build apps/windows/SuperDuper.Windows.sln --configuration Release
dotnet test apps/windows/SuperDuper.Windows.sln --configuration Release
```

`Verify-WindowsRelease.ps1` serializes the solution's test projects with `-m:1`. This keeps the WPF
STA surface suite isolated from the loaded Infrastructure host; running those projects concurrently
can starve dispatcher startup and produce a false UI timeout even when the isolated test passes.

Release C# integration tests select `target/release/super-duper-worker.exe`; Debug tests select
`target/debug/super-duper-worker.exe`.
