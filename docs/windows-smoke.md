# Windows smoke workflow

`scripts/Invoke-WindowsSmoke.ps1` builds a disposable filesystem fixture, drives the real worker
over its JSONL protocol, and on an interactive desktop launches the real WPF app and drives it
through UI Automation. It never touches your real database, hash cache or files.

## Run the smoke

```powershell
./scripts/Invoke-WindowsSmoke.ps1
./scripts/Invoke-WindowsSmoke.ps1 -Configuration Release
```

| Parameter | Effect |
|---|---|
| `-Configuration Debug\|Release` | Selects `target/<debug\|release>/super-duper-worker.exe` and the matching app build. Default `Debug`. |
| `-SkipBuild` | Skips `cargo build -p super-duper-worker` and `dotnet build` of the solution; uses what is on disk. |
| `-SkipWpf` | Runs only the worker protocol half. Use it on a headless agent or when UI Automation is blocked. |
| `-KeepArtifacts` | Keeps the fixture folder after the run instead of deleting it. |
| `-AdditionalRoot <path[]>` | Adds real removable, mapped or UNC test roots to the scan as best-effort extras. |
| `-AppPath <path>` | The WPF app to drive, for example a published `SuperDuper.Windows.exe`. It does not change the worker used by the protocol half. |

The protocol half always launches `target/<profile>/super-duper-worker.exe` directly. The WPF half
launches the app: a Debug app uses that same worker through `SUPER_DUPER_WORKER_PATH`, and a Release
app uses the worker beside it. `Verify-WindowsRelease.ps1` passes `-AppPath` so the WPF half drives
the published app.

The built-in fixture uses the local fixed drive that holds `%TEMP%`. Removable media, mapped drives
and UNC shares cannot be created portably, so pass real, non-production roots with
`-AdditionalRoot`:

```powershell
./scripts/Invoke-WindowsSmoke.ps1 -AdditionalRoot 'E:\Archive','Z:\Team','\\server\share'
```

An unavailable additional root becomes a warning; the fixture root still scans.

## What it checks

The fixture lives in `%TEMP%\super-duper-windows-smoke-<id>`. It holds 230 two-copy file sets (one
with a third `.JPG` copy), a pair of files without extensions, three identical folders, a file
under a path longer than 260 characters, a junction that must be skipped, and a file held open
without sharing so hashing records a warning.

Worker protocol half:

- Protocol negotiation, a cancelled run that stays `cancelled`, and a completed run.
- Warning drilldown (`warning.page`): the counted warnings are fully accounted for, with one to
  three examples each, and survive a worker restart.
- Progress frames: increasing sequence and revision, and the current contract versions.
- Completed history and results restored after a worker restart.
- Duplicate-file paging, sorting and filtering: size, copy count, drives, selected-root and drive
  facets, extension (any copy or all copies), no extension, and exact-path matching.
- Duplicate-folder paging, sorting, filtering and members.
- File and folder review decisions; a preferred-location rule saved, previewed, applied and
  reversed; and preflight start and replay. Every reviewed fixture file stays present and
  unchanged.
- Live validation after a fixture file is deleted and restored, a 1,000-event watcher hint batch,
  and an injected watcher overflow that keeps its root marked dirty across a restart.
- Recycle operation preparation, replay and item paging with `executorEnabled:false`; no Shell or
  Recycle Bin API is called.
- Timing records on stderr for five scan phases (`discovering`, `hashing`, `persisting`,
  `analyzing_folders`, `finalizing`) and eleven result queries: `duplicate_file_group.page`,
  `duplicate_file_group.members`, `duplicate_file_selected_root_facet.page`,
  `duplicate_file_drive_facet.page`, `duplicate_folder_group.page`,
  `duplicate_folder_group.members`, `review_plan.get`, `review_folder_group.page`,
  `preference_rule.preview`, `preflight.item.page` and `recycle_operation.item.page`.

WPF half (skipped by `-SkipWpf`):

- The restored completed run in Setup, Progress and History, including cloud location refresh.
- Warning drilldown: **Open duplicate results** (Alt+O) opens that run's duplicate files with focus
  in the set list, and closing the drilldown returns focus to run history.
- File filters, sorting, paging, previous and next set with focus restoration, and the filtered
  summary.
- The dirty-root warning and one **Reconcile next batch** request, a real watcher burst shown as a
  coalesced status, a review decision, and **Check these copies** after a copy is changed outside
  the app and then restored.
- Location preference preview, apply and reverse; preflight confirmation and summary; and the
  disabled Recycle Bin operation notice.
- **Show in Explorer** for an ordinary and a long-path file; for folders, Alt+E reveal, Alt+G
  grouped selection, a partial failure and a missing-location failure. Explorer windows opened on
  the fixture are closed at the end.
- A second launch with cloud registration discovery disabled
  (`SUPER_DUPER_DISABLE_CLOUD_REGISTRATION_DISCOVERY=1`), where scans must stay blocked.
- Four close scenarios: two idle closes, a close after worker startup failure (an app copy with no
  worker), and a close after a database failure. Each checks that the app exits within 10 seconds
  with exit code 0 and that no worker it started outlives it.

