# Review duplicate files and folders

This guide shows how to narrow a completed scan's results, compare copies, record Keep and Remove
decisions for files and folders, and open copies in File Explorer; it assumes you have a completed
scan open (see [Set up and run scans](scan-folders.md)).

Decisions record what you intend to do. They never delete, move or change a file.

## Find the sets that matter

Go to **Results** › **Files**. Every filter is a draft until you apply it.

1. To search by path, type part of a path in **Path search**. Press Ctrl+F to jump to this box.
   The search is not case-sensitive and can be up to 512 characters. To match one complete path
   instead, open **Filters** and select **Exact path**.
2. To filter further, select **Filters** and set any of these:
   - **Minimum size** and **Size unit** (B, KiB, MiB, GiB, TiB), or **1 GiB or larger**. Sizes
     apply to one copy of the file.
   - **Three or more copies** or **Across drives**.
   - **Extension** without the dot (for example `jpg`), or **No extension**. By default a set
     matches when any copy has the extension; select **All copies must match** to require every
     copy.
   - **Selected root** or **Drive**: choose one location or drive from the list. Each entry shows
     how many sets it has; the lists show 25 entries at a time and can be sorted by **Most sets
     first** or **Name A–Z**.
3. Select **Apply**, or press Enter in **Path search**, **Minimum size** or **Extension**.

While you edit, the page shows **Unapplied changes · use Apply or Enter.** After you apply, the
totals (**Sets:**, **Copies:**, **Potential savings:**) cover only the matching sets, and each
active filter appears as a chip such as **Path contains: …** or **Three or more copies**.

- To remove one filter, select its chip.
- To remove them all, select **Clear filters**.
- To collapse the filter panel, press Esc while you are in it, or select **Filters** again.

To change the order of sets, choose a sort order above the **Duplicate sets** list. The default is
**Potential savings, largest first**; you can also sort by one-copy size, number of copies, or
representative label.

**Results** › **Folders** has the same **Path search**, **Apply** and **Clear filters** controls;
its **Filters** panel offers **Minimum size** and **Size unit** only.

## Compare the copies in a set

1. Select a set in the **Duplicate sets** list. Each row shows the set's label, the number of copies,
   the size of one copy, how much you could save (**Save up to …**), and how many locations and
   drives it spans.
2. Read the copies under **Copies in selected set**. Each copy shows its path within its scanned
   location, the location itself, **Size**, **Modified**, its **Decision**, and its current state
   (for example **Not validated in this working view** or **Present; matches scan metadata**).
3. To see the set's decision totals, read the **Set review:** line above the copies, or expand **Set
   and review details** for totals across the whole scan.

The set's label is just one of its file names. It does not mean that copy is the original.

Lists show 200 sets or 200 copies at a time. Use the arrow buttons below each list (**Previous
sets** and **Next sets**, **Previous copies** and **Next copies**) to page. You can drag the divider
between the two lists to resize them.

In a narrow or short window, the page shows one list at a time. Select **Compare selected set** to
see the copies, then the **Back to sets** or **Back to copies** button to go back.

## Record a decision for a file

1. In **Results** › **Files**, select a copy under **Copies in selected set**.
2. In the panel below the list, select one of:
   - **Keep copy** to record that you want to keep this copy.
   - **Mark copy for removal** to record that you intend to remove it.
   - **Reset copy** to return it to **Undecided**.
3. Check the **Set review:** line. It shows how many copies you keep, remove and have not decided,
   and how many physical copies remain.

Super Duper refuses a decision that would leave a set with no copy to keep. If you try to mark the
last remaining copy for removal, the decision is not saved and a message explains that at least one
copy must remain. Keep or reset another copy first.

**Keep copy** and **Mark copy for removal** are unavailable for a copy that a recent check found
missing, changed or unavailable, or that is waiting to be checked. If a check invalidated your
earlier decision, the copy's state ends with **prior Keep decision invalidated** or **prior Remove
decision invalidated**; select **Reset copy** or check the copy again, then decide. See
[Check marked copies before acting on them](check-marked-copies.md).

If a decision fails for another reason, the page shows **The review decision was not saved.**
followed by the reason. Try again.

## Record a decision for a folder

Exact duplicate folders are listed under **Results** › **Folders**. Their contents match exactly,
even if the folders have different names.

1. Select a set in the **Exact-folder sets** list. Each row shows the number of folder copies, files
   per copy, size per copy and the recoverable size.
2. Select a folder copy in the comparison list. Each copy shows **Location:**, **Shared context:**
   and **Differs here:** to help you tell the locations apart.
3. Select **Keep folder**, **Mark folder for removal** or **Reset folder**.

A folder decision applies to the folder copy and everything inside it. When a file decision and a
folder decision cover the same file, the file is counted once in the review totals.

Super Duper refuses a folder decision when:

- It overlaps an existing Keep or Remove choice on a file or folder inside it. Clear that
  decision first, then try again.
- It would leave an exact-folder set without an intact copy, or a duplicate-file set without an
  accessible copy. Keep or reset another copy first.
- Your review choices changed before the update was saved. Reopen the scan and try again.

## Open or copy a location

For a file copy in **Results** › **Files**:

1. Select the copy.
2. In the panel below the list, select the **Copy path** button to copy the full path to the
   clipboard, or the **Show in Explorer** button to open File Explorer with the file selected. The
   full path is also shown in a box you can select text from.

For a folder copy in **Results** › **Folders**:

1. Select the folder copy.
2. Select **Copy path** or **Show in Explorer** next to **Complete folder path**. With the folder
   copy list focused, Alt+E does the same as **Show in Explorer**.
3. To select every folder copy on the current page in File Explorer at once, select **Select page
   in Explorer** below the list, or press Alt+G with the folder copy list focused. Explorer makes one
   selection per parent folder.

Related: [Windows app reference](app-reference.md) ·
[Apply location preferences](location-preferences.md) ·
[Check marked copies before acting on them](check-marked-copies.md) ·
[How Super Duper finds and reviews duplicates](how-it-works.md)
