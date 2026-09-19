# Look into a past scan

This guide shows how to reopen an earlier scan of a saved scan, see the settings it used, and
investigate its warnings and performance; it assumes you have run at least one scan (see
[Set up and run scans](scan-folders.md)).

Every scan is kept as a separate record. Opening an old scan shows its results and decisions as
they were recorded; it does not rescan anything.

## Open an earlier scan

1. Select the saved scan in the **Saved scans** pane.
2. Go to **History** › **Scans**. The heading reads **History ·** followed by the saved scan's name.
3. Select a scan in the list to highlight it. Scans are listed newest first, 500 per page; each row
   shows the start time, status, **Duration:** and a short outcome such as the number of exact file
   sets, potential savings and warnings.

   Highlighting only shows details. The card below the list says whether the highlighted scan is
   the one open in the workspace, for example **Highlighted only. The workspace remains on …**

4. Select **Open scan**, or press Alt+E.

Where the scan opens depends on its status:

- **Completed** — **Results** › **Files**, with its results and review decisions.
- **Pending**, **Scanning** or **Cancelling** — **Scan** › **Progress**.
- **Cancelled**, **Failed** or **Interrupted** — **Scan** › **Summary**, which shows the last
  progress it reported. These scans have no results.

Use **Previous scans** and **Next scans** to page through a long history, and **Refresh** to reload
the list.

## See the settings a scan used

1. Highlight the scan in **History** › **Scans**.
2. Expand **Recorded settings and locations** in the card below the list.

The card lists the number of locations, the **Repeat scans** choice, the number of ignore patterns
and recorded exclusions, and the exact locations the scan covered. These are the settings at the
time of that scan. Editing the saved scan later changes future scans only; it cannot change these
settings, results or decisions.

## Investigate warnings

A warning means part of the scan could not be examined as expected, for example a file that could
not be read or changed while it was being read.

1. Open the warning list in one of these ways:
   - In **History** › **Scans**, highlight the scan and select **Review warnings**, or press
     Alt+W.
   - On **Scan** › **Progress** or **Scan** › **Summary**, select **Review warnings**. It is
     available when **Warnings:** is more than zero.
2. Read each row. It shows **Count:**, **Severity:**, the phase in which it happened, a message, and
   up to three **Example:** paths. Expand **Warning code and category** for the exact code.
3. Rows are shown 25 at a time; select **Next warning page** for more. Select **Refresh current
   warnings** to reload the list, for example while a scan is still running.
4. For some hashing warnings, select **Open duplicate results** to go to that scan's **Results** ›
   **Files** page. Select **Cancel navigation** to stop while it loads.
5. When you are done, select the return button: **Return to run history**, **Return to progress**
   or **Return to scan summary**, depending on where you came from.

Expand **Warning details** for the location of the diagnostic log. The log adds detail for
troubleshooting; the counts in the list are the scan's own record. See
[Find the logs](troubleshooting.md#find-the-logs).

## See why a scan took as long as it did

1. Open the performance summary in one of these ways:
   - In **History** › **Scans**, highlight the scan and select **Performance details**.
   - On **Scan** › **Progress** or **Scan** › **Summary**, select **Performance details**.
   - Select **History** › **Performance** to see the scan that is open in the workspace.
2. Read the sections:
   - **Run health** — status and duration, **Full-read throughput** and **Warnings**.
   - **Phase durations** — how long each phase was active.
   - **Cache and read summary** and **Candidate funnel** — how much content was read, how much was
     reused from earlier scans, and how many files were narrowed down at each step.
   - **Host and drive summaries** — **System CPU**, **Process memory**, and, when you select a
     drive, its current and peak **Read throughput**, **Read operations**, **Average latency**,
     **Active time** and **Queue depth**. Up to 64 drives are listed.
3. To compare with an earlier scan, select a run under **Retained run comparison** (the newest 25
   are listed) and select **Compare selected run**, or press Alt+C. The result shows **Current**
   and **Selected prior** values side by side, and says whether the two runs are comparable or
   which context differs.
4. Select **Refresh** to reload, and the return button to go back to where you came from.

Values that were not recorded are shown as unavailable; the app does not estimate them.

Related: [Windows app reference](app-reference.md#history) ·
[Why repeat scans are faster](how-it-works.md#why-repeat-scans-are-faster) ·
[Fix startup and saved-data problems](troubleshooting.md)
