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

### 013 — Disable ScanDialog advanced options that are silently ignored
- Three controls in `ScanDialog.xaml` Step 2 (Options pivot): `NumberBox` (MinFileSize), `ComboBox` (HashAlgorithm), `Slider` (CpuThreads)
- Added `IsEnabled="False"` and appended `(coming soon)` to each `Header` attribute
- `IncludeHiddenFiles` ToggleSwitch left enabled — not in scope (it may actually be passed through)
- Pure XAML-only change, no code-behind or ViewModel modifications needed
- One file changed, no Rust changes. `dotnet build` unavailable on Linux

### 014 — Apply debounce guard to StorageTreemap SizeChanged to prevent concurrent renders
- `TreemapCanvas_SizeChanged` now checks `_renderPending` before scheduling a render — same pattern as `OnItemsCollectionChanged`
- Uses `DispatcherQueue.TryEnqueue` to coalesce rapid resize events into a single render at the end of the message pump batch
- `_renderPending` is set `true` immediately on first SizeChanged, reset `false` inside the dispatched callback before calling `RenderAsync()`
- Prevents multiple concurrent `Task.Run` squarify computations racing to write `TreemapCanvas.Children`
- One file changed (+9 lines, -1 line), no Rust changes. `dotnet build` unavailable on Linux

### 015 — Replace outer ItemsRepeater in GroupsPage with a virtualizing ListView
- Root cause: `ItemsRepeater` inside `StackPanel` inside `ScrollViewer` — the `StackPanel` gives unconstrained height, defeating virtualization (all items realized regardless of viewport)
- Replaced `ScrollViewer` > `StackPanel` > `ItemsRepeater` with `ListView` (built-in virtualization via `VirtualizingStackPanel`)
- `SelectionMode="None"` disables selection behavior since groups aren't selectable
- `ItemContainerStyle` sets `HorizontalContentAlignment="Stretch"`, `Padding="0"`, `Margin="0,2,0,2"` (2+2=4px gap, matching old `StackLayout Spacing="4"`)
- "Load More" button moved to `ListView.Footer` — scrolls into view after last item, same UX as before
- Inner per-file `ItemsRepeater` (inside each Expander) left unchanged — bounded by group size and lower priority
- `GroupsRepeater` x:Name not referenced in code-behind — no C# changes needed
- XAML-only change, one file. `dotnet build` unavailable on Linux

### 016 — Pin DLL load path with NativeLibrary.SetDllImportResolver
- Registered `NativeLibrary.SetDllImportResolver` in `App()` constructor for `typeof(SuperDuperEngine).Assembly`
- Resolver checks `libraryName == "super_duper_ffi"` and loads from `Path.Combine(AppContext.BaseDirectory, "super_duper_ffi.dll")`
- Returns `IntPtr.Zero` for any other library name (falls back to default resolution)
- Placed before `InitializeComponent()` to ensure resolver is active before any P/Invoke could trigger
- Used `static` lambda since no captured state — avoids unnecessary closure allocation
- One file changed (`App.xaml.cs`), no Rust changes. `dotnet build` unavailable on Linux

### 017 — Map drive color stripes to distinct High Contrast system colors
- Previously DriveColor0-3,5 all mapped to `SystemColorHighlightColor` and DriveColor4,6 to `SystemColorHighlightTextColor` — only 3 distinct values across 8 slots
- Mapped each DriveColor to a distinct system color resource: HighlightColor, HotlightColor, GrayTextColor, WindowTextColor, ButtonTextColor, HighlightTextColor, ButtonFaceColor, WindowColor
- All 8 reference unique system color resources; in typical HC themes (HC Black, HC White) at least 4-5 are visually distinct (e.g. cyan, yellow, green, white in HC Black)
- `DriveColorService.cs` uses hardcoded Color values not XAML resources — no code-behind changes needed
- XAML-only change in `Styles/Colors.xaml` HighContrast dictionary, one file. `dotnet build` unavailable on Linux

### 018 — Consolidate duplicate FormatBytes() helpers to FileSizeConverter.FormatBytes()
- Four private `FormatBytes()` copies found: DeletionReviewViewModel (line 145), MarkedFileViewModel inner class (line 190), DirectorySimilarityInfo in EngineWrapper.cs (line 472), WindowsNotificationService (line 42)
- Task notes mentioned StorageTreemap but it has `FileSizeLabel` (different name/logic) — not a `FormatBytes` copy
- Canonical `FileSizeConverter.FormatBytes(long)` uses decimal mode by default (1000-based: KB, MB, GB); copies used 1024-based — minor formatting change accepted per task design
- DirectorySimilarityInfo appended " shared" suffix — handled at call site: `$"{FileSizeConverter.FormatBytes(SharedBytes)} shared"`
- Added `using SuperDuper.Converters;` to all three changed files
- Three files changed, -30 net lines. `dotnet build` unavailable on Linux

### 019 — Centralize magic file type filter strings into FileTypeFilters constants class
- Created `Models/FileTypeFilters.cs` with `const string` fields: Images, Documents, Video, Audio, Archives
- `DatabaseService.QueryGroupsFilteredAsync` switch cases now use `FileTypeFilters.Images` etc. via `using static`
- `GroupsPage.xaml.cs` `WireNonVisualTreeEvents()` sets `MenuFlyoutItem.Tag` from `FileTypeFilters` constants instead of relying on XAML `Tag` attributes
- Removed `Tag="..."` attributes from the five filter `MenuFlyoutItem`s in `GroupsPage.xaml` — Tags are set in code-behind
- Task notes referenced `GroupsViewModel ~line 106` but that line is the ReviewStatus switch; GroupsViewModel has no file type string literals — the actual UI source was `GroupsPage.xaml` Tag attributes
- Four files changed (1 new, 3 modified). `dotnet build` unavailable on Linux
