# Windows Build And Release Verification

The Windows app targets Windows 11 x64 and .NET 10. It is an unpackaged WPF application; releases
ship as a self-contained zip. Build Rust before .NET so the selected worker profile can be copied
beside the app.

## Prerequisites

- Windows 11 build 22000 or newer on x64 hardware
- Rust stable (pinned by `rust-toolchain.toml`), 1.98 or newer (`rust-version` in `Cargo.toml`);
  run `rustup update stable` if an older stable toolchain is installed
- .NET SDK 10.0.400 or a compatible patch (pinned by `global.json`)
- A Windows 11 SDK capable of targeting `10.0.22000.0`
- Visual Studio C++ x64/x86 build tools (`Microsoft.VisualStudio.Component.VC.Tools.x86.x64`)
  for Rust's MSVC linker and RocksDB native compilation
- Visual Studio Clang (`Microsoft.VisualStudio.Component.VC.Llvm.Clang`) for RocksDB bindgen;
  set `LIBCLANG_PATH` to the installed x64 LLVM `bin` directory if bindgen cannot find `libclang.dll`
- PowerShell 7 (`pwsh`, for example `winget install --id Microsoft.PowerShell`) for the
  `scripts/*.ps1` workflows, which use .NET APIs that Windows PowerShell 5.1 lacks

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
`target/release/super-duper-worker.exe`. Debug builds honor `SUPER_DUPER_WORKER_PATH` and fall back
to `target/debug`; Release builds ignore both and launch only the worker beside the app.

For worker-backed UI development with a disposable database and optional five-file test root,
see [`windows-ui-dev-session.md`](windows-ui-dev-session.md) and run
`./scripts/Start-WindowsUiDev.ps1 -CreateFixture`.

## Repeatable Release Verification

```powershell
./scripts/Verify-WindowsRelease.ps1
```

The script verifies Windows 11 x64, runs the Rust Release tests/build, builds and tests the .NET
solution in Release, publishes the self-contained `win-x64` app, verifies the app/worker
artifacts and version, adds the license, third-party notices and changelog, runs the deterministic
smoke workflow against the published app, and packages `artifacts/super-duper-<version>-win-x64.zip`
with a SHA-256 file (see `release-checklist.md`). The generated
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
