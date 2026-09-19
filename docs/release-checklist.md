# Release checklist

How a Super Duper Windows release is prepared, verified, and published. The package is a
self-contained, unsigned Windows 11 x64 zip. There is no installer and no code signing yet.

## Gates

| Gate | Where it runs | What it proves |
|---|---|---|
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets` | CI, every PR and push to `master` | Formatting and lints |
| `cargo test --workspace`, `cargo build --workspace` (Debug) | CI | Rust engine, worker and CLI tests |
| `dotnet build` and `dotnet test -m:1` (Debug) | CI (`windows-latest`) | Core, Infrastructure and in-process STA smoke tests |
| Release build and tests (`cargo test --release`, `dotnet test -c Release`) | CI (`release-package` job) and the dedicated Windows VM, both via `Verify-WindowsRelease.ps1` | Release-only code paths, such as `WorkerExecutableLocator` ignoring dev overrides |
| Self-contained publish, version check, notices, package | CI (`release-package`) and the VM | The zip contents, product version, `LICENSE.txt`, `THIRD-PARTY-NOTICES.txt`, `CHANGELOG.md` |
| Worker protocol smoke | CI (`release-package`, `-SkipWpfSmoke`) and the VM | The worker beside the published app (`artifacts/windows-x64`), against disposable state. |
| **WPF smoke** against the published app | **The VM only**, `Verify-WindowsRelease.ps1` (needs a connected, idle RDP desktop) | Real WPF, and the worker beside it in the publish folder, against disposable state, including close and recovery scenarios |
| Failure-mode pass | **The VM**, by hand with the Release app (see below) | Graceful degradation; rerun when storage, worker lifecycle or path handling changes |
| Accessibility: high contrast, Narrator/NVDA, multi-monitor/200% DPI | **Not run for v0.1.0** (operator decision; issue #23) | Listed as unverified in `CHANGELOG.md` |

CI runs the Release configuration, the publish, and the worker protocol smoke against the published
worker on every PR and push, and keeps the zip from `master` builds for 14 days as the
`windows-x64-package` artifact. Only the WPF smoke needs the VM, and it is the only gate that drives
the published worker through the real app. Publish the zip built and smoked on the VM, not the CI
artifact.

### Failure-mode pass

With the Release app from the publish folder and disposable state (set `SUPER_DUPER_DB_PATH` to a
database file in a new folder under `artifacts/` or `%TEMP%`; the status database and hash cache
follow it), confirm the app handles each case gracefully:

- a corrupt database, a database from a newer version, and a read-only database;
- the worker process killed during a scan;
- a locked hash cache;
- missing roots and offline (disconnected) roots;
- paths longer than 260 characters.

[`windows-recovery.md`](windows-recovery.md) describes the expected behavior for each.

## Before the release

1. Every release PR is merged to `master`, and CI is green on `master`.
2. The version is the same in `[workspace.package] version` (root `Cargo.toml`) and `<Version>`
   (`apps/windows/Directory.Build.props`); the verifier fails if they differ.
3. `CHANGELOG.md` has the version's section, and its known limitations are current.
4. `cargo-about` 0.9.2, the version CI pins, is installed, and `about.toml` accepts every license
   in the worker's graph:

   ```powershell
   cargo install cargo-about --locked --features cli --version 0.9.2
   ```
5. On the VM, with RDP connected, the RDP window not minimized, and nobody using the desktop
   (another window taking the foreground fails the WPF focus checks):

   ```powershell
   pwsh -File scripts/Verify-WindowsRelease.ps1
   ```

   It must end with `Windows 11 x64 Release verification passed.` Do not use `-SkipSmoke` or
   `-SkipWpfSmoke` for a release candidate.
6. Copy the verified zip and its `.sha256` file out of `artifacts/`, for example to
   `artifacts/rc/<version>-<short commit>/`. Every verifier run rewrites
   `artifacts/super-duper-<version>-win-x64.zip`, so a later local run would replace the release
   candidate. Publish the copy.
7. Unzip the copied zip to a new folder, start `SuperDuper.Windows.exe` from Explorer, scan a small
   disposable folder, and close the app. This checks the zip itself rather than the publish folder.
8. Run the failure-mode pass above when storage, worker lifecycle or path handling changed since the
   last release.

## Publishing (operator only)

Tagging and publishing need explicit operator direction.

1. Tag the verified commit `v<version>` and push the tag.
2. Create the GitHub release from that tag. Attach the copied zip and its `.sha256` file, and use
   the version's `CHANGELOG.md` section as the release notes. Change "Unreleased" to the release
   date in a follow-up commit.

## After the release

- Confirm the release is recorded in `CHANGELOG.md` (its dated section) and on the GitHub release.
- Bump the version in both `Cargo.toml` and `apps/windows/Directory.Build.props` for the next
  cycle, and start a new "Unreleased" section in `CHANGELOG.md`.
