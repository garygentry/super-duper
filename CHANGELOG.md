# Changelog

All notable changes to Super Duper are recorded here. Versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-09-20

### Added

- A `super-duper export` CLI subcommand writes duplicate file groups (`duplicate-groups`) or
  session definitions and their run history (`sessions`) as CSV (the default) or JSON, to stdout.
  See `docs/export-format-v1.md`.
- `--format json` on `analyze-directories`, `count-hash-cache`, and `print-config` prints one JSON
  value to stdout instead of the human-readable text, for scripting. The default (no flag) is
  unchanged.
- A `super-duper auto-mark` CLI subcommand marks every duplicate file group's non-survivors for
  deletion, keeping one file per group by an explicit strategy: `keep-first` (the default,
  alphabetical, previously hardcoded), `keep-newest`, `keep-oldest`, or `preferred-path-prefix`
  (falls back to `keep-first` for any group with no member under the given prefix). The same
  strategies are exposed to FFI clients through `sd_auto_mark_for_deletion`.
- `Config.toml` now accepts `directory_similarity_threshold` (default `0.5`) and
  `directory_similarity_noise_cutoff` (default `50`) to tune `analyze-directories`' Jaccard
  similarity threshold and noise cutoff without recompiling; both are validated on load.
- A `super-duper trim-hash-cache` CLI subcommand (and `sd_trim_hash_cache` FFI function) removes
  hash-cache entries not confirmed unchanged, or created, within the last N scans (`--unseen-scans`,
  default 10). Every scan and confirmed cache hit now records the scan generation an entry was last
  seen in, kept separate from the entry's pinned on-disk encoding; a store written before this
  change falls back to the generation the entry was created in, so it still trims correctly. Fails
  cleanly while a scan holds the cache open, like the existing count and clear maintenance.
- New FFI entry points `sd_scan_start_async`, `sd_scan_observe`, and `sd_scan_join` run a scan on a
  background thread instead of blocking the calling thread for its duration, so `sd_scan_cancel`
  and every query stay usable while it runs. The existing blocking `sd_scan_start` is unchanged.
  Native clients only; the Windows app talks to the worker process instead.

### Changed

- `super-duper-worker.exe` now carries product, version, and copyright details in its file
  properties, like `SuperDuper.Windows.exe`.
- The CLI's logs (and the human-readable summaries built from them) now go to stderr instead of
  stdout, so stdout stays clean for `--format json` and CSV/JSON export.

### Fixed

- Opening the results database no longer writes to it, so reading results, saving progress, and
  other requests no longer wait for (or fail behind) a scan that is writing.
- Starting a preflight check, and its final survivor check, wait briefly for another write to
  finish instead of failing at once with "database is locked".
- The filesystem-change notice on **Results** › **Files** now names the **Check these copies**
  button instead of a nonexistent "Validate page".
- Setup validation messages, the delete confirmation, and the whole-plan check's cancel and
  announcement text now say "saved scan" and "check" instead of the protocol terms "session" and
  "preflight".
- Loading a saved scan now preselects the **Repeat scans** policy its latest run recorded, instead
  of always resetting to **Reuse verified hashes**.
- Every keyboard access key now activates its own control instead of cycling focus with another
  control that shared the same underlined letter (for example **Scan** and **Setup**, **Results**
  and **Review**, **Files** and **Filters**). The previous and next duplicate-set buttons on
  **Files** now respond to the Alt+P and Alt+N they already advertised, and **Review warnings** on
  **Progress** now responds to the Alt+W it announces. See the updated shortcut table in the user
  guide.
- Moving to the next or previous duplicate set on **Results** › **Files** no longer leaves the prior
  set's review counts (kept, removed, undecided) on screen while the new set's members load.
- An unexpected error no longer ends the app silently: it is logged to the worker's diagnostic log,
  the worker is stopped cleanly, and a plain message names the log file before the app closes.
- A bug in a view model's worker-event handler no longer looks like a worker crash: it is logged
  and the worker connection stays up.
- Cancelling a worker request now frees its tracking entry immediately instead of holding it until
  the worker eventually answers.
