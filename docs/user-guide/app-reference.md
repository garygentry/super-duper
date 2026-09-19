# Windows app reference

Lookup information for every screen, command, keyboard shortcut, status, limit and file location in
the Super Duper Windows app. Labels are quoted exactly as the app shows them. For task steps, start
with [Find your first duplicates](getting-started.md) and the guides it links to; for background,
see [How Super Duper finds and reviews duplicates](how-it-works.md).

## System requirements

| Item | Detail |
| --- | --- |
| Operating system | Windows 11 x64, build 22000 or newer |
| Package | `super-duper-<version>-win-x64.zip` from GitHub Releases, with a `.sha256` checksum file beside it |
| Installation | None. The zip is self-contained (no separate .NET runtime) and holds one folder with `SuperDuper.Windows.exe` and `super-duper-worker.exe` |
| Signing | Not code-signed. Windows SmartScreen may show a warning on first run |
| Not provided | Installer, Arm64 build, automatic updates |
| Theme | Follows the Windows light or dark app mode. There are no in-app theme or layout settings |
| Text size | Follows the Windows **Text size** setting |
| Not yet verified | High contrast themes, Narrator and NVDA, multi-monitor and 200% scaling setups |
| File changes | None. The app is review-only and never deletes, moves or modifies scanned files |

## Window layout

The window opens at 1180 × 760 pixels, centred, and cannot be made smaller than 900 × 600.

### Header

| Element | Description |
| --- | --- |
| **Saved scans** | Toggle that shows or hides the saved-scans pane on the left |
| Page title | Name of the open saved scan |
| Context line | `Scan <id> · <date> · <status> · <n> locations` for the open scan; otherwise **Loading scan history…**, **Scan history unavailable** or **No scan yet** |
| **Start scan** / **Scan again** | **Start scan** when no scan is open, **Scan again** when one is. Both save any setup edits and start a new scan immediately. Unavailable while another scan runs, while history loads, or while setup cannot start |

### Areas and pages

| Area (tab) | Pages (sub-tabs) | Purpose |
| --- | --- | --- |
| **Scan** | **Setup**, **Progress**, **Summary** | Edit the saved scan; watch the running scan; show the final progress of the open finished scan |
| **Results** | **Files**, **Folders** | Duplicate-file sets and exact duplicate folders of the open completed scan |
| **Review** | none | The review plan, whole-plan check and location preferences |
| **History** | **Scans**, **Performance** | Earlier scans of the saved scan; performance summaries |

### Banners and overlays

| Element | When shown | Content and commands |
| --- | --- | --- |
| Active-scan banner | A scan is running and **Progress** is not the current page | **Active scan:** name; phase · elapsed time · **Warnings:** count; **View progress** |
| Completion banner | A scan finished while you were on another page | **Scan complete ·** number of duplicate file sets; **View results**, **Dismiss** |
| Error banner | An action failed | **Something needs attention**, the message, **Dismiss** |
| Unsaved setup prompt | You leave **Setup** with unsaved edits | **Save setup changes before leaving?**; **Save**, **Discard**, **Stay** (focus starts on **Stay**) |
| Confirmation messages | Before deleting, checking, cancelling a check, or closing during work | Windows message box with **Yes**, **No**, **Cancel**. Only **Yes** proceeds; **No** is the default |

When a scan completes while **Progress** is the current page, the app opens **Results** directly
instead of showing the completion banner.

### Layout breakpoints

| Condition | Effect |
| --- | --- |
| Window narrower than 900 or shorter than 600 pixels | Not allowed (minimum size) |
| Window narrower than 1100 pixels, or Windows text size large enough to make body text larger than 18 pixels (above about 128%) | Saved-scans pane is hidden; the **Saved scans** toggle still opens it |
| **Files** or **Folders** area narrower than 960 or shorter than 500 pixels | One pane at a time: the set list, then the copies (**Compare selected set**, **Back to sets**, **Back to copies**) |

## Screens

### Start-up and empty state

| State | Shown text |
| --- | --- |
| Engine starting | **Starting worker** — "Establishing a private connection to the Super Duper engine." |
| No saved scans | **Find your duplicate files** — "Choose folders to compare. Save them once and scan again whenever you need." Command: **Choose folders** |

