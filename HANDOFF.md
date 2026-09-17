# HANDOFF

Development of this repository is moving to the dedicated Windows VM. This file is the plan for
that move and for the work that finishes on the other side of it. **Delete this file once the VM
session has picked the work up and the merge is done** — it is a temporary relay, not a permanent
document. Durable guidance belongs in `AGENTS.md`, `crates/CLAUDE.md` and `ROADMAP.md`.

Branch: `codex/ui-redesign`, pushed and clean. `master` has not moved from `54c7d48`, so the branch
is 236 commits ahead and merges as a fast-forward. The goal on the VM is to confirm stability on a
clean machine and then open the pull request into `master`.

## Why the move

The desktop machine cannot exercise the Windows app. The WPF smoke journey drives the real app
through UI Automation and opens real Explorer windows, so it needs an interactive desktop nobody
else is using. Everything else — the Rust workspace, the worker, the headless .NET tests, docs —
builds and tests fine there, and that is what the recent desktop sessions covered.

## State at handoff

Verified on the desktop machine at `9210725`:

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets` clean
- `cargo test --workspace`: 250 passing
- `dotnet build`: no warnings; `dotnet test -m:1`: 228 + 80 + 4 with six opt-in skips
- `scripts/Verify-WindowsRelease.ps1` was last run green end to end, including the WPF smoke, by
  the session that produced `4cb9a50` — on this machine, before the documentation commits

What landed recently, in case a failure needs attributing:

- `ae3658d` hardened legacy deletion-plan marking and execution
- `8cfd2a8` made hashing and exact-folder verification share one RocksDB hash-cache store
- `c7ce421..4cb9a50` moved the workspace to edition 2024 and a Rust 1.98 floor, upgraded dependency
  majors, migrated the Windows tests to MSTest 4, fixed two keyboard-accessibility regressions, and
  rewrote the WPF smoke journey for the redesigned shell
- `9210725` and the commit adding this plan rewrote the agent docs and cleaned up repository drift

## Plan on the VM

1. **Sync and set up.** Pull `codex/ui-redesign`, confirm the tip, then reset the toolchain:
   `rustup update stable` to 1.98 or newer, the .NET SDK `global.json` pins, a Windows 11 SDK
   targeting `10.0.22000.0`, VS C++ x64 build tools, and VS Clang with `LIBCLANG_PATH`. Run
   `cargo clean` once before the first build: any `target/` left from the old edition and
   dependency set is dead weight, and running out of disk mid-link is this project's most common
   failure. Debug builds are about 7 GB per cycle now.

2. **Establish the baseline** and compare against the numbers above:

   ```powershell
   cargo fmt --all --check
   cargo clippy --workspace --all-targets
   cargo test --workspace
   dotnet build apps/windows/SuperDuper.Windows.sln
   dotnet test apps/windows/SuperDuper.Windows.sln -m:1
   ```

   Keep `-m:1`. A deviation here is information about the machine or the move; report it rather
   than working around it.

3. **Run the UI verification this machine could not.** `scripts/Verify-WindowsRelease.ps1` on an
   idle interactive desktop, without `-SkipWpfSmoke`. Focus assertions reactivate the window and
   retry, so treat a lone focus failure as environmental and rerun before calling it a regression.
   This is the gate that makes "stable" mean something.

4. **Validate the CI workflow.** `.github/workflows/ci.yml` exists but has never run, so it is
   limited to manual dispatch. Run it from the Actions tab, fix what the runner needs — RocksDB's
   bindgen wanting `LIBCLANG_PATH` is the likely first failure — then enable the `push` and
   `pull_request` triggers that are commented out at the top, and delete the notice. If it proves
   more trouble than it is worth, delete the file; it should not be a merge blocker.

5. **Close or consciously defer the open items** listed below.

6. **Open the pull request into `master`.** Fast-forward or a merge commit, not a squash: the 236
   commit messages carry reasoning the diff does not. The description should cover the WPF app, the
   engine and worker work, the toolchain bump, and — importantly for anyone else building `master`
   afterwards — that it now requires `rustup update stable`.

7. **After the merge**, delete this file, delete the stale `codex/core-rust-docs-clean-slate`
   branch (it sits at `54c7d48`, identical to `master`), keep `wpf-poc` at `deefa40`, and consider
   tagging `v0.1.0` to match `Directory.Build.props`.

## Open items

None of these block the merge.

1. **`MoveLocationCardSelection` is dead production code.**
   `apps/windows/src/SuperDuper.Windows/Views/DuplicateFoldersView.xaml.cs` still maps
   Left/Right/Home/End for folder copies, but no key handler calls it; only `WpfSurfaceSmokeTests`
   does. The copies are a `DataGrid` now, so Down/Up/Ctrl+Home come from the grid. Either delete
   the method and its test, or wire it back up if the Right/Home card semantics are still wanted.
   This one needs the app in front of you, which is why it is still open.

2. **`scripts/Verify-WindowsHashReadPath.ps1` was edited but not run.** Its SOP7 assertion pointed
   at hashing code that moved out of `hasher/cache.rs`; it now asserts the hint policy in
   `hasher/xxhash.rs` and that `cache.rs` does no hashing. The script also re-verifies retained
   evidence hashes, which was not touched. Run it when an SOP7 check is next due.

3. **Other `scripts/Verify-Windows*.ps1` acceptance gates were not run.** Per `AGENTS.md` these are
   accepted gates that should not be replayed without the operator reopening that scope. Instead,
   every literal source string those scripts assert (66 of them) was checked to still exist after
   the clippy, edition and dependency rewrites.

4. **`SUPER_DUPER_WORKER_PATH` is honored in Release builds.** `WorkerExecutableLocator.Resolve()`
   takes the environment override before the deployed sibling executable, with no build guard and
   no directory restriction, and `App.xaml.cs` uses it for the production worker. Documented in
   `README.md` and low risk on a single-user machine, but if the app is ever distributed, gate it
   behind `#if DEBUG` or require a path inside the install directory.

