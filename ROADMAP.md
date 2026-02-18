# Roadmap

This document tracks future work for Super Duper, organized by urgency. See [README.md](README.md) for architecture and getting-started documentation.

---

## Current State

The core pipeline is complete and functional:

- Two-tier hashing (size bucket → 1 KB partial → full XxHash64) with RocksDB cache
- SQLite session storage with WAL mode, 64 MB page cache, 256 MB mmap
- Directory fingerprinting (bottom-up XxHash64 of sorted child hash sets)
- Jaccard similarity with inverted-index candidate generation
- FFI: handle-based design, paginated queries, thread-local error detail
- WinUI 3 UI: dashboard, duplicate groups, directory comparison, deletion review, settings, session history
- CLI: `process`, `analyze-directories`, `count-hash-cache`, `print-config`, `truncate-db`

---

## Now — Safety & Correctness

These items block confident daily use. No existing feature compensates for them.

### 1. Recycle Bin deletion

**Why it matters**: `fs::remove_file` in `analysis/deletion_plan.rs` permanently deletes files with no recovery path. A single mistaken auto-mark wipes data forever.

**Implementation**: Introduce the [`trash`](https://crates.io/crates/trash) crate (cross-platform) or call the Windows Shell `SHFileOperation` API via `platform/windows.rs`. Add a `use_trash: bool` flag to `execute_deletion_plan()` defaulting to `true`. Surface as a Settings toggle ("Move to Recycle Bin" vs "Permanently delete").

**Files**: `crates/super-duper-core/src/analysis/deletion_plan.rs`, `crates/super-duper-core/src/platform/windows.rs`, `ui/windows/SuperDuper/Views/SettingsPage.xaml`

---

### 2. Reveal in Explorer / Open file

**Why it matters**: Users identifying duplicates need to inspect files in context before deciding what to keep. There is currently no way to open a file or its folder from the UI.

**Implementation**: Add a context menu or icon button on each file row in DuplicateGroupsPage and DeletionReviewPage. Use `Process.Start("explorer.exe", $"/select,\"{path}\"")` in the code-behind to highlight the file in Explorer.

**Files**: `ui/windows/SuperDuper/Views/` (DuplicateGroupsPage, DeletionReviewPage)

---

### 3. Scan-in-progress UI guard

**Why it matters**: Double-clicking "Start Scan" while a scan is running submits a second scan request. The FFI layer blocks re-entry, but the UI gives no feedback and the button remains clickable.

**Implementation**: Bind the Start Scan button's `IsEnabled` to `ViewModel.IsNotScanning` — a property already implied by the existing `IsScanning` flag. No new logic required; just the binding.

**Files**: `ui/windows/SuperDuper/Views/MainPage.xaml`, `ui/windows/SuperDuper/ViewModels/MainViewModel.cs`

---

### 4. Shared bytes accuracy in directory similarity

**Why it matters**: The "shared bytes" figure shown on the Directory Comparison page is documented as approximate (`dir_similarity.rs` line 110). It currently counts shared file hashes without consulting actual file sizes, making the displayed savings misleading for groups of large files.

**Implementation**: Join `directory_fingerprint.hash_set` with `scanned_file.file_size` at query time in `storage/queries.rs`. All required data is already present; this is a query change only.

**Files**: `crates/super-duper-core/src/analysis/dir_similarity.rs`, `crates/super-duper-core/src/storage/queries.rs`

---

## Soon — Workflow & Usability

These items add missing workflow steps that users will hit on first real use.

### 5. Auto-mark strategy picker

**Why it matters**: The current auto-mark strategy ("keep first alphabetically") is hardcoded. Users with date-structured archives want to keep the oldest copy; users with preferred archive paths want to keep files under a specific prefix.

**Implementation**: Define a `KeepStrategy` enum in core (`FirstAlpha`, `Newest`, `Oldest`, `PreferredPath(prefix)`). Extend `auto_mark_duplicates()` to accept it. Add a `strategy: u32` parameter to `sd_auto_mark_for_deletion` in the FFI. Surface as a dropdown on the Deletion Review page next to the Auto-Mark button.

**Files**: `crates/super-duper-core/src/analysis/deletion_plan.rs`, `crates/super-duper-ffi/src/actions.rs`, `ui/windows/SuperDuper/Views/` (DeletionReviewPage)

---

### 6. Filter & sort in Duplicate Groups

**Why it matters**: Large scans produce hundreds of duplicate groups. Without filtering, users scroll through noise (tiny files, known-ignorable types) to find high-value targets.

**Implementation**: Add a filter bar above the groups list: minimum wasted size (slider), file extension filter (text field), filename search. All filtering operates client-side on loaded groups — no new FFI needed for basic cases. Add sort options: wasted bytes (current default), file count, individual file size.

**Files**: `ui/windows/SuperDuper/Views/` (DuplicateGroupsPage), `ui/windows/SuperDuper/ViewModels/` (DuplicateGroupsViewModel)

---

### 7. Export results

**Why it matters**: Power users want to script follow-up actions (move files, build reports) on the duplicate list without re-running a scan. There is currently no way to extract data from the database without a SQLite client.

**Implementation**:
- CLI: add an `export` subcommand to `super-duper-cli` writing the current session's duplicate groups to stdout as CSV or JSON (`--format csv|json`)
- UI (optional): add an "Export" button to the Duplicate Groups page that opens a `FileSavePicker` and writes CSV/JSON

**Files**: `crates/super-duper-cli/src/commands.rs`, `crates/super-duper-cli/src/main.rs`, `crates/super-duper-core/src/storage/queries.rs`

---

### 8. CLI deletion command

**Why it matters**: The CLI can detect duplicates but cannot act on them. Users running headless or in CI have no way to execute a reviewed deletion plan without launching the Windows UI.

**Implementation**: Add a `delete` subcommand with a `--dry-run` flag that prints the plan without acting, and a standard mode that prompts for terminal confirmation before calling `execute_deletion_plan()`.

**Files**: `crates/super-duper-cli/src/commands.rs`, `crates/super-duper-cli/src/main.rs`

---

### 9. Configurable similarity thresholds

**Why it matters**: The Jaccard threshold (0.5) and noise cutoff (50 directories) are hardcoded in `dir_similarity.rs`. A threshold of 0.5 misses near-duplicates at 0.4; the noise cutoff of 50 is arbitrary and may exclude valid candidates in large trees.

**Implementation**: Add `min_jaccard_score: f64` and `max_hash_frequency: usize` fields to `AppConfig` in `config.rs`. Update `analyze_directory_similarity()` to read them. Add Settings UI sliders. Running `analyze-directories` picks up the new values automatically.

**Files**: `crates/super-duper-core/src/config.rs`, `crates/super-duper-core/src/analysis/dir_similarity.rs`, `ui/windows/SuperDuper/Views/SettingsPage.xaml`

---

### 10. Drag-and-drop path addition

**Why it matters**: Users want to drag folders from Explorer onto the scan path list rather than type or paste paths. This is a standard Windows UX expectation.

**Implementation**: Subscribe to `DragOver` and `Drop` on the scan paths `ListView` in `MainPage.xaml`. Extract `StorageFolder` items from `DragEventArgs.DataView` and add their paths to the scan list.

**Files**: `ui/windows/SuperDuper/Views/MainPage.xaml`, `ui/windows/SuperDuper/Views/MainPage.xaml.cs`

---

## Later — Performance & Scale

These items become important as file libraries grow into the millions.

### 11. Hash cache eviction

**Why it matters**: The RocksDB hash cache grows without bound. Users with large file libraries will accumulate stale entries for files that no longer exist, consuming disk space indefinitely.

**Implementation**: Add an `sd_trim_hash_cache(max_entries: u64)` FFI function. Iterate the RocksDB column family and delete entries beyond the cap, approximating LRU with an insertion-order key prefix or a separate timestamp column. Show the entry count in Settings with a "Trim" button (the count query already exists: `count-hash-cache` CLI command).

**Files**: `crates/super-duper-core/src/hasher/cache.rs`, `crates/super-duper-ffi/src/actions.rs`, `ui/windows/SuperDuper/Views/SettingsPage.xaml`

---

### 12. Incremental scan (dirty-directory detection)

**Why it matters**: Every scan re-walks all configured paths. For large stable archives (say, 500 GB of unchanged video files), this wastes significant time re-checking files that haven't changed since the last scan.

**Implementation**: Record directory `mtime` in the `directory_node` table. On re-scan, compare current `mtime` against the stored value; skip subtrees where it hasn't changed. This is the single biggest performance win for users with large, mostly-static archives.

**Files**: `crates/super-duper-core/src/storage/schema.sql`, `crates/super-duper-core/src/storage/queries.rs`, `crates/super-duper-core/src/scanner/walk.rs`

---

### 13. Virtual scrolling

**Why it matters**: The "Load More" button requires manual interaction to page through results. For sessions with thousands of duplicate groups, the experience is tedious.

**Implementation**: Replace the `ListView + Load More` pattern with `ItemsRepeater` driven by scroll position. `sd_query_duplicate_groups` already returns `TotalAvailable`; a scroll-triggered load threshold is the only missing piece.

**Files**: `ui/windows/SuperDuper/Views/` (DuplicateGroupsPage), `ui/windows/SuperDuper/ViewModels/` (DuplicateGroupsViewModel)

---

### 14. CLI JSON output

**Why it matters**: Piping `process` output into `jq` or other tools is blocked because the CLI currently emits only human-readable log lines. Structured output is essential for scripting.

**Implementation**: Add `--format json` to the `process` and `export` subcommands. Emit a JSON object to stdout: `{ "session": {...}, "groups": [...] }`. Keep the default as human-readable.

**Files**: `crates/super-duper-cli/src/commands.rs`, `crates/super-duper-cli/src/main.rs`

---

### 15. Async scan FFI

**Why it matters**: `sd_scan_start()` blocks the calling thread. The current C# workaround (`Task.Run` + callback marshalling to the UI dispatcher) is functional but adds complexity and makes it harder to cancel a running scan.

**Implementation**: Add `sd_scan_start_async(handle, on_complete_callback)` that returns immediately and delivers the result via callback on a background thread. Add `sd_scan_cancel()` to request early termination. The synchronous variant stays for CLI use.

**Files**: `crates/super-duper-ffi/src/actions.rs`, `crates/super-duper-ffi/src/callbacks.rs`, `ui/windows/SuperDuper/NativeMethods/EngineWrapper.cs`

---

## Someday — Platform & Advanced Analysis

These items expand scope significantly. Each is a meaningful project in its own right.

### 16. macOS / Linux UI

**Why it matters**: The FFI crate already compiles as a shared library on Linux and macOS. A cross-platform frontend would make Super Duper useful on non-Windows machines without any core changes.

**Implementation**: [Tauri](https://tauri.app/) (Rust + web frontend) would reuse the FFI directly and produce a native binary for all three platforms. Alternatives: MAUI, Flutter, or [Slint](https://slint.dev/).

**Files**: New `ui/tauri/` directory; no core changes required.

---

### 17. File type breakdown

**Why it matters**: Aggregate "X GB wasted" figures are useful, but users want to know whether the waste is video files, document backups, or build artifacts before deciding what to act on.

**Implementation**: Add a query in `storage/queries.rs` that categorizes duplicate groups by extension group (Images, Video, Audio, Archives, Documents, Other) and returns per-category wasted bytes and file counts. Display as stat cards or a breakdown chart on the dashboard.

**Files**: `crates/super-duper-core/src/storage/queries.rs`, `crates/super-duper-ffi/src/queries.rs`, `ui/windows/SuperDuper/ViewModels/MainViewModel.cs`

---

### 18. Perceptual image deduplication

**Why it matters**: Content-identical images are caught by the existing pipeline. Near-identical images — same photo at different JPEG quality levels or with a slight crop — are not. These are common in photo library archives.

**Implementation**: Compute a [perceptual hash (pHash)](https://www.phash.org/) for image files alongside the content hash. Store in a new `image_phash` column on `scanned_file`. Compare pairs using Hamming distance ≤ N as the similarity threshold. Surface results in a new "Similar Images" page.

**Files**: `crates/super-duper-core/src/hasher/`, `crates/super-duper-core/src/storage/schema.sql`, new `ui/windows/SuperDuper/Views/SimilarImagesPage.*`

---

### 19. Near-duplicate file detection (fuzzy hashing)

**Why it matters**: `report_v1.docx` and `report_v2.docx` are not content-identical and are missed by the current pipeline. Fuzzy hashing detects modified copies of text and document files — common in working directories and email archives.

**Implementation**: Compute [TLSH](https://github.com/trendmicro/tlsh) or `ssdeep` hashes for text, document, and source-code files. Store in a new `fuzzy_hash` column. Compare pairs using the respective distance metric. Surface in a new "Similar Files" section.

**Files**: `crates/super-duper-core/src/hasher/`, `crates/super-duper-core/src/storage/schema.sql`

---

### 20. Installer / distribution

**Why it matters**: Currently requires building from source with a full Rust toolchain. A packaged installer would allow non-developer use.

**Implementation**:
- MSIX package using `<WindowsAppSDKSelfContained>true</WindowsAppSDKSelfContained>` to bundle the Windows App Runtime (no separate runtime installer required)
- winget manifest submission to the [winget-pkgs](https://github.com/microsoft/winget-pkgs) repository
- GitHub Releases CI pipeline: build → sign → publish `.msix` on tag push

**Files**: `ui/windows/SuperDuper/SuperDuper.csproj`, new `.github/workflows/release.yml`

---

*Last updated: February 2026*
