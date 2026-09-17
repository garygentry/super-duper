# HANDOFF

Loose ends and recommended next actions from the 2026-09-17 session that consolidated the hash
cache and then stabilized the toolchain, dependencies, and the Windows smoke. Scope is limited to
that work: commits `8cfd2a8..4cb9a50` on `codex/ui-redesign` (22 commits, plus the commit adding
this file), none pushed.

Everything in those commits is verified: `cargo test --workspace` (250 passing), workspace clippy
clean, `dotnet test -m:1` (228 + 80 + 4, six opt-in skips), and `scripts/Verify-WindowsRelease.ps1`
green end to end including the WPF UI-automation smoke.

## Needs a decision or an owner

1. **`crates/super-duper-core/tests/storage_tests.rs` is not rustfmt-clean.**
   That file has uncommitted edits that predate this session. `cargo fmt` touched it during the
   edition 2024 migration and it was restored byte-for-byte, so it is exactly as it was. The 2024
   style edition sorts imports differently, so `cargo fmt --all --check` fails until it is
   formatted. Run `cargo fmt` before committing that file. It also still carries one clippy
   warning (`assertions_on_constants` on the Release-only guard at its line ~670); the equivalent
   guard in `sop10_scale_tests.rs` was given an explicit `#[allow]` in this session.

2. **`MoveLocationCardSelection` is dead production code.**
   `apps/windows/src/SuperDuper.Windows/Views/DuplicateFoldersView.xaml.cs` still maps
   Left/Right/Home/End for folder copies, but no key handler calls it; only
   `WpfSurfaceSmokeTests` does. The copies are a `DataGrid` now, so Down/Up/Ctrl+Home come from
   the grid. Either delete the method and its test, or wire it back up if the Right/Home card
   semantics are still wanted. The smoke journey was updated to the grid's actual behavior.

3. **Nothing is pushed.** The branch is 24 commits ahead of `origin/codex/ui-redesign`: the 23
   from this session plus `ae3658d` ("Harden legacy deletion plan marking and execution"), which
   landed from a separate effort during the session and is not part of this work.

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