- Folder review decision errors (for example an overlapping Keep or Remove choice) are recognized
  by their worker error code instead of by searching the error text, so a wording change can no
  longer make the wrong message appear.
- The worker's diagnostic log now follows a database moved with `SUPER_DUPER_DB_PATH` (or a
  UI-development session) into a `logs` folder beside it, instead of always writing to
  `%LOCALAPPDATA%\SuperDuper\logs`.
- Checking a scan root's drive type and availability while editing **Setup** no longer runs on the
  window's UI thread; a slow removable or network drive can no longer stall typing.
- A scan's own database writes now wait up to 30 seconds for a stalled writer instead of failing
  the whole run at 5 seconds with "database is locked".
- The performance view now says telemetry is unavailable, instead of showing a database error, when
  no scan has ever run in the current state folder.
- Deleting a saved scan that a durable Recycle Bin operation still locks now reports which run and
  operation are responsible instead of a generic internal error.
- An exclusion spelled through a `subst` drive letter or a directory junction or symlink now prunes
  the scan; previously only the real path pruned it, because the walk compares canonical paths.
- A scan with heavy repeat-cache reuse (most files verified as cache hits) could abort outright with
  "scan engine rejected a hash progress observation" under real concurrency, because a hit's cache
  counter was published separately from, and could race ahead of, its matching completed-file
  counter. Both now travel together in one update (#65).

### Known limitations

- Not yet verified: Windows high contrast themes, Narrator and NVDA, and multi-monitor or 200% DPI
  setups.
- Moving files to the Recycle Bin is intentionally not available in this release.
- An exclusion written through a mapped network drive is not resolved to its share path; choose the
  shared folder directly. (`subst` drive letters and directory junctions or symlinks are resolved.)
- Only the plain Windows 11 x64 zip is provided: no installer, no Arm64 build, and no automatic
  updates.

## [0.1.0] - 2026-09-19

First release of the Windows app. It is **review-only**: Super Duper finds duplicates and records
what you want to keep or remove, but it never deletes, moves, or modifies scanned files.

### Download and requirements

- `super-duper-0.1.0-win-x64.zip`: a self-contained Windows 11 x64 build. No .NET runtime or
  installer is needed; unzip it anywhere and run `SuperDuper.Windows.exe`.
- Windows 11 x64 (build 22000 or newer).
- The build is not code-signed. Windows SmartScreen may say it "protected your PC" from an
  unrecognized app; choose **More info**, then **Run anyway**, if you trust the download. The
  zip's SHA-256 checksum is published next to it.

### Features

- Saved scans of any set of local, removable, or network folders, with ignore patterns and manual
  exclusions.
- Content-verified duplicate files (size, then partial hash, then full hash) and verified exact
  duplicate folders, with nested duplicates collapsed.
- Cloud-safe scanning: registered cloud sync folders such as OneDrive are excluded before any
  content is read, so placeholders are never downloaded. If cloud detection is unavailable, scans
  do not start.
- Paged results that stay responsive for very large scans, with filters by location, drive, size,
  extension, and exact or partial path.
- Durable keep/remove/undecided decisions per file and per folder, preferred-location rules, and a
  non-deleting preflight check of what a plan would affect.
- Run history with warnings, performance summaries, and Explorer reveal.
- A repeat-scan cache that reuses verified hashes for unchanged files.

### Data and privacy

- Everything stays on this PC. Scans, decisions, and the hash cache live in
  `%LOCALAPPDATA%\SuperDuper`; logs are in `%LOCALAPPDATA%\SuperDuper\logs`.
- Super Duper runs one window per data folder. Starting it again brings the open window forward.

### Known limitations

- Not yet verified: Windows high contrast themes, Narrator and NVDA, and multi-monitor or 200% DPI
  setups.
- Moving files to the Recycle Bin is intentionally not available in this release.
- Exclusions written through a junction or `subst` drive letter are not matched to the real path;
  choose the folder directly.
- Only the plain Windows 11 x64 zip is provided: no installer, no Arm64 build, and no automatic
  updates.

[Unreleased]: https://github.com/garygentry/super-duper/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/garygentry/super-duper/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/garygentry/super-duper/releases/tag/v0.1.0
