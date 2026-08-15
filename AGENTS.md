# AGENTS.md

Guidance for fresh coding-agent sessions in this repository.

## Current State

This branch is a clean-slate Rust workspace for Super Duper. The previous Windows app implementation
has been removed, including its `ui/windows` tree, solution files, build outputs, local launch
scripts, tracked editor state, and runtime artifacts.

The repository now intentionally contains only the Rust engine, CLI, reusable FFI boundary, docs,
and config. A future Windows app should be treated as a new product surface, not a continuation of
the deleted app.

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

The last verification on this branch was `cargo test --workspace`, which passed.

## Development Notes

- Keep `super-duper-core` UI-agnostic.
- Keep `super-duper-ffi` as a stable native-client contract, not tailored to one app.
- Do not reintroduce the old Windows app structure, XAML, C# view models, services, or workarounds.
- Runtime files such as `super_duper.db`, `content_hash_cache.db`, and `logs/` should stay out of
  source control.
- The generated FFI header is `crates/super-duper-ffi/super_duper.h`; building the FFI crate may
  refresh it.

## Fresh Windows App Work

When Windows app work starts, choose a new app directory and architecture deliberately. The intended
starting point is the Rust core and FFI API, plus the product behavior described in `README.md` and
`ROADMAP.md`.
