# Super Duper — Feature Roadmap

This roadmap is organized by theme. Items within each theme are loosely prioritized top-to-bottom.

---

## 1. Duplicate Detection & Analysis

### 1.1 Fuzzy / Near-Duplicate File Matching
**Problem**: Bit-for-bit duplicate detection misses files that are functionally identical but differ by metadata — e.g. two JPEGs with different EXIF timestamps, or two MP3s with different ID3 tags.

**Approach**:
- Images: perceptual hashing (pHash / dHash) with Hamming-distance threshold; expose as a configurable second pass after exact-hash detection
- Audio: acoustic fingerprinting (Chromaprint) for music files
- Text/documents: MinHash / Locality Sensitive Hashing (LSH) for document similarity
- Configurable similarity threshold per media type

---

### 1.2 File-Type–Aware Duplicate Groups
**Problem**: The UI presents all duplicate groups identically regardless of content type. Users working through large photo or video libraries want to filter and sort by type.

**Approach**:
- Store detected MIME type in `scanned_file` during scan (via `infer` crate, magic-byte detection)
- Add type filter to Duplicate Groups page (All / Images / Video / Audio / Documents / Archives / Other)
- Sort groups by type, then by wasted bytes within type

---

### 1.3 Incremental / Watch-Mode Scanning
**Problem**: A full re-scan is required to detect changes. For large libraries this takes minutes.

**Approach**:
- Inotify/FSEvents/ReadDirectoryChangesW watcher that enqueues changed paths
- On change: re-hash only the affected file, update its `scanned_file` row, recalculate affected duplicate groups
- Expose as an optional "watch" flag in the CLI and a toggle in Settings

---

### 1.4 Archive-Aware Scanning
**Problem**: Duplicates hidden inside ZIP, 7z, or tar archives are invisible to the current scanner.

**Approach**:
- Optional second pass: for each archive file, extract a virtual file list and compute member hashes in-memory
- Store virtual members in a separate `archive_member` table with a foreign key to the parent archive file
- Report cross-archive and archive-to-filesystem duplicates distinctly in the UI

---

### 1.5 Duplicate Video Frame Detection
**Problem**: Users often have the same video in multiple resolutions or re-encoded formats. Bit-level hashing never matches these.

**Approach**:
- Extract evenly-spaced frame thumbnails with ffmpeg
- Run perceptual hash on each thumbnail
- Compare frame-hash sequences between candidate videos; threshold on sequential match rate
- Flag as "probable video duplicate" with confidence score

---

## 2. Deletion & Space Recovery

### 2.1 Intelligent Keep-Selection Rules
**Problem**: Auto-mark currently keeps the alphabetically first file in each group. Users often want a different policy.

**Proposed policies** (selectable per session):
- **Newest**: keep the most recently modified copy
- **Oldest**: keep the earliest modified copy (original)
- **Shortest path**: keep the file closest to a root directory
- **Path regex**: keep the copy whose path matches a user-supplied regular expression
- **Largest parent folder**: keep the copy in the directory with the most files (heuristic for "primary" collection)

---

### 2.2 Recycle Bin / Trash Integration
**Problem**: `fs::remove_file()` permanently deletes without recovery.

**Approach**:
- Windows: move to Recycle Bin via `SHFileOperation` (COM) before permanent delete
- macOS/Linux: move to `~/.Trash` or `XDG_DATA_HOME/Trash`
- Add "Move to Trash" vs "Delete Permanently" option in the UI; default to Trash
- Store trash path in `deletion_plan.execution_result` for potential undo

---

### 2.3 Symbolic Link / Hard Link Replacement
**Problem**: After deleting redundant copies, users still need to update references. An alternative is to replace duplicates with hard links or symbolic links, eliminating the redundant bytes while preserving all original paths.

**Approach**:
- Post-deletion: offer to create a hard link at the deleted path pointing to the kept copy
- For cross-filesystem duplicates: offer symbolic links instead
- Record the link type in `deletion_plan`

---

### 2.4 Undo / Deletion History
**Problem**: There is no way to recover from an executed deletion plan short of restoring from backup.

**Approach**:
- Before execution, snapshot the deletion plan to a timestamped log file (JSON)
- For Trash-based deletions: offer "Undo Last Plan" that moves files out of Trash and clears their `marked_deleted` flag
- Retention: keep last N plans (configurable); auto-purge older logs

---

### 2.5 Dry-Run Mode
**Problem**: Users want to see what *would* be deleted before committing.