## Read the result

A full run prints these lines, in order:

```text
WPF automation passed for restored run <run id>, including …
WPF cloud setup automation passed, including deterministic provider-unavailable fail-closed start behavior.
WPF shutdown passed: idle connected close 1
WPF shutdown passed: idle connected close 2
WPF shutdown passed: worker startup failure close
WPF shutdown passed: database failure close
Windows smoke passed. Fixture: <fixture path>
```

With `-SkipWpf`, only the last line appears. The last line always names the fixture path. Without
`-KeepArtifacts` the script deletes that folder afterwards, including after a failure; with it,
the folder stays for diagnosis.

Any failure stops the script with an error naming the check that failed.

## Run the WPF part by hand

If UI Automation is blocked by a locked session, an elevation boundary or a headless runner, run
the protocol half and keep its fixture:

```powershell
./scripts/Invoke-WindowsSmoke.ps1 -SkipWpf -KeepArtifacts
```

Then start the app on an interactive Windows 11 desktop against the fixture's database:

```powershell
$fixture = '<path printed after "Fixture:">'
$env:SUPER_DUPER_DB_PATH = "$fixture\smoke.db"
$env:HASH_CACHE_PATH = "$fixture\hash-cache"
dotnet run --project apps/windows/src/SuperDuper.Windows/SuperDuper.Windows.csproj
```

1. Choose **Saved scans** and select **Milestone 6 Cancellation**; in **History** > **Scans**,
   confirm its cancelled run. Select **Milestone 6 Smoke** and confirm its completed run. If asked
   to save setup changes, choose **Discard**.
2. In **History** > **Scans**, select the completed run and choose **Review warnings**. On the hash
   warning, choose **Open duplicate results** (Alt+O). Confirm **Results** > **Files** opens for
   the same run with focus in the set list. Go back to **History** > **Scans**, confirm the
   warning is unchanged, and choose **Return to run history**.
3. In **Results** > **Files**, confirm the warning that working results are dirty and
   reconciliation is required. Choose **Reconcile next batch** (Alt+X). Confirm the status reports
   the copies checked (at most 200 per request) and focus returns to the copy list. If more remain,
   the warning stays; otherwise the status says the overflow dirty marker is cleared. No file moves
   or disappears.
4. Choose **Filters** and check each filter with **Apply**, then clear it:
   - **1 GiB or larger**: no sets match.
   - **Three or more copies**: the three-copy set matches.
   - `JPG` in **Extension**: exactly one set matches. Add **All copies must match**: none match.
   - Clear **Extension**, check **No extension** with **All copies must match** still checked:
     exactly one set (the extensionless pair) matches.
   - A root in **Selected root**, then a drive in **Drive**: each shows as an applied filter.

   Choose **Clear filters** when done.
5. Sort by **One-copy size, largest first**, go to the next page of sets, and select a set. Use the
   previous and next duplicate set buttons and confirm focus returns to the selected set.
6. Enter `group010` in **Path search** and choose **Apply**. Confirm the summary and **Location
   coverage and largest opportunity** show the selected-root and drive counts. Select a copy and
   choose **Show in Explorer**.
7. Check **Exact path**, replace the search with a complete copy path, and choose **Apply**:
   exactly one set matches. Clear **Exact path**, search for `long-a.txt`, select a copy and
   choose **Show in Explorer** to reveal the long-path file.
8. In **Results** > **Folders**, sort by **Representative path, A–Z**, enter `original-set` in
   **Path search**, choose **Apply**, and select the set. Confirm three folder copies. Select one
   and choose **Show in Explorer** (Alt+E from the copy row), then **Select page in Explorer**
   (Alt+G) and confirm Explorer selects the copies grouped by parent folder.
9. In **Results** > **Files**, rapidly change the last-write time of one visible copy and restore
   the exact original time. Confirm a status says filesystem events were coalesced into bounded
   path hints, and the copy shows **Validation pending after a coalesced filesystem hint**. A hint
   is not a result; choose **Check these copies** (Alt+V) for an authoritative check.
10. Select a copy and choose **Mark copy for removal**. Outside the app, change that file's length,
    then choose **Check these copies**. Confirm the copy shows **Changed since scan; prior Remove
    decision invalidated** and the file still exists. Restore the exact bytes and last-write time,
    choose **Check these copies** again, and confirm the copy shows **Present; Remove decision
    remains invalidated until reviewed again**. Choose **Mark copy for removal** again to record a
    fresh decision. Do not use a cloud placeholder for this step.
11. Close and reopen the app with the same environment variables. Confirm run history, results and
    the latest check results are restored.

When you finish, close the app, remove the two environment variables from the session, close any
Explorer windows showing the fixture, and delete the fixture folder.

## Opt-in real provider tests

The smoke never calls the Recycle Bin or a cloud provider. The tests that do are opt-in categories
in the Infrastructure test project: `RealRecycleBin` (moves disposable fixture files into the
current user's Recycle Bin), `RealRecycleBinProvider` and `RealCloudProvider`. Their environment
variables and commands are in
[`windows-testing.md`](windows-testing.md#opt-in-real-environment-categories).
