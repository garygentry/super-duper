# AGENTS.md

Guidance for fresh coding-agent sessions in this repository.

## Active Roadmap Streams

At the beginning of every coding session, read and follow
`docs/windows-roadmap-session-handoff.md` before choosing or starting work. It schedules and
coordinates both durable roadmap streams:

- the active Windows post-MVP release-validation plan in
  `docs/windows-roadmap-closure-ledger.md` (with reusable kickoff prompt in
  `docs/windows-release-validation-kickoff-prompt.md`); and
- the parked large-drive scan optimization and observability plan in
  `docs/scan-optimization-plan.md`.

For an efficient cold start, audit Git; read the handoff's current-control sections and the active
closure ledger completely; then read only the selected gate's product/acceptance sections and
directly linked procedure. Do not load the handoff's historical accepted-slice record or decision
log unless the selected gate cites it. Do not
replay historical iteration logs or re-audit accepted/`locally_exhausted` gates without a documented
reopen condition. Active scheduling is not physical/provider/performance-campaign or production-
wiring authority; obtain every distinct approval required by the ledger. Do not infer work from the
parked scan plan. Keep the handoff and the selected stream's authoritative plan updated after every
completed gate or coherent gate group; once both roadmap streams are complete, remove this startup
instruction.

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
