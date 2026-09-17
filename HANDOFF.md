# HANDOFF

Loose ends and recommended next actions from the 2026-09-17 sessions on `codex/ui-redesign`: the
branch review that preceded them, the hash-cache consolidation and the follow-on toolchain,
dependency and Windows-smoke stabilization (`8cfd2a8..4cb9a50`), the parallel deletion-plan
hardening (`ae3658d`), and the documentation pass that closed them out.

Everything is verified: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets`
clean, `cargo test --workspace` (250 passing), `dotnet test -m:1` (228 + 80 + 4, six opt-in skips),
and `scripts/Verify-WindowsRelease.ps1` green end to end including the WPF UI-automation smoke.

Items 1 and 3 are resolved and struck through; the rest are open. Sections are grouped by what the
next session has to decide, know, or run.

## Needs a decision or an owner

1. ~~**`crates/super-duper-core/tests/storage_tests.rs` is not rustfmt-clean.**~~ **Resolved.** The
   uncommitted edits were a Rust 1.76 workaround (`each_ref` replaced by `iter().collect()`) from
   the review session that preceded this work; the 1.98 floor makes it unnecessary, so the file was
   restored, formatted with `cargo fmt --all`, and given the same scoped
   `#[allow(clippy::assertions_on_constants)]` as `sop10_scale_tests.rs`. `cargo fmt --all --check`,
   `cargo clippy --workspace --all-targets` and `cargo test --workspace` (250 passing) are clean.

2. **`MoveLocationCardSelection` is dead production code.**
   `apps/windows/src/SuperDuper.Windows/Views/DuplicateFoldersView.xaml.cs` still maps
   Left/Right/Home/End for folder copies, but no key handler calls it; only
   `WpfSurfaceSmokeTests` does. The copies are a `DataGrid` now, so Down/Up/Ctrl+Home come from
   the grid. Either delete the method and its test, or wire it back up if the Right/Home card
   semantics are still wanted. The smoke journey was updated to the grid's actual behavior.

3. ~~**Nothing is pushed.**~~ **Resolved.** Those 24 commits — the 23 from this session plus
   `ae3658d` ("Harden legacy deletion plan marking and execution") from the parallel effort — were
   pushed to `origin/codex/ui-redesign` together with the documentation pass described under
   "Documentation pass" below.

   **Still open: the branch has not been merged.** The session that commissioned both fix threads
   was reviewing `codex/ui-redesign` for a merge into `master`. `master` has not moved since the
   branch started, so the merge is still a fast-forward. It needs an explicit operator decision.

## Environment notes for other machines

4. **Rust 1.98 is now the declared floor** (`[workspace.package] rust-version`). Any other
   checkout needs `rustup update stable`; an older stable now fails with a clear rust-version
   message instead of an obscure `each_ref` error.

5. **Debug builds now use `debug = "line-tables-only"`** (`[profile.dev]` in `Cargo.toml`).
   Full MSVC debug info for this dependency tree produced ~63 GB per debug build cycle, filled the
   disk mid-build, and hit the linker's program-database limit (LNK1140). Backtraces keep file and
   line numbers. If someone needs full local symbols for a step-debugging session, override it in
   `.cargo/config.toml` or a profile override rather than reverting the default.

6. **Disk pressure is the recurring failure mode.** `target/` reached ~67 GB this session and the
   drive hit 100%. `cargo clean --profile dev` is the quick reclaim.

## Deferred upgrade follow-ups

7. **`resolver` stays `"2"`** even though edition 2024 defaults to `"3"`. Left explicit so the
   edition move did not change dependency resolution in the same commit. Moving to `"3"` (MSRV
   aware) is a separate, testable change.

8. **`bincode` stays on 2.0.1.** Version 3.0.0 on crates.io is a 4 KB placeholder with no
   dependencies or features. The stored encoding uses `config::legacy()` to stay byte-identical
   with the 1.x on-disk format; `stored_encoding_bytes_are_pinned` in `hasher/repeat_cache.rs`
   fails if that ever changes.

9. **RocksDB 0.25 (bundled 11.8) is a one-way upgrade.** A store written by it is not guaranteed
   to open under the previous bundled 8.10, so rolling back the crate means discarding existing
   `content_hash_cache.db` stores. Reading an 8.10-written store with 11.8 was verified.

10. **`Microsoft.NET.Test.Sdk` was dropped** when the test projects moved to the MSTest 4.4.1
    meta-package. `dotnet test` still honors `--filter`, `TestCategory=` filters, the `trx` logger,
    `--results-directory` and `--blame-hang-timeout`; those were checked explicitly because
    `scripts/Verify-*` and `Invoke-WindowsPolishJourney.ps1` depend on them.

## Verification gaps

11. **`scripts/Verify-WindowsHashReadPath.ps1` was edited but not run.** Its SOP7 assertion
    pointed at hashing code that moved out of `hasher/cache.rs`; it now asserts the hint policy in
    `hasher/xxhash.rs` and that `cache.rs` does no hashing. The script also re-verifies retained
    evidence hashes, which this session did not touch. Run it when an SOP7 check is next due.

12. **Other `scripts/Verify-Windows*.ps1` acceptance gates were not run.** Per `AGENTS.md` these
    are accepted gates that should not be replayed without the operator reopening that scope. What
    was done instead: every literal source string those scripts assert (66 of them) was checked to
    still exist after the clippy, edition, and dependency rewrites.

13. **The WPF smoke needs an uninterrupted desktop.** The journey opens real Explorer windows and
    asserts keyboard focus, so it is unsuitable while someone is using the machine. Focus
    assertions now reactivate the window and retry, and three consecutive full runs passed, but
    treat a lone focus failure as environmental before treating it as a regression.

