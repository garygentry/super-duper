# Set up and run scans

This guide covers the recurring scan tasks — creating and editing saved scans, choosing locations,
skipping folders, handling cloud sync folders, repeating, cancelling and deleting scans — and
assumes you have finished [Find your first duplicates](getting-started.md).

## Create a saved scan

1. Select **New saved scan** at the bottom of the **Saved scans** pane. If the pane is hidden,
   select **Saved scans** in the top-left corner first. On a new install with no saved scans you can
   also select **Choose folders**.
2. On the **Choose locations** page, add one or more locations (see the next section).
3. Replace the suggested name (**New saved scan**, **New saved scan 2** and so on) under **Saved
   scan name**. Names must be unique, ignoring case, and at most 200 characters.
4. Select **Save changes** to store the saved scan without scanning, or **Start scan** to save it
   and scan straight away.

Nothing is stored until you choose **Save changes** or **Start scan**. **New saved scan** is not
available while a scan is running.

## Add folders, drives and network paths

1. Select **Add folder or drive** and choose a folder in the window that opens, or select **Enter
   path** and type an absolute path such as `D:\Photos` or `\\server\share\Photos`.
2. Read the note under each location:
   - **Location configured; availability is checked again on Start.** The location is fine.
   - A warning explains something to be aware of but does not stop the scan. For example, a whole
     drive such as `D:\` "scans an entire drive and may take a long time", a removable drive may
     disconnect, and a mapped or UNC network location is best-effort.
   - An error, such as **Scan root must be an absolute path**, must be fixed before you can save.
3. To remove a location, select the button beside it (its tooltip reads **Remove location:**
   followed by the path).

A saved scan can hold up to 64 locations. If you add a folder that sits inside another location,
Super Duper drops the inner one and says which location already covers it. At least one location
must be reachable when the scan starts.

Links, junctions and other reparse points are skipped rather than followed. If you want a folder
that you normally reach through a junction or a `subst` drive letter, add its real path instead.

For the full list of messages, see
[Saved-scan validation messages](app-reference.md#saved-scan-validation-messages).

## Skip folders and files

1. On **Choose locations**, expand **Advanced: content reads and exclusions**.
2. Under **Ignore patterns**, enter one glob pattern per line, for example `**/*.tmp`.

   New saved scans start with these patterns, which skip common Windows system folders and
   `node_modules` folders:

   ```text
   **/node_modules/**
   */$RECYCLE.BIN
   */.bzvol
   */System Volume Information
   */Recovery
   ```

3. Select **Save changes**.

A saved scan can have up to 512 ignore patterns of up to 1,024 characters each. A pattern with an
unmatched `[` is rejected.

## Scan folders that contain cloud sync folders

Super Duper skips folders that sync with a cloud service, such as OneDrive, before it reads any of
their files. Windows reports these folders as registered cloud locations.

1. On **Choose locations**, read the line under **Scan settings**:
   - **No registered cloud locations intersect the selected scan roots.** Nothing is skipped.
   - A count such as **2 registered cloud location(s) will be excluded.** Expand **Scan policy
     details** to see each one. A location you chose that is inside a cloud folder is skipped
     entirely; a cloud folder inside a location you chose is skipped while the rest is scanned.
   - **Registered cloud location detection is unavailable. Refresh before starting a scan.** Select
     **Refresh** beside the line.
2. If a sync tool on your PC does not register its folders with Windows, expand **Advanced: content
   reads and exclusions** and enter each of its folders under **Manual cloud location exclusions**,
   one absolute path per line.
3. Select **Save changes** or **Start scan**.

**Start scan** stays unavailable until cloud detection has succeeded, and Super Duper checks again
when the scan starts. If the line reads **Windows Cloud Files registration detection is not
supported. Scans fail closed under this policy.**, scans cannot start on this PC.

If your cloud folders change after you saved the scan, the line adds **Cloud folders on this PC
changed since this scan was saved; the new list is saved when a scan starts.** You do not need to
do anything.

## Scan the same locations again

1. Select the saved scan in the **Saved scans** pane.
2. Optional: to make this scan read file content instead of reusing stored hashes, go to
   **Scan** › **Setup**, expand **Advanced: content reads and exclusions**, and set **Repeat scans**
   to **Re-read candidate content**.
3. Select **Scan again** in the top-right corner.

**Scan again** starts a new scan at once with the saved locations; it does not open **Setup**
first. Make any setup changes before you select it.

The **Repeat scans** choice is not saved with the saved scan. It returns to **Reuse verified
hashes** whenever you open a saved scan, so set it just before you start. See
[Why repeat scans are faster](how-it-works.md#why-repeat-scans-are-faster).

Each scan is kept separately in the history of the saved scan; earlier scans and their decisions
are not changed.

## Watch or cancel a scan

1. While a scan runs, the **Scan** › **Progress** page shows the phase, elapsed time, warnings and
   an estimate under **Time left**.
2. If you move to another page, a banner shows **Active scan:** with the scan's name. Select **View
   progress** to return.
3. To stop the scan, select **Cancel scan** on the **Progress** page. There is no confirmation; the
   button changes to **Cancelling…** and the scan ends as **Cancelled**.

A cancelled scan has no results; the **Files** page says partial results are not shown. Start the
scan again when you are ready.

If you close Super Duper during a scan, it asks **Cancel scan and exit?** Choose **Yes** to cancel
the scan and close, or **No** to keep scanning.

## Rename or delete a saved scan

To rename a saved scan:

1. Select it in the **Saved scans** pane and go to **Scan** › **Setup**.
2. Change **Saved scan name** and select **Save changes**.

To delete a saved scan:

1. Select it and go to **Scan** › **Setup**.
2. Expand **Manage saved scan** and select **Delete saved scan and history**.
3. In the **Delete saved scan?** message, select **Yes**.

Deleting removes the saved scan together with its scan history, results and review decisions. It
cannot be undone. It never touches the scanned files. You cannot delete a saved scan while a scan is
running.

If you leave **Setup** with unsaved edits, Super Duper asks **Save setup changes before leaving?**
Choose **Save** to keep the edits, **Discard** to go back to the saved setup, or **Stay** to keep
editing.

Related: [Windows app reference](app-reference.md) ·
[How Super Duper finds and reviews duplicates](how-it-works.md) ·
[Look into a past scan](past-scans.md)
