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

### 002 — Add CancellationToken + await to prevent Dispose race during active scan
- `ScanService.StartScanAsync` now tracks completion via `TaskCompletionSource` stored in `_activeScanTask`
- `CancelAndWaitAsync()` calls `_engine.CancelScan()` (swallowing errors) then awaits `_activeScanTask`, ensuring the entire `StartScanAsync` (including its `finally` cleanup) completes before returning
- `MainWindow.Closed` uses `args.Handled = true` pattern to defer window close, await `CancelAndWaitAsync()`, Dispose services, then call `this.Close()` — `_isClosing` flag prevents re-entry on the second close
- `TaskCreationOptions.RunContinuationsAsynchronously` prevents continuations from running inline on the `finally` thread
- `clippy` and `fmt` not installed on this Linux machine — only `cargo build` + `cargo test` verified (no Rust changes anyway)

### 003 — Offload ExecuteDeletion() to background thread
- Renamed `ExecuteDeletion()` → `ExecuteDeletionAsync()` returning `Task`
- Wrapped `_engine.ExecuteDeletionPlan(useTrash)` in `Task.Run()` — captured `UseTrash` into a local before entering `Task.Run()` to avoid reading UI-bound `_settings` from background thread
- `IsExecuting` set `true` before `Task.Run`, `false` in `finally` — both on UI thread thanks to `await`
- `DeletionReviewPage.ExecuteButton_Click` (already `async void`) now `await`s the method
- All post-deletion UI updates (StatusMessage, ErrorOccurred, RefreshSummary, LoadMarkedFilesAsync) execute on UI thread after the `await` returns

### 004 — Add fault logging to all fire-and-forget task discards
- Created `TaskExtensions.FireAndForget(this Task, string)` as an extension method in `TaskExtensions.cs` at project root
- Uses `ContinueWith(OnlyOnFaulted)` + `Debug.WriteLine` — faults appear in VS Debug Output with caller context
- Replaced 9 bare `_ =` discards across 3 files: DashboardViewModel (3), DeletionReviewViewModel (3), StorageTreemap (3)
- DispatcherQueue lambda (DashboardViewModel line 263): the `TryEnqueue` lambda is non-async (`Action` delegate), so `FireAndForget()` is the correct pattern — it observes the returned Task's fault via ContinueWith without needing the lambda to be async
- Additional `_ =` sites exist in other files (FileListControl, DirectoryTreeControl, GroupsPage, MainWindow, SessionsViewModel) — not in scope for this item per acceptance criteria

### 005 — Replace brittle string-matching cancellation check with typed OperationCanceledException
- `SdResultCode.Cancelled = 7` already existed in `SuperDuperEngine.cs` — no enum changes needed
- `EngineWrapper.ThrowOnError` now checks `code == SdResultCode.Cancelled` before the general `!= Ok` check, throwing `OperationCanceledException` instead of `InvalidOperationException`
- `ScanService.StartScanAsync` now catches `OperationCanceledException` separately (silent, no toast) before the general `Exception` catch
- Removed the brittle `ex.Message.Contains("Cancelled")` string guard — typed exception is immune to Rust error message wording changes
- Two files changed, no Rust changes needed

### 006 — Register DeletionReviewViewModel and SessionsViewModel in DI container
- Both ViewModels now accept required dependencies via constructor (`EngineWrapper` + `SettingsService` for DeletionReviewViewModel, `EngineWrapper` for SessionsViewModel)
- `Initialize()` methods removed from both — constructor performs initialization (RefreshSummary + LoadMarkedFilesAsync for deletion, DatabasePath + LoadSessionsAsync for sessions)
- All `if (_engine == null) return` / `if (_engine is null) return` null guards removed (6 total: 4 in DeletionReviewViewModel, 2 in SessionsViewModel)
- Both registered as `AddTransient<T>()` in `App.ConfigureServices()` — consistent with existing ViewModel registrations
- Pages resolve via `App.Services.GetRequiredService<T>()` in constructor (matching existing page patterns like DashboardPage, ExplorerPage)
- `DeletionReviewPage.OnNavigatedTo` removed entirely — no longer needed since ViewModel is fully initialized by DI
- `SettingsPage.OnNavigatedTo` retained — it triggers `LoadSessionsCommand.Execute(null)` to refresh sessions on each navigation
- `_ = LoadSessionsAsync()` in `SessionsViewModel` constructor replaced with `FireAndForget()` for fault observability (consistent with item 004 pattern)

### 007 — Replace visual-tree traversal in FileListControl.WireRadioButtons() with x:Name wiring
- Added `x:Name="AllFilesButton"` and `x:Name="DupesOnlyButton"` to the two RadioButtons in FileListControl.xaml
- Replaced `this.Loaded += (_, _) => WireRadioButtons()` with direct `AllFilesButton.Checked += AllFiles_Checked` / `DupesOnlyButton.Checked += DupesOnly_Checked` in constructor after `InitializeComponent()`
- Deleted `WireRadioButtons()` method and `FindChildren<T>()` VisualTreeHelper traversal helper (25 lines removed)
- Removed unused `using Microsoft.UI.Xaml.Media` import
- Net result: -25 lines, eliminates fragile visual-tree traversal that could break on XAML nesting changes

### 008 — Add Debug.WriteLine to all silent catch blocks
- Added `Debug.WriteLine` with `[ClassName.MethodName]` prefix to all bare catch blocks across 5 files
- ScanService: `TrySetActiveSession` catch + `CancelAndWaitAsync` catch (added `using System.Diagnostics`)
- MarkedFileViewModel: `RevealInExplorer` catch (already had `using System.Diagnostics` via parent file)
- SettingsService: `Load()` + `Save()` catches (added `using System.Diagnostics`). No ILogger — project has no `Microsoft.Extensions.Logging` infrastructure
- WindowsShellService: `RevealInExplorer`, `OpenFile`, `RegisterContextMenu`, `UnregisterContextMenu` catches (already had `using System.Diagnostics`)
- SessionsViewModel: `LoadSessionsAsync` catch (added `using System.Diagnostics`)
- All catches changed from bare `catch` / `catch { }` to `catch (Exception ex)` with `Debug.WriteLine($"[Context] {ex}")`
- No behavioral changes — all exceptions still swallowed, just now visible in VS Debug Output
