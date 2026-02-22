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

### 009 — Forward PropertyName in ScanProgressOverlay instead of broadcasting all 7 properties
- Replaced anonymous lambda `(_, _) => NotifyAllProperties()` with named handler `Service_PropertyChanged`
- Handler checks `e.PropertyName`: if null (bulk refresh convention), fires all 7 PropertyChanged events; otherwise forwards the single `PropertyChangedEventArgs` directly
- ScanService uses CommunityToolkit.Mvvm `[ObservableProperty]` which always sets a specific PropertyName — so the else branch (single forward) is the hot path during scanning
- Net effect: each progress tick fires 1 PropertyChanged instead of 7 (~7x reduction)
- Removed unused `using System.Runtime.CompilerServices`
- One file changed, no Rust changes

### 010 — Replace FFI page query in RefreshMetricsAsync with a SQL aggregate query
- Added `GetSessionMetricsAsync(long sessionId)` to `IDatabaseService` returning `(int GroupCount, long WastedBytes)`
- `DatabaseService` implements with `SELECT COUNT(*), COALESCE(SUM(wasted_bytes), 0) FROM duplicate_group WHERE session_id = $sessionId`
- `DashboardViewModel.RefreshMetricsAsync` now calls `_db.GetSessionMetricsAsync(sessionId)` instead of `_engine.QueryDuplicateGroups(0, 500)`
- Eliminates marshalling up to 500 `DuplicateGroupInfo` structs across FFI boundary just to sum two scalars
- `QueryDuplicateGroups` reference fully removed from `DashboardViewModel` — no remaining uses in that file
- Three files changed, no Rust changes. `dotnet build` unavailable on Linux — changes are minimal and follow existing `DatabaseService` patterns exactly

### 011 — Implement ComparisonPane.LoadCopiesAsync to populate DuplicateCard ItemsRepeater
- Added `QueryFilesInGroupAsync(long groupId)` to `IDatabaseService` / `DatabaseService` — SQL joins `duplicate_group_member` → `scanned_file` with copy count subquery, returns `IReadOnlyList<DbFileInfo>`
- `ComparisonPane` now resolves `DriveColorService`, `IUndoService`, `IShellIntegrationService` from DI alongside existing `IDatabaseService` and `SuggestionEngine`
- `LoadCopiesAsync` flow: query group files → determine newest/oldest by modified date → run `SuggestionEngine.Suggest()` for InfoBar → fetch existing decisions per file → create `DuplicateCardViewModel` instances → set `CardsRepeater.ItemsSource`
- Added `ItemTemplate` with `<controls:DuplicateCard />` DataTemplate to XAML; wired `ElementPrepared` event to call `card.Bind(vm)` — consistent with `FileListControl.FileRepeater_ElementPrepared` pattern
- Created/Accessed dates passed as empty strings — `scanned_file` table only has `last_modified`
- Non-duplicate files (`GroupId == 0`) show "This file has no duplicates." in InfoBar; null selection clears cards and hides InfoBar
- Replaced bare `_ =` with `FireAndForget()` for consistency with item 004 pattern
- Four files changed, no Rust changes. `dotnet build` unavailable on Linux

### 012 — Implement DirectoryDiffDialog.LoadAsync left/right file list population
- Reused `ComparisonFileItem` and `MatchStatus` from `DirectoriesViewModel` — same hash-based comparison logic
- `LoadAsync` resolves `IDatabaseService` and `ScanService` from DI via `App.Services.GetRequiredService<T>()`
- Queries files from both DirAPath/DirBPath using `QueryFilesInDirectoryAsync`, compares by `ContentHash` (excluding 0 = unhashed)
- Updated XAML DataTemplates: each file entry shows a 3px colored status bar (via `StatusBrush`), file name (Consolas), and formatted size
- Added `LeftEmptyText`/`RightEmptyText` TextBlocks (Collapsed by default) for empty directory state
- Summary bar updated with counts: "N shared · N left-only · N right-only"
- Two files changed, no Rust changes. `dotnet build` unavailable on Linux
