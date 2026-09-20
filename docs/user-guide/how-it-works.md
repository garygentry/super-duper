# How Super Duper finds and reviews duplicates

Super Duper asks you to trust it with a delicate question: which of your files are copies you could
do without? This page explains why its answers can be trusted, why it never deletes anything
itself, and why it keeps what a scan found separate from what is on your disk right now. None of it
is needed to use the app, but it explains the choices you meet along the way.

## Saved scans and scans

A saved scan is a reusable description: a name, a set of locations, the patterns to ignore and the
folders to exclude. A scan is one run of that description at one moment. Each time you select
**Start scan** or **Scan again**, Super Duper records a new scan with a copy of the settings it used,
and that record never changes afterwards.

This separation is what makes history trustworthy. If you add a location to a saved scan today,
last month's scan still shows the locations it actually covered, under **Recorded settings and
locations**. Its results, and the decisions you recorded against them, stay attached to the scan
that produced them. A decision always refers to a specific copy found by a specific scan, so there
is never any doubt about which file it meant.

The cost is that results do not update themselves. To see your files as they are now, you run a new
scan, and it starts a fresh review plan.

## How a duplicate is confirmed

Reading every byte of every file would be slow, so Super Duper narrows the field in stages and only
reads what it must.

First it walks the locations and notes each file's size. Two files of different sizes cannot be
identical, so a file whose size is unique is settled from its metadata alone, without being opened.
Empty files are skipped entirely.

Files that share a size are then screened by hashing their first 1 KB. This cheap step separates
most files that merely happen to be the same size, such as documents made from the same template.

Only files that still match are read in full, and each one's complete content is hashed. Files are
reported as duplicates when their sizes, first-kilobyte hashes and full-content hashes all agree.
The full-content hash is a fast 64-bit hash rather than a byte-by-byte comparison; the chance of two
different files of the same size matching on both hashes is vanishingly small, and the whole-plan
check reads the content again before you act on a plan.

Some things are deliberately not treated as copies. Windows lets one physical file have several
names (hard links); Super Duper recognises them by the file identity Windows reports and counts
such a file once, because removing one of its names would free no space. Links, junctions and other
reparse points are skipped rather than followed, so the same folder is not scanned twice by
another route. A file that changes, disappears or cannot be read during the scan is recorded as a
warning and left out of the affected results rather than guessed at.

Folders get the same care. An exact-folder set is a group of folders whose complete contents match:
the same file and subfolder names (ignoring letter case), arranged the same way, with identical
file content throughout. The folders themselves can have different names and live in different
places; only what is inside them has to match. When two large folders match, every pair of
subfolders inside them matches too, and listing all of those would bury the useful result, so
nested matches that a larger exact match already covers are left out.

A set's label is simply one of its names. Because the copies are identical, none of them is "the
original" as far as the content is concerned, and the app does not pretend otherwise.

## Why repeat scans are faster

The expensive part of a scan is reading file content. Super Duper keeps a persistent cache of the
hashes it has verified, stored on your PC, so that a later scan can skip reading files that have not
changed.

The difficulty is knowing that a file has not changed. A cached hash is reused only when the file's
identity as Windows reports it, its size, its precise modification time and a change marker from
the file system all match what was recorded, and they are checked both before and after the file is
used. If any of these is missing, imprecise or different — a location whose file system keeps only
coarse modification times, for example — Super Duper reads the content instead. Each scan still
walks every location again and discovers new, moved and deleted files; the cache only saves the
reading.

The **Re-read candidate content** choice exists for the times you want certainty that does not
depend on metadata at all. It bypasses hash reuse but keeps the normal narrowing by size and first
kilobyte, so it does not read every discovered file in full. The hashes it computes still refresh
the cache for later scans. The choice applies to the scan you start and is not stored with the saved
scan: opening one preselects whatever its latest scan used, defaulting to **Reuse verified hashes**
for a saved scan with no runs yet.

## Why cloud folders are skipped

Cloud sync services such as OneDrive often keep files as placeholders: the name and size are on your
PC, but the content lives online until something opens the file. Hashing a placeholder would make
Windows download it. A scan of a large synced folder could quietly pull down gigabytes, fill a
disk, and change which files are stored locally, all to answer a question about duplicates.