**Approach**:
- CLI flag: `--dry-run` on `execute-deletion`
- UI: "Preview" button alongside "Execute" that shows the list without touching disk
- Print per-file would-delete log and aggregate totals

---

## 3. Directory Comparison & Organization

### 3.1 Interactive Directory Tree Browser
**Problem**: The current Directory Comparison page shows a flat list of similar pairs. There is no way to visualize where they sit in the overall tree.

**Approach**:
- Add a tree-view panel showing the directory hierarchy with color-coded similarity badges
- Click a node to see which other directories it overlaps with, and by how much
- Filter tree to show only directories above a similarity threshold

---

### 3.2 Adjustable Similarity Threshold (Live Slider)
**Problem**: The 0.5 Jaccard threshold is set at scan time. Changing it requires a full re-analyze pass.

**Approach**:
- Store all pairs with similarity ≥ 0.1 in `directory_similarity` at scan time
- The UI filters in-memory (or re-queries with a `WHERE similarity_score >=` clause) as the slider moves
- No re-scan needed; threshold adjustment is instant

---

### 3.3 Side-by-Side Directory Diff
**Problem**: When two directories are flagged as near-duplicates, there is no way to see what files are exclusive to each side.

**Approach**:
- New panel: given a selected pair, show three columns: "Only in A", "In both", "Only in B"
- "In both" shows files present (by content hash) in both trees; "Only in A/B" shows unique content
- Allow bulk-marking files in a single column for deletion

---

### 3.4 Cross-Drive / Cross-Machine Duplicate Detection
**Problem**: The scanner only runs on the local machine. Archives on external drives or NAS are missed unless manually mounted.

**Approach**:
- Exportable scan manifest: `super-duper-cli export-manifest --out scan.json` writes all `scanned_file` rows to JSON
- Import command: `super-duper-cli import-manifest scan.json` merges rows from another machine/drive into the local DB
- Cross-origin duplicate groups resolved by matching `content_hash`

---

## 4. User Interface

### 4.1 macOS / Linux UI
**Problem**: The native UI is Windows-only. The core library and CLI are fully cross-platform.

**Approach options**:
- **Tauri** (Rust backend + web frontend): smallest binary, cross-platform; reuse all Rust core directly without FFI
- **Slint** (Rust-native UI): native-feeling widgets, no web runtime
- Share all scan/analysis/storage logic; only the view layer changes

---

### 4.2 File Preview in Duplicate Groups
**Problem**: When deciding which copy to keep, users can't tell files apart without opening them externally.

**Approach**:
- Images: inline thumbnail rendered in a `BitmapImage` control
- Text/code: first N lines shown in a monospace read-only text area
- Video: first-frame thumbnail via ffmpeg or Windows Media Foundation
- Audio: file metadata (artist, album, duration) from ID3/FLAC tags
- All other types: file metadata summary (size, path, modified date)

---

### 4.3 Persistent Column Sorting & Filters
**Problem**: Duplicate group sort order resets to "wasted bytes descending" on every navigation.

**Approach**:
- Persist selected sort column, direction, and active type filter in `user_config.json`
- Apply on page load without user interaction

---

### 4.4 Bulk Path Operations
**Problem**: Adding 20 scan paths one by one is tedious.

**Approach**:
- "Add multiple folders" button opens a multi-select folder picker (Windows `IFileOpenDialog` multi-select mode)
- Paste from clipboard: detect newline-separated paths in the clipboard and offer to add all

---

### 4.5 Keyboard Navigation & Shortcuts
**Problem**: Power users working through large lists must use the mouse for every action.

**Proposed bindings**:

| Key | Action |
|---|---|
| `Space` | Toggle mark-for-deletion on focused file |
| `Enter` | Expand / collapse focused duplicate group |
| `Delete` | Remove focused scan path |
| `Ctrl+A` | Select all files in focused group |
| `Ctrl+Z` | Undo last mark |
| `F5` | Start scan |
| `Escape` | Cancel scan |

---

### 4.6 Export Reports
**Problem**: There is no way to share scan results without sharing the SQLite database.

**Approach**:
- Export to CSV: duplicate groups with all member paths, sizes, and wasted bytes
- Export to HTML: self-contained report with sortable tables and summary statistics
- Export to JSON: machine-readable format suitable for scripting downstream actions

---

### 4.7 System Tray / Background Mode
**Problem**: The app must stay in focus to monitor scan progress.

**Approach**:
- Minimize to system tray; show progress as tray tooltip
- Windows notification on scan completion with "View Results" action that brings the window to focus
- Optional: run a background watch-mode scan (see 1.3) from the tray without opening the full UI

