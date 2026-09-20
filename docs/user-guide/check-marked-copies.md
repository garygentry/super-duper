# Check marked copies before acting on them

This guide shows how to confirm that your review decisions still match the files on disk, from a
quick check of the copies on screen to a check of the whole plan, and what to do with the plan
afterwards; it assumes you have recorded decisions (see
[Review duplicate files and folders](review-duplicates.md)).

A scan's results describe the files as they were when the scan ran. Files can change afterwards.
None of these checks change, move or delete a file, and none of them change the scan's recorded
results.

## Check the copies you are looking at

1. In **Results** › **Files**, select a duplicate set.
2. Select **Check these copies** below the copy list, or press Alt+V.

   Super Duper checks every copy in the set, or only the copies on the visible page when the set
   has more than 200. Select **Cancel validation** to stop.

3. Read each copy's state:
   - **Present; matches scan metadata** — the copy is as the scan saw it. Your decision stands.
   - **Missing** or **Changed since scan** — the file is gone or different. Your Keep or Remove
     decision is invalidated and the copy returns to Undecided; the state ends with, for example,
     **prior Remove decision invalidated**.
   - **Unavailable; decision retained until validation can complete** — the file could not be
     reached, for example on a disconnected drive. Your decision stands.

4. If copies were unavailable, reconnect or restore the location and select **Check these copies**
   again.

While **Results** › **Files** is open, Super Duper notices file changes under the scanned
locations. Affected copies on screen show **Validation pending after a coalesced filesystem
hint**, and a notice asks you to "select Check these copies" — select that button to validate them.

## Catch up after many file changes

If so many files change at once that Super Duper cannot track them one by one, **Results** ›
**Files** shows a warning that starts **Working results are dirty and reconciliation is required
after a filesystem watcher overflow.** It names the affected location.

1. Select **Reconcile next batch**, or press Alt+X. Super Duper checks up to 200 copies under that
   location.
2. When the status says the location remains dirty, select **Reconcile next batch** again.
3. Repeat until the status says reconciliation checked the final copies.

Select **Cancel reconciliation** to stop; the location stays marked as needing reconciliation.
Reconciling updates copy states and decisions like **Check these copies** does. The scan's recorded
history is unchanged.

## Check the whole plan

The whole-plan check reads every copy you marked for removal and the copies you are keeping, and
compares each one with the scan, including a hash of its complete content. It does not open cloud
placeholders or excluded locations.

1. Select the **Review** tab. The **Marked for removal** card shows how many files are marked.
2. Under **Check the whole plan**, select **Check marked copies**. The button is available only for
   a completed scan with at least one copy marked for removal.
3. The **Check marked copies?** message says how many marked paths will be checked and that no
   files will be deleted. Select **Yes**.

   **Whole-plan check in progress** shows **Checked … of … validation items.** To stop, select
   **Cancel check**, then **Yes** in the **Cancel check?** message. Completed results stay
   available.

4. Read the outcome:
   - **Ready** — marked copies and the copies you are keeping passed the check.
   - **Blocked** — a set has no safe copy to keep. Open it and keep at least one copy.
   - **Needs review** — copies changed, went missing or could not be checked, or the plan has not
     been checked yet. The explanation below the outcome says which.
5. Under **Whole-plan check details**, read each row. It shows the outcome, whether the item is a
   removal or a survivor (a copy you keep), the path, and an explanation such as **The file size
   changed after the scan.** Select **Open set** to jump to that set in **Results**. Rows are shown
   100 at a time; use **Previous page** and **Next page**. On the **Review** tab, Ctrl+Home moves
   to this heading.
6. Fix any problems in **Results**, then select **Check marked copies** again.

The heading above the outcome tells you whether the last check still applies:

- **Checked against your current decisions** — the check matches your current plan.
- **Plan changed — check again** — you changed decisions after the check.
- **Copies need another check** — files may have changed since the check.
- **Plan has not been checked** — no check has run for this scan.

The **Files** and **Folders** tabs lower on the **Review** page list every set that has decisions,
with counts of marked, kept and undecided copies and how many copies remain. Select **Open set** to
review a set in **Results**. If the set is not on the page of results currently loaded in
**Results**, a message asks you to go to that page first and then select **Open set** again.

## Act on the plan outside Super Duper

Super Duper is review-only. The **Review** page states: **This build can review and check a removal
plan. Moving files to the Recycle Bin is not available.** When you decide to remove files, you do it
yourself:

1. Run **Check marked copies** and confirm the outcome is **Ready**.
2. On the **Review** tab, under **Files**, select **Open set** for a set with marked copies.
3. In **Results** › **Files**, select each copy marked **Remove**, then use **Show in Explorer** to
   open it in File Explorer, or **Copy path** to copy its full path.
4. Delete or move the file with File Explorer or another tool you trust. Deleting to the Recycle
   Bin lets you restore it later.
5. Repeat for the other sets. For folder decisions, use **Results** › **Folders** the same way.

After you remove files, a new check reports them as missing. That is expected. Run a new scan when
you want results that reflect the files as they are now.

Related: [Windows app reference](app-reference.md) ·
[What a scan saw and what is there now](how-it-works.md#what-a-scan-saw-and-what-is-there-now) ·
[Review duplicate files and folders](review-duplicates.md)