With saved scans, the app opens the first saved scan in the list and its newest scan.

### Saved scans pane

| Element | Description |
| --- | --- |
| **Saved scans** heading and refresh button | Reloads the list (**Refresh saved scans**) |
| Rows | Saved-scan name and the status of its latest scan: **No scans yet**, a scan status, or **Recovery required** |
| **New saved scan** | Opens **Setup** with a draft named **New saved scan** (or **New saved scan 2**, …). Nothing is stored until **Save changes** or **Start scan**. Unavailable while a scan runs |

Selecting a row loads that saved scan's setup and history and opens its newest scan.

### Scan › Setup

Page title **Choose locations**.

| Element | Description |
| --- | --- |
| **Add folder or drive** | Folder picker; fills the empty location row or adds a row, then refreshes cloud detection |
| **Enter path** | Adds an empty location row (only when none is empty) |
| Location row | Path box, remove button (**Remove location:** path), and a note: **Enter an absolute folder or drive path.**, **Location configured; availability is checked again on Start.**, or a validation message |
| **Saved scan name** | Name of the saved scan |
| **Save changes** | Saves the saved scan; available only when there are valid unsaved edits |
| **Scan settings** | "Cloud sync folders and ignored files are skipped." and **Repeat scans:** current choice |
| Cloud detection line and **Refresh** | Result of registered cloud location detection (see below) |
| **Scan policy details** | Counts of locations, ignore patterns and manual exclusions; the policy **Exclude registered cloud sync roots**; each detected cloud location; the repeat-scan choice |
| **Advanced: content reads and exclusions** | **Repeat scans** (**Reuse verified hashes** or **Re-read candidate content**), **Manual cloud location exclusions** (one absolute path per line), **Ignore patterns** (one glob per line) |
| **Manage saved scan** | **Delete saved scan and history**, with a **Delete session?** confirmation |

**Repeat scans** is not saved with the saved scan. It resets to **Reuse verified hashes** whenever a
saved scan is loaded.

Default ignore patterns for a new saved scan: `**/node_modules/**`, `*/$RECYCLE.BIN`, `*/.bzvol`,
`*/System Volume Information`, `*/Recovery`.

Cloud detection messages:

| Message | Meaning |
| --- | --- |
| **Checking registered cloud locations…** | Detection is running |
| **No registered cloud locations intersect the selected scan roots.** | Nothing will be excluded |
| `<n>` **registered cloud location(s) will be excluded.** | Listed under **Scan policy details**, each marked "A selected scan root is inside this location and will be fully excluded." or "This registered subtree will be excluded from the broader scan root." |
| **Registered cloud location detection is unavailable. Refresh before starting a scan.** | Detection has not succeeded; **Start scan** is unavailable |
| **Windows Cloud Files registration detection is not supported. Scans fail closed under this policy.** | Scans cannot start on this PC |
| **Registered cloud location detection failed:** reason | Shown in red; **Start scan** is unavailable |
| …**Cloud folders on this PC changed since this scan was saved; the new list is saved when a scan starts.** | Appended when detection differs from the saved list |

Detection runs again when a scan starts.

### Scan › Progress and Scan › Summary

**Progress** shows the running scan. **Summary** shows the same view for the open scan once it has
stopped.