---

## 5. Performance & Scalability

### 5.1 Streaming DB Writes
**Problem**: All results are written in a single transaction at the end of the scan. For very large scans this can cause a multi-second UI freeze and risks losing all results if the process is killed.

**Approach**:
- Write `scanned_file` rows in batches of 10,000 during the hash phase (not just at the end)
- Write duplicate groups incrementally as each size bucket is resolved
- The UI can display partial results while scanning continues

---

### 5.2 Parallel Directory Analysis
**Problem**: `build_directory_fingerprints()` processes one depth level at a time (sequentially across levels) to respect the parent-before-child dependency. Within a level, computation is serial.

**Approach**:
- Within each depth level, fingerprint computation for sibling directories is fully independent — run with `par_iter()`
- Profile and identify the bottleneck level; likely to yield meaningful speedup on deeply nested trees

---

### 5.3 Configurable Hash Algorithm
**Problem**: XxHash64 is non-cryptographic. For users who need collision resistance (forensic use, legal record-keeping), SHA-256 or BLAKE3 would be preferable.

**Approach**:
- Add `hash_algorithm` field to `AppConfig` (`xxhash64` | `blake3` | `sha256`)
- Implement each behind a `ContentHasher` trait
- Store algorithm identifier in `scan_session` so mixed-algorithm sessions are never compared
- Cache keys include algorithm identifier to prevent stale cross-algorithm hits

---

### 5.4 Large File Streaming
**Problem**: Files larger than available RAM cannot be safely fully-read into a single buffer. The current implementation reads the entire file into memory for hashing.

**Approach**:
- Stream file through hash in fixed-size chunks (e.g. 1 MB) using `Read::read_exact` in a loop
- Already works for XxHash64 (incremental update); BLAKE3 and SHA-256 also support incremental APIs
- Memory usage becomes constant regardless of file size

---

### 5.5 Multi-Database / Split Storage
**Problem**: On very large libraries the SQLite file can grow to several GB, and writes become contended.

**Approach**:
- Separate `scan.db` (write-heavy, per-session data) from `analysis.db` (read-heavy, directory similarity)
- Attach both databases in a single SQLite connection via `ATTACH DATABASE`
- Or migrate to a proper embedded database (e.g. DuckDB) for analytical queries

---

## 6. Developer & Operational

### 6.1 Automated Integration Tests
**Problem**: The test suite covers individual crates but there are no end-to-end tests that exercise the full pipeline from CLI invocation through to database state.

**Approach**:
- Generate a deterministic synthetic test corpus with known duplicates and directory structures
- CLI integration test: invoke `cargo run -p super-duper-cli -- process`, assert output contains expected group counts
- FFI integration test: drive the C ABI directly from a Rust test binary

---

### 6.2 Metrics & Telemetry (opt-in)
**Problem**: There is no visibility into scan performance regressions across versions.

**Approach**:
- Emit structured timing events at each pipeline stage: scan, partial hash, full hash, db write, dir analysis
- Write to a `performance_log` SQLite table: session_id, stage, duration_ms, file_count
- CLI flag `--benchmark` prints per-stage breakdown after each run

---

### 6.3 Plugin / Extension System
**Problem**: Custom duplicate-detection strategies (e.g. perceptual image hashing, audio fingerprinting) require modifying the Rust core and rebuilding.

**Approach**:
- Define a `ContentHasher` plugin trait with a stable C ABI
- Load plugins from a configurable directory at startup via `dlopen`/`LoadLibrary`
- Allow plugins to register for file type patterns (e.g. `*.jpg`, `*.mp3`)
- This unlocks community-contributed detectors without coupling them to the core release cycle

---

### 6.4 Installer & Auto-Update (Windows)
**Problem**: Distribution requires manually building from source.

**Approach**:
- MSIX package with `<WindowsAppSDKSelfContained>true</WindowsAppSDKSelfContained>` to bundle the Windows App Runtime
- Or WiX-based MSI that chains the Windows App SDK installer as a prerequisite
- GitHub Releases workflow: build, sign, and upload installer on tag push
- In-app update check against GitHub Releases API; prompt user when a newer version is available

---

### 6.5 Configuration UI
**Problem**: Ignore patterns, hash algorithm, similarity threshold, and other advanced settings are only configurable by editing files.

**Approach**:
- Expand the existing Settings page to expose all `AppConfig` fields
- Changes are written to `user_config.json` immediately; take effect on next scan
- Add a "Reset to defaults" button

---

*Last updated: February 2026*
