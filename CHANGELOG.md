# Changelog

All notable changes to Super Duper are recorded here. Versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- `super-duper-worker.exe` now carries product, version, and copyright details in its file
  properties, like `SuperDuper.Windows.exe`.

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

[Unreleased]: https://github.com/garygentry/super-duper/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/garygentry/super-duper/releases/tag/v0.1.0