| Element | Description |
| --- | --- |
| Title | Current phase; **Last phase:** phase for a stopped scan |
| Status | Scan status (see [Scan statuses and phases](#scan-statuses-and-phases)) |
| **Cancel scan** | Cancels the running scan without confirmation; shows **Cancelling…** afterwards. Available only while the status is **Scanning** |
| **Run elapsed** / **Elapsed at last observation** | Time since the scan started |
| **Warnings:** | Warning count |
| **Performance details** | Opens the performance summary for this scan |
| **Review warnings** | Opens the warning list in **History**; available when there are warnings |
| Freshness | **Last update** time **ago**, or **No live updates — scan has stopped** |
| **Time left:** | **Still estimating**, **About** time **for file reads**, **No reliable estimate yet**, **Not estimated for this step**, or **Complete** |
| **Current activity** / **Last reported activity** | A sampled file name and folder. "This is a sample of scan activity; a large file may stay here while it is read." |
| **Work details** | Six-stage funnel (**Discovered**, **Resolved from metadata**, **Partial screened**, **Selected for full hash**, **Full hash satisfied**, **Finalized duplicates**), folder substage, read throughput |
| **Hash reuse details** | Partial and full hash cache hits, misses, errors and stores |
| **Diagnostics** | Exact sampled path (selectable), phase elapsed time, active devices, **Excluded locations:** count |
| Empty state | **No scan selected** — "Start a scan or select one from Run history." |

### Results › Files

| Element | Description |
| --- | --- |
| **Path search** | Case-insensitive text contained in a copy's path; with **Exact path**, one complete path |
| **Apply** | Applies all draft filters as one query |
| **Filters** | Shows or hides the filter panel |
| **Clear filters** | Removes all filters |
| Query status | **Unapplied changes · use Apply or Enter.**, **Updating results… Showing the previous applied query.**, **Filtered results · totals cover matching sets.** |
| Totals | **Sets:**, **Copies:**, **Potential savings:** for the applied filters |
| Filter chips | One per applied filter, for example **Path contains: …**, **Exact path: …**, **One copy ≥ … bytes**, **Three or more copies**, **Across drives**, **Any copy: .jpg**, **All copies: no extension**, **Root: …**, **Drive: …**. Selecting a chip removes that filter |
| Filter panel | **Exact path**; **Minimum size** and **Size unit** (B, KiB, MiB, GiB, TiB); **1 GiB or larger**; **Three or more copies**; **Across drives**; **Extension**, **No extension**, **All copies must match**; **Selected root** and **Drive** lists with **Root order** / **Drive order** (**Most sets first**, **Name A–Z**); **Location coverage and largest opportunity** |
| Set sort | **Potential savings, largest first** (default) or smallest first; **One-copy size**, largest or smallest first; **Copies**, most or fewest first; **Representative label**, A–Z or Z–A |
| **Duplicate sets** list | Label, `<n>` **copies**, **Size**, **Save up to**, location and drive span; `<n>` **groups** below |
| Selected set heading | Label, or **Select a duplicate set**; `<n>` **copies**; previous and next set buttons |
| **Set review:** line | `<k>` keep, `<r>` remove, `<u>` undecided · physical copies remaining |
| **Set and review details** | Location span and whole-scan review totals |
| **Copies in selected set** | Path within the location, location, **Size**, **Modified**, **Decision:**, and current state |
| Selected copy panel | **Current decision:**, **Keep copy**, **Mark copy for removal**, **Reset copy**, **Full path**, **Copy path**, **Show in Explorer** |
| **Check these copies** | Checks the copies in the set, or the visible page of copies; **Cancel validation** |
| Reconciliation warning | Shown after a file-change overflow: **Reconcile next batch**, **Cancel reconciliation** |

States:

| State | Text |
| --- | --- |
| Loading | **Loading duplicate groups…** |
| Scan not completed | **Results unavailable** — "Duplicate results become available after this scan completes." or "This run is `<status>`; partial results are not shown." |
| No duplicates | **No duplicate files** — "No duplicate files in this completed run." |
| No matches | **No duplicate files** — "No duplicate sets match the applied filters. Clear filters to see all sets." |

Copy states:

| State | Meaning |
| --- | --- |
| **Not validated in this working view** | Not checked since the results were loaded |
| **Present; matches scan metadata** | Checked and unchanged |
| **Missing** | Checked and not found; a Keep or Remove decision is invalidated |
| **Changed since scan** | Checked and different; a Keep or Remove decision is invalidated |
| **Unavailable; decision retained until validation can complete** | Could not be reached; the decision is kept |
| **Validation pending after a coalesced filesystem hint** | A file change was noticed; not yet checked |
| …**; prior Keep decision invalidated** / **; prior Remove decision invalidated** | Suffix after a check invalidated a decision |

**Keep copy** and **Mark copy for removal** are available only for a copy that is unchecked or
**Present**. **Reset copy** is available when the copy has a decision or an invalidated decision.

### Results › Folders

| Element | Description |
| --- | --- |
| **Path search**, **Apply**, **Filters**, **Clear filters** | As on **Files**; the filter panel (**Filter and result details**) has **Minimum size** and **Size unit** only |
| Notes | "Exact content only; nested matches covered by a larger exact root are omitted." **Why locations can differ**: "Contents can match exactly even when their root names differ." |
| Set sort | **One-copy size**, **Copies**, **Descendant files**, **Representative path**, each in both directions |
| **Exact-folder sets** list | Path; `<n>` **folder copies ·** `<n>` **files per copy ·** size **per copy ·** size **recoverable** |
| Comparison | **Folder copies in selected set**, each with name, decision, **Location:**, **Shared context:**, **Differs here:** |
| Folder actions | **Keep folder**, **Mark folder for removal**, **Reset folder**; **Complete folder path** with **Copy path** and **Show in Explorer**; **Select page in Explorer**; **Decision scope details** |

States: **No folders match these filters**, **No exact duplicate folders**, **No scan selected**,
**Folder results not ready**, **Scan cancelled**, **Scan failed**, **Folder results unavailable**.

Folder decision refusals:

| Message starts with | Cause |
| --- | --- |
| "This folder choice overlaps an existing Keep or Remove choice." | A file or folder inside already has a decision |
| "This choice would leave an exact-folder set without an intact copy." | No intact folder copy would remain |
| "This choice would leave a duplicate-file set without an accessible physical copy." | No copy of a file inside would remain |
| "Review choices changed before this update was saved." | The plan changed in the meantime |

Folder copies have no **Check these copies** or reconciliation commands.

### Review

Page title **Review marked copies**, with "Marking copies does not delete files."

| Element | Description |
| --- | --- |
| **Marked for removal** | `<n>` **file(s) marked ·** size **planned**. **Count details**: distinct affected files, physical items, plan revision. File and folder overlap and hard-link aliases are counted once |
| **Check the whole plan** | Freshness heading, outcome, explanation, **Check marked copies** (**Starting check…** while it starts), **Check details** |
| Check in progress | **Whole-plan check in progress**, **Checked** `<x>` **of** `<y>` **validation items.**, **Cancel check** |
| **Location preferences** | Rule editor, preview and application (see below) |
| **Files** / **Folders** tabs | "Your file decisions." / "Folder decisions include their contents." Rows **File set** `<id>` or **Folder set** `<id>`, `<r>` **marked ·** `<k>` **kept ·** `<u>` **undecided**, copies remaining, **Open set**; **Previous Files page** / **Next Files page** (and Folders) |
| Build notice | "This build can review and check a removal plan. Moving files to the Recycle Bin is not available." |
| **Whole-plan check details** | After a check: summary (**Ready**, changed, missing, unavailable, conflicts), "Results describe the files at the time of this check. Files can change afterward; nothing has been deleted.", rows with outcome, **Removal**/**Survivor** file or folder, path, explanation, **Open set**; **Previous page** / **Next page** |

**Check marked copies** is available only for a completed scan with at least one copy marked for
removal.

Whole-plan check freshness and outcome:

| Freshness heading | Meaning |
| --- | --- |
| **Plan has not been checked** | No check for this scan |
| **Checked against your current decisions** | The last check matches the current plan |
| **Plan changed — check again** | Decisions changed after the last check |
| **Copies need another check** | Files may have changed after the last check |

| Outcome | Meaning |
| --- | --- |
| **Ready** | All checked copies passed |
| **Blocked** | A set has no safe copy to keep |
| **Needs review** | Copies changed, are missing or could not be checked; or no current check exists |
| **Checking marked copies** | A check is running |

Location preferences:

| Section | Elements |
| --- | --- |
| **1. Choose preferred locations** | **Saved rule**, **New**, **Rule name**, **Roots, highest rank first** with **Move up**, **Move down**, **Remove**, path box and **Add root**, **Save preference** |
| **2. Preview the effect** | **Preview scope** (**Current complete filter**, **Selected set**, **Completed run**), **Preview**, **Next page**, **Apply preview…**; confirmation **Confirm saved rule decisions** with **Confirm application** and **Cancel**; **Preview counts and details**; rows **Applicable** or **Blocked** with an explanation |
| **3. Review applied decisions** | **Application details**, **Reverse rule application**; confirmation with **Confirm reversal** and **Cancel** |

### History

**Scans** page:

| Element | Description |
| --- | --- |
| Header | **History ·** saved-scan name; **Open scan** |
| **Scan history** list | Newest first. Row: start time, status, **Duration:**, and for completed scans `<n>` **exact file sets ·** size **potential savings ·** `<n>` **warnings** |
| **Refresh**, **Previous scans**, **Next scans** | Reload and page the list |
| Highlighted scan card | Relationship to the open scan (for example **Highlighted only. The workspace remains on …**), **Recorded settings and locations**, **Performance details**, **Review warnings** |
| Warning list | **Warnings · selected scan** or **Warnings · active scan**; rows with **Count:**, **Severity:**, phase, message, up to three **Example:** paths, **Warning code and category**; **Open duplicate results** (some hashing warnings only); **Refresh current warnings**, **Next warning page**, **Cancel load**, **Cancel navigation**; **Warning details** with the diagnostic log path; return button **Return to run history**, **Return to progress** or **Return to scan summary** |
| Empty state | **No scans yet** — "Start this saved scan’s first run from Scan." |

**Open scan** opens a **Completed** scan in **Results** › **Files**; a **Pending**, **Scanning** or
**Cancelling** scan in **Scan** › **Progress**; any other scan in **Scan** › **Summary**.

**Performance** page:

| Section | Content |
| --- | --- |
| Header | Context such as **Performance · opened scan**; **Refresh**; return button (**Return to scan history**, **Return to progress**, **Return to scan summary**); **About these measurements** |
| **Run health** | Status and duration, **Full-read throughput**, **Warnings** |
| **Phase durations** | **Phase**, **State**, **Active duration** |
| **Cache and read summary** | Hash reuse and content reads; **Candidate funnel** |
| **Host and drive summaries** | **System CPU**, **Process memory**; per drive: **Capacity**, **Free at scan start**, **Current** and **Peak** **Read throughput**, **Read operations**, **Average latency**, **Active time**, **Queue depth** |
| **Retained run comparison** | **Run**, **Status**, **Started**, **Software build**; **Compare selected run**; **Current** and **Selected prior** **Duration**, **Full-read throughput**, **Warnings**, **Peak device read** |
| Empty state | "Select a scan to view bounded performance telemetry." |

Unrecorded values are shown as unavailable rather than estimated.

### Recovery screen

Shown when the scan engine cannot start, cannot open its data, or stops unexpectedly.

| Element | Description |
| --- | --- |
| Title and detail | **Worker connection failed**, **Worker exited unexpectedly**, **Worker restart failed**, or a saved-data title (see [Fix startup and saved-data problems](troubleshooting.md)) |
| **Technical details** | **Worker executable** path and **Diagnostic log** path |
| **Reconnect** | Starts a fresh engine, reloads saved scans, and marks any scan that was running as **Interrupted**. Shows **Restarting worker**, then **Worker recovered** |
| Status bar | **Scan engine:** title and detail |

## Keyboard shortcuts

| Keys | Where | Action |
| --- | --- | --- |
| Alt+H | Anywhere in the workspace | Opens the **History** area |
| Alt+M | **Scan** area | Opens **Summary** |
| Alt+O | **Results** area | Opens **Folders** |
| Alt+C | **Scan** › **Progress** | **Cancel scan** |
| Ctrl+F | **Results** › **Files** | Moves to **Path search** and selects its text |
| Alt+A | **Results** › **Files** or **Folders** | **Apply** |
| Enter | **Results** › **Files**, in **Path search**, **Minimum size** or **Extension** | Applies the filters |
| Enter | **Results** › **Folders**, in **Path search** or **Minimum size** | Applies the filters |
| Esc | **Results** › **Files**, inside the open filter panel | Collapses the filter panel and returns to **Filters** |
| Alt+M, Alt+U | **Results** › **Files** or **Folders**, filter panel open | Moves to **Minimum size**, **Size unit** |
| Alt+E, Alt+D | **Results** › **Files**, filter panel open | Moves to **Extension**, **Drive** |
| Alt+V | **Results** › **Files** | **Check these copies** |
| Alt+X | **Results** › **Files**, reconciliation warning shown | **Reconcile next batch** |
| Alt+E | **Results** › **Folders**, folder-copy list focused | **Show in Explorer** for the selected folder copy |
| Alt+G | **Results** › **Folders**, folder-copy list focused | **Select page in Explorer** |
| Ctrl+Home | **Review**, after a whole-plan check | Moves to **Whole-plan check details** |
| Esc | **Review** › **Location preferences**, confirmation open | Cancels the application or reversal confirmation |
| Alt+E | **History** › **Scans** | **Open scan** |
| Alt+W | **History** › **Scans** | **Review warnings** for the highlighted scan |
| Alt+C | **History** › **Performance** | **Compare selected run** |

Other underlined letters in the app are shared by more than one control on the same screen, for
example Alt+S (**Scan** and **Setup**) and Alt+R (**Results** and **Review**). Pressing a shared
letter moves focus between those controls instead of acting, so those keys are not listed. The next
and previous duplicate-set buttons on **Files** mention Alt+N and Alt+P in their tooltips, and
**Review warnings** on **Progress** is announced as Alt+W, but none of these keys act on them.

## Scan statuses and phases

| Status | Meaning |
| --- | --- |
| **Pending** | Created, not yet running |
| **Scanning** | Running |
| **Cancelling** | Cancel requested |
| **Completed** | Finished; results available |
| **Cancelled** | Stopped by you; no results |
| **Failed** | Stopped by an error; no results |
| **Interrupted** | The engine stopped before the scan finished; no results |

The saved-scans pane can also show **No scans yet** and **Recovery required**.

| Phase | Work |
| --- | --- |
| **Discovering files** | Walks the locations and records file sizes and metadata |
| **Hashing candidates** | Reads the start, then the full content, of files that could be duplicates |
| **Saving results** | Stores the duplicate sets |
| **Analyzing folders** | Finds exact duplicate folders |
| **Finalizing** | Completes the scan record |

A stopped scan's title reads **Last phase:** followed by the phase it reached.

Whole-plan check statuses: **Pending**, **Running**, **Cancelling**, **Completed**, **Cancelled**,
**Interrupted**, **Failed**.

Decisions: **Keep**, **Remove**, **Undecided**.

## Limits

| Item | Limit |
| --- | --- |
| Saved scan name | 1–200 characters; unique, ignoring case |
| Locations per saved scan | 64 |
| Ignore patterns | 512 per saved scan, 1,024 characters each |
| **Path search** | 512 characters; 32,767 with **Exact path** |
| **Extension** | 255 characters |
| **Minimum size** | 9,223,372,036,854,775,807 bytes |
| Duplicate sets, copies, folder sets, folder copies per page | 200 |
| **Selected root** and **Drive** entries per page | 25 |
| **Check these copies** | 200 copies per check |
| **Reconcile next batch** | 200 copies per batch |
| **Review** › **Files** / **Folders** sets per page | 200 |
| **Whole-plan check details** rows per page | 100 |
| Location preference rule | Name 1–128 characters without surrounding spaces; 1–64 locations |
| Location preference preview rows per page | 100 |
| **Scan history** rows per page | 500 |
| Warning rows per page | 25, each with up to three examples |
| **Retained run comparison** | Newest 25 runs |
| **Host and drive summaries** | 64 drives |
| `worker.log` | Rotated to `worker.log.previous` at about 5 MiB |

## Saved-scan validation messages

Errors prevent saving and scanning. Warnings are shown but do not block.

| Type | Message |
| --- | --- |
| Error | Enter a session name. |
| Error | Session names may contain at most 200 characters. |
| Error | Another session already uses this name. |
| Error | Add at least one scan root. |
| Error | A session may contain at most 64 scan roots. |
| Error | Scan root must be an absolute path: `<path>` |
| Error | Scan root is not a valid Windows path: `<path>` |
| Error | Ignore patterns may contain at most 1024 characters. |
| Error | Ignore patterns cannot contain control characters. |
| Error | Ignore pattern has an unmatched character class: `<pattern>` |
| Error | A session may contain at most 512 ignore patterns. |
| Error | Manual cloud location exclusions must be absolute paths. |
| Error at start | At least one scan root must be available before starting a scan. |
| Warning | `<drive>` scans an entire drive and may take a long time. |
| Warning | Root is currently unavailable: `<path>` |
| Warning | Removed nested root `<path>`; it is already covered by `<path>`. |
| Warning | Removable root availability may change during a scan; disconnects are reported as warnings: `<path>` |
| Warning | Mapped network root is best-effort and uses the worker process account's drive mapping: `<path>` |
| Warning | UNC network root is best-effort; latency, credentials, and disconnects may produce warnings: `<path>` |
| Warning | Root uses a filesystem type that has not been classified as fixed, removable, or network: `<path>` |

Validation messages say "session" where the rest of the app says "saved scan". Network locations
are treated as reachable while you edit; the engine checks them when the scan starts.

## Files the app keeps

All state lives in one data folder, by default `%LOCALAPPDATA%\SuperDuper`. The app runs one window
per data folder. Nothing is written to the folder the app was unzipped to, and nothing is sent off
the PC.

| Path in the data folder | Content |
| --- | --- |
| `super_duper.db` | Saved scans, scan records, results, review decisions and location preference rules |
| `super_duper.db-wal`, `super_duper.db-shm` | Recent database changes; belong with `super_duper.db` |
| `super_duper.db.lock` | Held by the scan engine while it runs, so only one engine uses the database |
| `scan_status.db` | Scan progress and performance history |
| `content_hash_cache.db` | Folder holding the repeat-scan hash cache |
| `presentation-preferences.json` | Whether the saved-scans pane is open, the last **Results** page (**Files** or **Folders**), and which sections are expanded. No paths, decisions or errors |

The diagnostic log is always in `%LOCALAPPDATA%\SuperDuper\logs`, as `worker.log` and
`worker.log.previous`.

### Environment overrides

For advanced use, such as keeping a separate disposable data set. Set these before starting the app.

| Variable | Effect |
| --- | --- |
| `SUPER_DUPER_DB_PATH` | Full path of the main database file. Its folder becomes the data folder: the status database, hash cache and `presentation-preferences.json` follow it unless overridden below, and the one-window rule applies to that folder |
| `SUPER_DUPER_STATUS_DB_PATH` | Full path of the status database |
| `HASH_CACHE_PATH` | Full path of the hash cache folder |

Logs stay in `%LOCALAPPDATA%\SuperDuper\logs` whatever these are set to.

## Glossary

| Term | Meaning |
| --- | --- |
| Saved scan | A named, reusable list of locations and settings. Validation messages call it a session |
| Scan | One run of a saved scan, identified as **Scan** `<id>`. Its settings and results never change afterwards |
| Location (scan root) | A folder, drive or network path in a saved scan |
| Duplicate set | Two or more files with identical content, also called a group |
| Copy | One file in a duplicate set, or one folder in an exact-folder set |
| Representative label | The name shown for a set. It does not identify an original |
| Exact-folder set | Folders whose complete contents match; the folders' own names may differ |
| Potential savings | Space the extra copies in a set take up, as recorded by the scan. Not a promise of reclaimed space |
| Decision | **Keep**, **Remove** or **Undecided** for a copy. Records intent only |
| Review plan | All decisions for one scan |
| Invalidated decision | A decision cleared because a check found the copy missing or changed |
| Survivor | A copy you keep so that each set retains at least one copy |
| **Check these copies** | A check of the copies in the selected set against the scan |
| Reconcile | Checking copies under a location in batches after too many file changes to track individually |
| Whole-plan check | **Check marked copies**: a check of every marked copy and the copies you keep, including a full content hash |
| Location preference rule | A ranked list of locations used to preview and apply Keep and Remove decisions |
| Registered cloud location | A folder Windows reports as synced by a cloud provider; skipped by scans |
| Warning | A recorded problem in part of a scan, such as a file that could not be read |
| Scan engine (worker) | The background process that scans and stores data for the app |