5. **New warning code `exact_folder_hash_cache_warning`** (documented in
   `docs/worker-protocol-v1.md`) separates "verified, but the hash cache degraded" from
   `exact_folder_verification_warning`, which now means only "omitted". The Windows app gives
   navigation affordances to `hash_recoverable_warning` only; the new code currently renders as a
   plain aggregate row. Surfacing it is optional.

6. **Switching saved scans always prompts "Save setup changes before leaving?"** on a machine whose
   registered cloud locations differ from the saved definition, because detection marks Setup dirty
   without any operator edit. That is the app behaving as designed, and the smoke answers Discard,
   but it is worth confirming it is the intended experience.

7. **`resolver` stays `"2"`** even though edition 2024 defaults to `"3"`. Left explicit so the
   edition move did not change dependency resolution in the same commit. Moving to `"3"` is a
   separate, testable change.

8. **`README.md` has not had a full editorial pass.** The factual drift was corrected (schema 15,
   .NET 10.0.400, the Rust floor, the shared repeat cache, `count-hash-cache` semantics), but the
   acceptance and status prose further down may still describe an earlier state.

## Environment notes

9. **Rust 1.98 is the declared floor** (`[workspace.package] rust-version`), and the workspace is
   edition 2024. An older stable fails with a clear rust-version message instead of an obscure
   `each_ref` error.

10. **Debug builds use `debug = "line-tables-only"`** (`[profile.dev]`). Full MSVC debug info for
    this dependency tree produced ~63 GB per debug build cycle, filled the disk, and hit the
    linker's program-database limit (LNK1140). Backtraces keep file and line numbers. If you need
    full symbols for a step-debugging session, override locally rather than reverting the default.
    `cargo clean --profile dev` is the quick reclaim.

11. **`bincode` stays on 2.0.1.** Version 3.0.0 on crates.io is a 4 KB placeholder. The stored
    encoding uses `config::legacy()` to stay byte-identical with the 1.x on-disk format;
    `stored_encoding_bytes_are_pinned` in `hasher/repeat_cache.rs` fails if that ever changes.

12. **RocksDB 0.25 (bundled 11.8) is a one-way upgrade.** A store written by it is not guaranteed
    to open under the previous bundled 8.10, so any existing `content_hash_cache.db` on the VM is
    upgraded in place the first time a scan touches it. Reading an 8.10-written store with 11.8 was
    verified.

13. **`Microsoft.NET.Test.Sdk` was dropped** when the test projects moved to the MSTest 4.4.1
    meta-package. `dotnet test` still honors `--filter`, `TestCategory=` filters, the `trx` logger,
    `--results-directory` and `--blame-hang-timeout`; those were checked explicitly because
    `scripts/Verify-*` and `Invoke-WindowsPolishJourney.ps1` depend on them.

14. **`cargo run -p super-duper-cli -- count-hash-cache` changed meaning.** It counts live cache
    entries through a read-only handle (safe while a scan holds the store) instead of every raw
    RocksDB key, so the number is smaller than before and excludes bookkeeping and pre-repeat-cache
    leftovers. `clear_all` removes those leftovers too, and fails rather than racing an open scan.

15. **`.gitattributes` now normalizes line endings** (`* text=auto`, CRLF for PowerShell and
    solution files). Adding it changed no stored content, so diffs between the two machines should
    stay quiet.

## Standing boundaries

The app stays review-only: `App.xaml.cs` registers `DisabledRecycleOperationCapabilityExecutor`,
and production Recycle Bin execution stays disabled unless the operator says otherwise. Do not
replay accepted acceptance gates or rerun consumed campaign identities; `plans/`, `docs/evidence/`
and the roadmap handoffs are records, not work queues. Point smoke and fault-injection runs at
disposable state (`SUPER_DUPER_DB_PATH`, `SUPER_DUPER_STATUS_DB_PATH`, `HASH_CACHE_PATH`), never at
real user data. Preserve `wpf-poc` at `deefa40`.
