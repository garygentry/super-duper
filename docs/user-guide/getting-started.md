# Find your first duplicates

By the end of this tutorial you will have scanned a small practice folder with Super Duper, found
the two identical files inside it, marked one of them for removal, and checked that plan. Nothing
on your PC is deleted along the way: Super Duper is review-only and never deletes, moves or changes
the files it scans.

The tutorial takes about ten minutes.

## Before you start

You need:

- A PC running Windows 11 x64.
- The Super Duper release zip, `super-duper-<version>-win-x64.zip`, from the
  [GitHub Releases page](https://github.com/garygentry/super-duper/releases). Download the zip for
  the newest release to your Downloads folder.

You also need a practice folder that contains two copies of the same file and one file that exists
only once. Create it now:

1. Open the Start menu, type **PowerShell**, and open **Windows PowerShell** or **PowerShell**.
2. Paste these lines and press Enter:

   ```powershell
   New-Item -ItemType Directory -Path C:\SuperDuperPractice\A, C:\SuperDuperPractice\B | Out-Null
   Set-Content -Path C:\SuperDuperPractice\A\notes.txt -Value 'Practice file for Super Duper. Both copies of this file hold exactly this text.'
   Copy-Item -Path C:\SuperDuperPractice\A\notes.txt -Destination C:\SuperDuperPractice\B\notes.txt
   Set-Content -Path C:\SuperDuperPractice\B\shopping.txt -Value 'Milk, bread, apples.'
   ```

3. Close PowerShell.

You now have this folder:

```text
C:\SuperDuperPractice
  A\notes.txt       (a copy)
  B\notes.txt       (an identical copy)
  B\shopping.txt    (a file that exists only once)
```

## Step 1 — Start Super Duper

1. Open your Downloads folder in File Explorer.
2. Right-click `super-duper-<version>-win-x64.zip` and choose **Extract All**, then choose
   **Extract**. Windows creates a folder named `super-duper-<version>-win-x64`.
3. Open that folder and double-click `SuperDuper.Windows.exe`.
4. The app is not code-signed, so Windows SmartScreen may say that it protected your PC. If it
   does, choose **More info**, then **Run anyway**.

The Super Duper window opens and shows **Starting worker** for a moment while it starts its scan
engine. Then you see **Find your duplicate files** with a **Choose folders** button.

## Step 2 — Choose the practice folder

1. Select **Choose folders**.

   The **Choose locations** page opens. It shows one empty location box with the note **Enter an
   absolute folder or drive path.**

2. Select **Add folder or drive**.
3. In the folder window, go to `C:\SuperDuperPractice`, select it, and choose **Select Folder**.

   The location box now shows `C:\SuperDuperPractice`, with the note **Location configured;
   availability is checked again on Start.**

4. Under **Saved scan name**, replace the suggested name with `Practice`.

Below the name, **Scan settings** shows a line about cloud locations. Wait until it reads **No
registered cloud locations intersect the selected scan roots.** Super Duper checks for cloud sync
folders such as OneDrive before every scan, and the practice folder is not one of them.

A saved scan is the list of locations you want to compare. You can scan it again whenever you like.

## Step 3 — Run the scan

1. Select **Start scan** in the top-right corner of the window.

   Super Duper saves the **Practice** saved scan and switches to the **Progress** page. The large
   title shows the current phase, such as **Discovering files** or **Hashing candidates**.

2. Stay on the **Progress** page and wait. The practice folder is tiny, so the scan finishes in a
   few seconds.

When the scan completes, Super Duper opens **Results** and its **Files** page for you. If you left
the **Progress** page while the scan ran, a banner reading **Scan complete** appears instead;
select **View results** in that banner.

## Step 4 — Compare the copies

The **Files** page lists duplicate sets on the left and the copies in the selected set on the right.

1. Under **Duplicate sets**, select the set labelled **notes.txt**. It shows **2 copies**.

   The right side lists the two copies under **Copies in selected set**: `A\notes.txt` and
   `B\notes.txt`. Each one shows **Decision: Undecided**. The file `shopping.txt` is not listed
   anywhere, because it has no duplicate.

2. Select the `B\notes.txt` copy.

   A panel opens below the list. It shows **Current decision: Undecided**, three decision buttons,
   and the copy's **Full path**.

3. Under the full path, select the second icon button, whose tooltip reads **Show in Explorer**.

   File Explorer opens `C:\SuperDuperPractice\B` with `notes.txt` selected. Close File Explorer and
   return to Super Duper.

## Step 5 — Mark a copy for removal

1. With `B\notes.txt` still selected, select **Mark copy for removal**.

   The copy now shows **Decision: Remove**. The **Set review:** line above the list now counts one
   copy to remove and ends with **1 physical copy remains**.

2. Open `C:\SuperDuperPractice\B` in File Explorer. `notes.txt` is still there.

Marking a copy records what you intend to do. It does not delete anything. Super Duper also keeps
at least one copy of every set: it would refuse to mark `A\notes.txt` for removal as well.

## Step 6 — Check the plan

Before you act on a plan, Super Duper can check that the files still match what the scan saw.

1. Select the **Review** tab at the top of the window.

   The **Review marked copies** page opens. The **Marked for removal** card shows **1 file
   marked**. Under **Check the whole plan** you see **Plan has not been checked**.

2. Select **Check marked copies**.
3. A **Check marked copies?** message explains that the check reads file content and deletes
   nothing. Select **Yes**.

   **Whole-plan check in progress** appears briefly. When the check finishes, the page shows
   **Checked against your current decisions** and **Ready**, with the note **Marked copies and the
   copies you are keeping passed the check. Nothing has been deleted.**

The **Whole-plan check details** list further down shows each item the check looked at, labelled
as a removal (the copy you marked) or a survivor (the copy you are keeping).

## What you did

You created a saved scan, scanned a folder, found a set of identical files, recorded a decision to
remove one copy, and confirmed that the plan still matched the files on disk. Your files are
unchanged. When you are done, you can delete `C:\SuperDuperPractice` yourself in File Explorer.

Super Duper never deletes files for you. When you are ready to act on a real plan, you remove the
marked copies yourself, for example from File Explorer.

Next steps:

- [Set up and run scans](scan-folders.md) of your own folders and drives.
- [Review duplicate files and folders](review-duplicates.md) with filters, sorting and folder
  decisions.
- [Check marked copies before acting on them](check-marked-copies.md).
- [How Super Duper finds and reviews duplicates](how-it-works.md) explains what happened in each
  step.
