# Release Checklist

How a Super Duper Windows release is prepared, verified, and published. The package is a
self-contained, unsigned Windows 11 x64 zip. There is no installer and no code signing yet.

## Gates

| Gate | Where it runs | What it proves |
|---|---|---|
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets` | CI, every PR and push to `master` | Formatting and lints |
| `cargo test --workspace`, `cargo build --workspace` (Debug) | CI | Rust engine, worker and CLI tests |
| `dotnet build` and `dotnet test -m:1` (Debug) | CI (`windows-latest`) | Core, Infrastructure and in-process STA smoke tests |
| Release build and tests (`cargo test --release`, `dotnet test -c Release`) | CI (`release-package` job) and **this VM**, both via `Verify-WindowsRelease.ps1` | Release-only code paths, such as `WorkerExecutableLocator` ignoring dev overrides |
| Self-contained publish, version check, notices, package | CI (`release-package`) and **this VM** | The zip contents, product version, `LICENSE.txt`, `THIRD-PARTY-NOTICES.txt`, `CHANGELOG.md` |
| Worker protocol smoke against the published app | CI (`release-package`, `-SkipWpfSmoke`) and **this VM** | Real worker from the publish folder against disposable state |
| **WPF smoke** against the published app | **This VM only**, `Verify-WindowsRelease.ps1` (needs a connected, idle RDP desktop) | Real WPF against disposable state, including close and recovery scenarios |
| Failure-mode pass (corrupt/newer/read-only database, worker kill, locked cache, missing and offline roots, long paths) | **This VM**, by hand with the Release app; see ROADMAP "First Release" | Graceful degradation; rerun when storage, worker lifecycle or path handling changes |
| Accessibility: high contrast, Narrator/NVDA, multi-monitor/200% DPI | **Not run for v0.1.0** (operator decision) | Listed as unverified in `CHANGELOG.md` |

CI runs the Release configuration, the publish and the worker smoke on every PR and push, and keeps
the zip from `master` builds for 14 days as the `windows-x64-package` artifact. Only the WPF smoke
needs this VM. Publish the zip built and smoked here, not the CI artifact.

## Before the release

1. Every release PR is merged to `master`, and CI is green on `master`.
2. The version is the same in `[workspace.package] version` (root `Cargo.toml`) and `<Version>`
   (`apps/windows/Directory.Build.props`); the verifier fails if they differ.
3. `CHANGELOG.md` has the version's section, and its known limitations are current.
4. `cargo-about` is installed (`cargo install cargo-about --locked --features cli`), and
   `about.toml` accepts every license in the worker's graph.
5. On this VM, with RDP connected, the RDP window not minimized, and nobody using the desktop
   (another window taking the foreground fails the WPF focus checks):

   ```powershell
   pwsh -File scripts/Verify-WindowsRelease.ps1
   ```

   It must end with `Windows 11 x64 Release verification passed.` Do not use `-SkipSmoke` or
   `-SkipWpfSmoke` for a release candidate.
6. Unzip `artifacts/super-duper-<version>-win-x64.zip` to a new folder, start
   `SuperDuper.Windows.exe` from Explorer, scan a small disposable folder, and close the app.
   This checks the zip itself rather than the publish folder.

## Publishing (operator only)

Tagging and publishing need explicit operator direction.

1. Tag the verified commit `v<version>` and push the tag.
2. Create the GitHub release from that tag. Attach the zip and its `.sha256` file, and use the
   version's `CHANGELOG.md` section as the release notes. Change "Unreleased" to the release date
   in a follow-up commit.

## After the release

- Record the release in `ROADMAP.md`, then bump the version in both places for the next cycle.
