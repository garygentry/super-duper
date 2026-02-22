# Ralph — Per-Iteration Instructions

<!-- ralph:managed:start -->

## Verification Commands

This project has two isolated sub-systems. Run only the commands relevant to the files you changed.

### Rust changes (`crates/`)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

- **Build:** `cargo build --workspace`
- **Test:** `cargo test --workspace`
- **Lint:** `cargo clippy --workspace -- -D warnings`
- **Format:** `cargo fmt --check`

### Windows / C# changes (`ui/windows/`)

```bash
cd ui/windows/SuperDuper && dotnet build
```

- **Build:** `cd ui/windows/SuperDuper && dotnet build`
- **Test:** _not configured — skip_
- **Lint/Format:** _not configured — skip_

> Note: `dotnet build` automatically triggers `cargo build -p super-duper-ffi` via the
> `BuildNativeDll` MSBuild target in `SuperDuper.csproj`. No separate FFI build step needed.

### FFI boundary changes (touching both `crates/super-duper-ffi/` and `NativeMethods/`)

Run **both** pipelines in order:

```bash
cargo build --workspace && cargo clippy --workspace -- -D warnings
cd ui/windows/SuperDuper && dotnet build
```

### UI acceptance criteria

WinUI 3 cannot run headlessly. Any acceptance criterion that requires observing UI behavior
(rendering, responsiveness, visual state) **cannot be verified automatically** in this loop.
Mark these as `RALPH_NEEDS_HUMAN:<criterion>` and describe what to look for.

<!-- ralph:managed:end -->

## Workflow

1. You are one iteration of an autonomous coding loop
2. Read `.ralph/backlog.json` — your current task is the `in_progress` item
3. Read the item's `acceptanceCriteria` — each must pass
4. Read `.ralph/progress.md` for context from previous iterations
5. Implement the task
6. Run verification: see **Verification Commands** above — use the Rust pipeline, C# pipeline, or both depending on which files you changed
7. Commit with: `[ralph] <id>: <title>`
8. Output your exit signal:
   - `RALPH_DONE` — all criteria met, verification passes
   - `RALPH_BLOCKED:<reason>` — cannot proceed, explain why
   - `RALPH_NEEDS_HUMAN:<reason>` — need human decision or input

## Important Rules

- Work on ONE item only — the current `in_progress` item
- Do NOT modify `.ralph/backlog.json` status — the loop runner manages it
- Do NOT modify `.ralph/state.json` — the loop runner manages it
- DO read `.ralph/progress.md` for accumulated learnings
- DO append new learnings to `.ralph/progress.md` if you discover important patterns
- The backlog.json file is your source of truth for what to work on
- Claude Code Tasks (if you use them internally) are your own planning — they don't affect the backlog

## Project-Specific Instructions

<!-- Add custom instructions below this line — they survive ralph update -->

### XAML rules (WinUI 3 + .NET 10)

This project uses a XAML compiler workaround. Violating these rules silently empties `.g.cs`
and breaks all x:Name bindings:

- **Never** add event handler attributes (`Click="..."`, `SelectionChanged="..."`, etc.) to
  non-DataTemplate XAML elements. Wire them in the constructor after `InitializeComponent()`.
- Use `{Binding}` not `{x:Bind}`. Do not use `x:DataType` on DataTemplates.
- DataTemplate-level event attributes are safe in XAML.
- All `Application.Resources` must live in `App.xaml`, never set in code-behind.

See `ui/windows/SuperDuper/CLAUDE.md` for full XAML conventions.

### DI and service resolution

- ViewModels are registered in `App.ConfigureServices()` and resolved via
  `App.Services.GetRequiredService<T>()`.
- `EngineWrapper` is a singleton IDisposable wrapping the native FFI handle. Do not
  instantiate it with `new` — always resolve from DI.

### Build working directory

All `cargo` commands should be run from the repo root (`d:/dev/super-duper`).
`dotnet build` must be run from `ui/windows/SuperDuper/` (or pass the project path).
