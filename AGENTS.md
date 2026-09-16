# AGENTS.md

Guidance for fresh coding-agent sessions in this repository.

## Active Roadmap Streams

The operator activated the Windows UI redesign on 2026-09-08 and accepted its final scoped workflow
at UIR-09 on 2026-09-14. All redesign plans, prototypes, implementation, tests and completion records
stay on `codex/ui-redesign`. The prior
checkout and operator README edit are preserved on `wpf-poc` at `deefa40`. Do not switch branches,
merge back, rebase or delete the redesign branch unless the user explicitly changes this instruction.

At the beginning of every coding session, read and follow
`docs/windows-roadmap-session-handoff.md` before choosing or starting work. For the redesign,
read `plans/ui-redesign/README.md`, `plans/ui-redesign/execution-plan.md` and the compact
`plans/ui-redesign/session-checkpoint.md`, then only the selected
gate's linked specifications, code and tests. The redesign package owns the new UI scope and next
step. Do not restart or re-audit the retained streams merely to begin UI work:

- the completed-at-SOP10 large-drive scan optimization and observability plan in
  `docs/scan-optimization-plan.md` (with reusable kickoff prompt in
  `docs/scan-optimization-kickoff-prompt.md`); and
- the parked Windows post-MVP release-validation plan in
  `docs/windows-roadmap-closure-ledger.md`.

For an efficient cold start, audit Git; read the handoff's current-control sections and the selected
stream's execution plan completely; then read only the selected gate's directly linked code, tests, and
procedures. The redesign has no remaining gate; do not invent a follow-on without explicit operator
direction. Do not load the handoff's historical accepted-slice record or decision log unless the
selected gate cites it. Do not
replay historical iteration logs or re-audit accepted/`locally_exhausted` gates without a documented
reopen condition. Active scheduling is not physical/provider/performance-campaign or production-
wiring authority; obtain every distinct approval required by the active plan. Do not infer work
from the parked Windows plan. Keep the handoff and the selected stream's authoritative plan updated
after every completed gate or coherent gate group; once all scheduled roadmap streams are complete, remove
this startup instruction.

Use `plans/ui-redesign/codex-session-guide.md` for multi-session execution and update its compact
checkpoint with each coherent slice. Long-scan monitoring and persistent qualified rescan reuse
are required by `plans/ui-redesign/scan-and-rescan-experience.md` (A08/A16/A17).
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
