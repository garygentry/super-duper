# Progress & Learnings

## Codebase Patterns

- Event handlers must be wired in constructor after `InitializeComponent()`, never in XAML (per XAML rules)
- `dotnet build` is not available on the Linux CI machine — C# changes can only be verified on Windows
- `App.Services` is a `ServiceProvider` which implements `IDisposable` — casting and disposing propagates to all singleton `IDisposable` registrations

## Session Log

### 001 — Dispose ServiceProvider on window close
- Added `MainWindow.Closed` handler that calls `(App.Services as IDisposable)?.Dispose()`
- Added `Debug.WriteLine` to `EngineWrapper.Dispose()` so `sd_engine_destroy()` call is visible in VS Debug Output
- No Rust changes needed — `sd_engine_destroy()` already exists in `crates/super-duper-ffi/src/actions.rs:57`
- Could not run `dotnet build` on Linux — changes are syntactically trivial (event sub + handler + Debug.WriteLine)