## Product follow-ups worth considering

14. **New warning code `exact_folder_hash_cache_warning`** (documented in
    `docs/worker-protocol-v1.md`) separates "verified, but the hash cache degraded" from
    `exact_folder_verification_warning`, which now means only "omitted". The Windows app gives
    navigation affordances to `hash_recoverable_warning` only; the new code currently renders as a
    plain aggregate row. Surfacing it is optional.

15. **Switching saved scans always prompts "Save setup changes before leaving?"** on a machine
    whose registered cloud locations differ from the saved definition, because detection marks
    Setup dirty without any operator edit. That is the app behaving as designed, and the smoke now
    answers Discard, but it is worth confirming it is the intended experience for operators.

16. **`cargo run -p super-duper-cli -- count-hash-cache` changed meaning.** It counts live cache
    entries through a read-only handle (safe while a scan holds the store) instead of every raw
    RocksDB key, so the number is smaller than before and excludes bookkeeping and pre-repeat-cache
    leftovers. `clear_all` removes those leftovers too, and fails rather than racing an open scan.

17. **`SUPER_DUPER_WORKER_PATH` is honored in Release builds.** `WorkerExecutableLocator.Resolve()`
    takes the environment override before the deployed sibling executable, with no build guard and
    no directory restriction, and `App.xaml.cs` uses it for the production worker. It is documented
    in `README.md` and is low risk (anyone who can set the variable can generally do worse), but if
    the app is ever shipped beyond this machine, gate it behind `#if DEBUG` or require a path inside
    the install directory.

## Documentation pass

The review session that commissioned these two fix threads also rewrote the agent-facing docs, and
this file was updated in the same commit as that work.

18. **`AGENTS.md` is now the single shared guide**, and `CLAUDE.md` imports it with `@AGENTS.md` so
    the two cannot drift. It carries the repository layout, the Rust-before-.NET build order, the
    architecture and layering rules, the safety invariants, storage and environment variables, and
    the toolchain facts from this session (edition 2024, the 1.98 floor, fmt/clippy expectations,
    MSTest 4 without `Microsoft.NET.Test.Sdk`, the `line-tables-only` debug profile, the pinned
    dependency reasons, and the single-hash-cache-store rule). Before this it still claimed the
    branch contained no Windows app. `crates/CLAUDE.md` covers the Rust modules and crates.

19. **`README.md` drift was corrected**: schema version 6 to 15, .NET SDK 10.0.303 to the 10.0.400
    that `global.json` pins, the Rust 1.98 floor, the shared repeat-cache description, and the new
    `count-hash-cache` semantics. It has not had a full editorial pass; the acceptance and status
    prose further down may still describe an earlier state.

20. **`Super Duper.lnk` was untracked** (`git rm --cached`, and `*.lnk` added to `.gitignore`). It
    was a machine-specific shortcut to `artifacts/windows-x64/SuperDuper.Windows.exe`, a gitignored
    publish directory. The file is untouched on disk, so the local launcher still works.

## Picking this up in a new sandbox session

21. **Prepare the machine before anything else.** Rust stable 1.98 or newer (`rustup update
    stable`), the .NET SDK `global.json` pins, a Windows 11 SDK targeting `10.0.22000.0`, VS C++
    x64 build tools, and VS Clang with `LIBCLANG_PATH` set for RocksDB's bindgen. Budget disk
    generously: a debug cycle is about 7 GB now, but a full Debug plus Release matrix with the
    .NET publish still runs into the tens of GB, and running out of disk mid-link is the failure
    this project hits most often.

22. **Establish a baseline before changing anything**, so a later failure is attributable:

    ```powershell
    cargo fmt --all --check
    cargo clippy --workspace --all-targets
    cargo test --workspace
    dotnet build apps/windows/SuperDuper.Windows.sln
    dotnet test apps/windows/SuperDuper.Windows.sln -m:1
    ```

    Expect 250 Rust tests and 228 + 80 + 4 .NET tests with six opt-in skips, and no warnings. Keep
    `-m:1`: the STA smoke suite and the Infrastructure host starve each other in parallel.

23. **Give the WPF smoke an idle desktop.** It drives the real app through UI Automation and opens
    real Explorer windows, so it needs an interactive session nobody else is using, and it fails in
    confusing ways over a backgrounded or locked remote desktop. Focus assertions reactivate the
    window and retry, so treat a lone focus failure as environmental and rerun before investigating
    it as a regression. `scripts/Verify-WindowsRelease.ps1` is the full gate, and
    `docs/windows-ui-dev-session.md` covers what can still be verified when native input is
    unavailable.

24. **Suggested order of work**, smallest first: the dead `MoveLocationCardSelection` decision (2),
    then `scripts/Verify-WindowsHashReadPath.ps1` (11) when an SOP7 check is next due, then the
    resolver `"3"` move (7) as its own commit with the full matrix, then the two optional product
    questions (14, 15). Item 17 only matters if distribution is on the table.

25. **Keep the standing boundaries.** The app stays review-only: `App.xaml.cs` registers
    `DisabledRecycleOperationCapabilityExecutor`, and production Recycle Bin execution stays
    disabled unless the operator says otherwise. Do not replay accepted acceptance gates or rerun
    consumed campaign identities; historical plans and evidence under `plans/`, `docs/evidence/`
    and the roadmap handoffs are records, not work queues. Point smoke and fault-injection runs at
    disposable state (`SUPER_DUPER_DB_PATH`, `SUPER_DUPER_STATUS_DB_PATH`, `HASH_CACHE_PATH`),
    never at real user data.
