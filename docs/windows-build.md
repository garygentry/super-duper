# Windows build and release verification

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
- For release verification only: `cargo-about`, which `scripts/New-ThirdPartyNotices.ps1` needs
  to write the third-party notices. CI pins version 0.9.2:

  ```powershell
  cargo install cargo-about --locked --features cli --version 0.9.2
  ```

## Build and test

Debug:

```powershell
cargo build --workspace
cargo test --workspace
dotnet build apps/windows/SuperDuper.Windows.sln
dotnet test apps/windows/SuperDuper.Windows.sln -m:1
```

Release:

```powershell
cargo test --workspace --release
cargo build --workspace --release
dotnet build apps/windows/SuperDuper.Windows.sln --configuration Release
dotnet test apps/windows/SuperDuper.Windows.sln --configuration Release -m:1
```

- Run `cargo build` before the .NET build. `cargo test` and `cargo clippy` do not produce
  `super-duper-worker.exe`.
- Always pass `-m:1` to `dotnet test`. It runs the test projects one at a time; running the WPF
  STA surface suite next to the loaded Infrastructure host can starve dispatcher startup and
  produce a false UI timeout.
- The worker-backed Infrastructure tests use `target/debug/super-duper-worker.exe` in Debug and
  `target/release/super-duper-worker.exe` in Release. If that file is missing they report
  Inconclusive instead of failing.

[`windows-testing.md`](windows-testing.md) describes each test project, the opt-in
real-environment tests and the test-only environment variables.

## Run the app

```powershell
dotnet run --project apps/windows/src/SuperDuper.Windows/SuperDuper.Windows.csproj
```

The Debug WPF build copies `target/debug/super-duper-worker.exe` beside the app; Release copies
`target/release/super-duper-worker.exe`. The copy runs only if that file exists. A missing worker
gives a build warning in Debug; it fails the build in Release, and fails a `dotnet publish` in any
configuration.

A Release app launches only the worker beside it. A Debug app looks for the worker in this order
(`WorkerExecutableLocator`):

1. `SUPER_DUPER_WORKER_PATH`, when set.
2. `super-duper-worker.exe` beside the app.
3. `target/debug/super-duper-worker.exe` in the repository that contains the app or the current
   directory.

Because the Debug app prefers any worker already beside it, a stale worker left in `bin/` from an
earlier build can run against newer app code even though the build warned about it. After a
`cargo clean`, or whenever the worker changed, run `cargo build` before building the app.

For worker-backed UI development with a disposable database and an optional five-file test root,
run `./scripts/Start-WindowsUiDev.ps1 -CreateFixture`; see
[`windows-ui-dev-session.md`](windows-ui-dev-session.md).

## Verify a release build

```powershell
./scripts/Verify-WindowsRelease.ps1
```

The script:

1. Checks for Windows 11 x64 (build 22000 or newer) and that `<Version>` in
   `apps/windows/Directory.Build.props` matches `[workspace.package] version` in `Cargo.toml`.
2. Runs `cargo test --workspace --release` and `cargo build --workspace --release`, then builds
   the solution in Release and runs `dotnet test --configuration Release --no-build -m:1`.
3. Deletes `artifacts/windows-x64/` so stale binaries cannot satisfy the artifact checks, then
   publishes the self-contained `win-x64` app there.
4. Requires `SuperDuper.Windows.exe`, `super-duper-worker.exe`, `SuperDuper.Windows.dll` and
   `coreclr.dll`, and checks that both executables report the release version.
5. Writes `THIRD-PARTY-NOTICES.txt` and copies `LICENSE` (as `LICENSE.txt`) and `CHANGELOG.md`
   into the publish folder.
6. Runs `scripts/Invoke-WindowsSmoke.ps1 -Configuration Release -SkipBuild` with `-AppPath`
   pointing at the published `SuperDuper.Windows.exe`. The worker protocol half of the smoke
   always launches `target/release/super-duper-worker.exe`; only the WPF half drives the published
   app and the worker beside it.
7. Writes `artifacts/super-duper-<version>-win-x64.zip` (one top-level folder) and a `.sha256`
   file next to it.

It ends with `Windows 11 x64 Release verification passed.` followed by the publish folder, package
path and SHA-256.

Options:

```powershell
./scripts/Verify-WindowsRelease.ps1 -SkipSmoke
./scripts/Verify-WindowsRelease.ps1 -SkipWpfSmoke
```

`-SkipWpfSmoke` keeps the worker protocol smoke but skips the WPF automation, which needs an
interactive desktop. Use it only on a headless build agent; a release candidate still needs the
WPF smoke on an interactive Windows 11 desktop. See [`windows-smoke.md`](windows-smoke.md).

CI (`.github/workflows/ci.yml`) runs two jobs on every pull request and push to `master`:
`build-and-test` runs `cargo fmt --all --check`, `cargo clippy --workspace --all-targets` and the
Debug commands above, and `release-package` runs
`Verify-WindowsRelease.ps1 -SkipWpfSmoke`. The steps for preparing and publishing a release are in
[`release-checklist.md`](release-checklist.md).