So Super Duper asks Windows which folders are registered cloud locations and skips them before any
of their files are inspected or read. If a location you chose sits inside a cloud folder, it is
skipped entirely; if a cloud folder sits inside a location you chose, just that folder is skipped.

If the app cannot find out which folders are registered, it does not start the scan. This is called
failing closed: an incomplete answer about cloud folders could mean downloading files you never
meant to download, so the safe response to not knowing is to wait. Some sync tools do not register
with Windows at all, which is why you can also list folders under **Manual cloud location
exclusions**. The whole-plan check follows the same rule and never opens cloud placeholders or
excluded locations.

## Decisions are a plan, not an action

Keep, Remove and Undecided are notes about your intent. Recording one changes a database entry on
your PC; it does not touch the file. Super Duper has no delete command, and this release cannot move
files to the Recycle Bin either. When you decide to remove files, you do that yourself, with tools
you already trust, after you have reviewed and checked the plan.

This division is deliberate. Finding duplicates is a question that can be answered carefully and
revisited; deleting files is an irreversible act that deserves its own moment. Keeping the app to
the first job means a mistaken click costs nothing, and every decision can be reset.

Within the plan, one rule is absolute: every duplicate set must keep at least one copy. Super Duper
refuses a decision that would mark the last copy of a set for removal, and for folders it refuses a
choice that would leave a set without an intact folder copy. The rule counts physical copies that
can be reached independently, so two names for the same hard-linked file do not count as two
survivors.

Folder decisions apply to everything inside the folder. A file can therefore be covered by both a
file decision and a folder decision, and Super Duper counts it once in the totals rather than
inflating them. It also refuses a folder choice that overlaps an existing choice inside it, so
that two decisions never disagree about the same file.

Location preferences build on the same plan. A rule proposes Keep and Remove decisions from a ranked
list of locations, but you see a preview first, apply it only after confirming, and can reverse it
later. Reversal removes only what the rule added, and decisions you made by hand are left alone.

## What a scan saw and what is there now

A scan's results are a record of the moment it ran. Files keep changing afterwards: you edit them,
move them, disconnect the drive they are on. Super Duper never rewrites the record to match; instead
it checks the present separately and shows both.

On **Results** › **Files**, each copy has a current state alongside its recorded details. **Check
these copies** compares the copies you are looking at with the scan. While you work, the app also
notices file changes in the scanned locations and marks affected copies as waiting to be checked;
if changes come too fast to track one by one, it asks you to reconcile that location in batches.

When a check finds a copy missing or changed, any Keep or Remove decision on it is invalidated,
because it was made about a file that no longer exists in that form. A copy that simply cannot be
reached, perhaps on a disconnected drive, keeps its decision until it can be checked. The scan's own
results are never altered by these checks.

The whole-plan check on the **Review** tab is the strongest form of this. It reads every copy you
marked for removal and the copies you are keeping, including their complete content, and compares
them with the scan. Its result applies to one version of your plan: change a decision and the check
reports that the plan changed; if files may have changed since, it asks you to check again. Even a
**Ready** result describes the files at the time of the check, which is why it is best run just
before you act.

## Everything stays on this PC

Super Duper keeps its data in `%LOCALAPPDATA%\SuperDuper`: the database of saved scans, results
and decisions, a separate database of scan progress and performance, the hash cache, a small file
of display preferences, and a diagnostic log. The app does not upload any of it. The only files it
reads are the ones in the locations you choose, including network locations if you add them.

The scanning itself happens in a separate background process, the scan engine, which the app starts
and talks to privately. Because the engine is separate, the window stays open if the engine stops:
the app shows a recovery screen and can start a fresh engine without losing completed work.

Only one engine may use a database at a time. The engine locks it while running, and the app allows
one window per data folder; starting it again brings the existing window forward. Two windows
writing to the same decisions could silently overwrite each other's work, so the app refuses rather
than risks it.

## See also

- [Find your first duplicates](getting-started.md) walks through a first scan.
- [Check marked copies before acting on them](check-marked-copies.md) covers the checks described
  above.
- [Windows app reference](app-reference.md) lists every screen, status and limit.
- [Fix startup and saved-data problems](troubleshooting.md) covers the recovery screen and backups.
