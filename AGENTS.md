# AGENTS.md

Guidance for fresh coding-agent sessions in this repository.

## Standing computer-control approval

The operator explicitly grants standing approval to control this dedicated Windows VM/sandbox
(reconfirmed 2026-09-16). Always treat Computer Use for the authorized task as approved; do not ask
again to launch, capture or operate native apps or to perform the requested appearance checks.
An app-approval timeout is a tool availability failure, not missing operator authorization.
After the operator reports approving the tool, resume with fresh window discovery. Preserve the
task's explicit no-merge/push/release/deletion-activation boundaries and runtime isolation.

## Completed roadmap streams

UIR-00–UIR-09 and the subsequent usability/visual-polish P00–P08 stream are complete.
`plans/ui-polish/session-checkpoint.md` and `plans/ui-polish/implementation-evidence.md` record final UX23
native loading acceptance from `ecd8bc7` and retained verification. No scheduled UI work remains;
the mandatory roadmap cold-start procedure is retired. Do not replay accepted gates or infer
new work from the parked Windows release/provider/deletion campaign.

Keep all work on `codex/ui-redesign`. Preserve `wpf-poc` and `origin/wpf-poc` at `deefa40`,
including the operator README edit. Do not switch branches, merge, rebase, push, release,
delete the branch or enable deletion without explicit operator direction. The app remains
review-only. Historical plans and evidence remain available for a specifically reopened scope.
Long-scan monitoring and persistent qualified rescan reuse remain required by
`plans/ui-redesign/scan-and-rescan-experience.md` (A08/A16/A17).

On the dedicated Windows VM, remote desktop input may be unavailable while the operator's session is
backgrounded or locked. Continue authorized repository work, Rust/.NET builds and tests, isolated
worker fixtures, and loaded-STA WPF fixture control/captures without waiting for desktop input. Follow
`docs/windows-ui-dev-session.md` for the verified background workflow. If Computer Use cannot capture
or send native input, stop those input calls and record that specific native check as unrun; do not
pause unrelated authorized development or ask the operator to unlock the VM merely to proceed with
background checks. Recheck native input only when the desktop is available or the selected work
actually requires physical desktop acceptance. Background evidence does not stand in for a required
physical/provider/release gate.
At every session handoff with remaining work, print a copyable continuation prompt in the final
response, tailored to the committed checkpoint and exact next slice; follow the session guide.

## Current State

This branch contains the clean-slate Rust workspace and the new WPF/.NET 10 Windows MVP scaffold.
The previous Windows app implementation remains removed, including its `ui/windows` tree and its
XAML/C# structure.

The app under `apps/windows` is a new product surface over the Rust engine's worker-process boundary,
not a continuation of the deleted app.

## What Remains

```text
super-duper/
  Cargo.toml
  Cargo.lock
  Config.toml
  crates/
    super-duper-core/     # scanner, hasher, analysis, SQLite storage, deletion plans
    super-duper-cli/      # headless command-line driver
    super-duper-ffi/      # C ABI for future native clients
    super-duper-worker/   # versioned JSONL worker for the Windows app
  apps/windows/           # WPF/.NET 10 solution, application layers, and tests
  docs/
    architecture.svg
  README.md
  ROADMAP.md
  CLAUDE.md
  crates/CLAUDE.md
```

## Build And Test

Use the Rust workspace commands:

```bash
cargo build --workspace
cargo test --workspace
```

Use the Windows solution commands after building Rust so the worker executable is available:

```bash
dotnet build apps/windows/SuperDuper.Windows.sln
dotnet test apps/windows/SuperDuper.Windows.sln
```

Use the repeatable hardening workflows on Windows 11 x64:

```powershell
./scripts/Invoke-WindowsSmoke.ps1
./scripts/Verify-WindowsRelease.ps1
```

Build, smoke, diagnostics, known limitations, and recovery are documented in
`docs/windows-build.md`, `docs/windows-smoke.md`, and `docs/windows-recovery.md`.
For isolated real-app UI iteration on Windows, use `scripts/Start-WindowsUiDev.ps1 -CreateFixture`
and `docs/windows-ui-dev-session.md`.

The last Milestone 6 verification ran the Debug/Release Rust and .NET matrix plus the real Release
worker/WPF smoke workflow on Windows 11 x64; all checks passed.

## Development Notes

- Keep `super-duper-core` UI-agnostic.
- Keep `super-duper-ffi` as a stable native-client contract, not tailored to one app.
- Do not reintroduce the old Windows app structure, XAML, C# view models, services, or workarounds.
- Runtime files such as `super_duper.db`, `content_hash_cache.db`, and `logs/` should stay out of
  source control.
- The generated FFI header is `crates/super-duper-ffi/super_duper.h`; building the FFI crate may
  refresh it.

## Windows App Work

Follow `docs/windows-mvp-plan.md` and `docs/worker-protocol-v1.md`. Keep WPF views in the executable,
application contracts/view models in `SuperDuper.Windows.Core`, and process/native concerns in
`SuperDuper.Windows.Infrastructure`.
