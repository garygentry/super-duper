# Fix startup and saved-data problems

This guide tells you what to do for each startup, connection and saved-data message Super Duper can
show, and how to back up, reset and upgrade your saved data; it assumes you know where you unzipped
the app.

Super Duper keeps its data in `%LOCALAPPDATA%\SuperDuper` unless you have changed that with the
advanced settings in [Files the app keeps](app-reference.md#files-the-app-keeps). Every saved-data
message ends with **Data:** followed by the path of the database it tried to open.

Maintainers diagnosing the worker process should also read
[Windows diagnostics, limitations and recovery](../windows-recovery.md).

## The app says it is already open

Super Duper runs one window per data folder. If you start it while it is already running, the open
window comes to the front and the new one closes. This is expected.

If you see **Super Duper is already open** instead of the app:

1. Switch to the other Super Duper window and use that one.
2. If you would rather use this window, close the other one, then select **Reconnect** here.
3. If you cannot find another window, open Task Manager and look for `SuperDuper.Windows.exe` or
   `super-duper-worker.exe`. End them, then start Super Duper again.

## The scan engine stopped

Super Duper does its work in a separate scan engine process that starts with the app. When the
engine cannot start or stops unexpectedly, the window shows a recovery screen with a **Reconnect**
button, and the status bar at the bottom reads **Scan engine:** followed by the problem.

### Worker connection failed

The engine did not start.

1. Expand **Technical details**. It shows the **Worker executable** path Super Duper tried to start
   and the **Diagnostic log** path.
2. Check that `super-duper-worker.exe` is in the same folder as `SuperDuper.Windows.exe`. If it is
   missing, extract the whole release zip again and run the app from the extracted folder, without
   moving files out of it.
3. Select **Reconnect**.

### Worker exited unexpectedly

The engine stopped while the app was running. Your completed scans, results and decisions are not
affected.

1. Select **Reconnect**. The window shows **Restarting worker**, then **Worker recovered**.
2. If a scan was running, it is now shown as **Interrupted**. Its partial results are not shown.
   Start the scan again with **Scan again**.
3. If **Worker restart failed** appears instead, check the diagnostic log (see
   [Find the logs](#find-the-logs)), then select **Reconnect** again.

## Saved data can't be opened

These messages appear when the engine starts but cannot use its database.

### Super Duper is already open

Another Super Duper window is using this data. See
[The app says it is already open](#the-app-says-it-is-already-open).

### Saved data is from a newer version

The data was created by a newer version of Super Duper, and this version cannot open it. Nothing was
changed.

- Run the newer version instead.
- If you need this older version, restore a backup you made before the upgrade (see
  [A newer version can't be undone](#a-newer-version-cant-be-undone)).

### Saved data can't be upgraded

The data comes from an early development version that cannot be upgraded. Nothing was changed.

1. Close Super Duper.
2. Move the database named after **Data:** to another folder (see
   [Start again with empty saved data](#start-again-with-empty-saved-data)).
3. Start Super Duper. It creates new, empty saved data.

### Saved data can't be read

The database appears to be damaged. Nothing was changed.

1. Close Super Duper.
2. Move the database named after **Data:**, together with any files beside it with the same name
   ending in `-wal` and `-shm`, to a safe place.
3. Start Super Duper. It starts with an empty history.

Keep the moved files if you want to try recovering them later.

### Saved data can't be written

Super Duper cannot write to its data.

1. In File Explorer, check that the database file and its folder are not read-only, and that your
   Windows account has permission to change them.
2. Select **Reconnect**.

### Saved data location is unavailable

Super Duper could not open its data folder.

1. Check that the folder in the **Data:** path exists and is reachable. If you moved the data to
   another drive with an advanced setting, make sure that drive is connected.
2. Select **Reconnect**.

### Disk is full

There is not enough free space on the drive that holds Super Duper's data.

1. Free some space on that drive.
2. Select **Reconnect**.

### Saved data couldn't be opened

Any other database problem shows this title with the engine's own explanation. Check the diagnostic
log (see [Find the logs](#find-the-logs)), fix what it reports, and select **Reconnect**.

## A banner says Something needs attention

A **Something needs attention** banner shows an error from the last action, for example a saved
scan that could not load. Read the message, select **Dismiss**, and try the action again.

## The app closes with an unexpected error

If Super Duper hits a bug it cannot recover from, it shows **Super Duper ran into an unexpected
error and needs to close. Nothing was changed. Details were written to** followed by the diagnostic
log path, then closes. Nothing was changed: your saved scans, results and decisions are exactly as
they were before.

1. Start Super Duper again.
2. If it keeps happening, check the diagnostic log (see [Find the logs](#find-the-logs)) for the
   exception it recorded, and report it with that detail.

## Back up or reset your saved data

Your saved scans, results and decisions live in `super_duper.db` in the data folder. While the app
runs, the database may also have `super_duper.db-wal` and `super_duper.db-shm` files beside it; they
hold recent changes and belong with the database.

### Make a backup

1. Close Super Duper. If a scan is running, let it finish or cancel it first.
2. Open `%LOCALAPPDATA%\SuperDuper` in File Explorer.
3. Copy `super_duper.db`, and `super_duper.db-wal` and `super_duper.db-shm` if they exist, to your
   backup location together. Also copy `scan_status.db` and any `-wal` and `-shm` files beside it
   if you want to keep performance history.

Always copy the database and its `-wal` and `-shm` files at the same time, with the app closed.
Copying only some of them can produce a backup that cannot be opened. You do not need to back up
`content_hash_cache.db`; it only speeds up repeat scans.

To restore, close Super Duper and copy the backed-up files back into the data folder, replacing the
ones there.

### Start again with empty saved data

1. Close Super Duper.
2. Move `super_duper.db` and `scan_status.db`, with any `-wal` and `-shm` files beside them, out of
   `%LOCALAPPDATA%\SuperDuper` to another folder.
3. Start Super Duper. It creates new, empty saved data.

Moving these files never touches the files you scanned.

## Find the logs

The scan engine's diagnostic log is:

```text
%LOCALAPPDATA%\SuperDuper\logs\worker.log
```

When the log grows to about 5 MB, Super Duper renames it to `worker.log.previous` and starts a new
`worker.log`, so you have at most two files. The logs stay on your PC and can contain the paths of
scanned files; review them before sharing them with anyone.

## A newer version can't be undone

A newer version of Super Duper may upgrade your saved data the first time it opens it. After that,
older versions cannot open the data and show **Saved data is from a newer version**.

Before you run a newer version for the first time:

1. Close Super Duper.
2. Copy the whole `%LOCALAPPDATA%\SuperDuper` folder to a backup location. This includes the
   database, its `-wal` and `-shm` files, and the hash cache, which a newer version may also
   upgrade.

To go back to the older version later, close Super Duper and put that copy back in place of the
folder. Changes you made with the newer version are not in the backup.

Related: [Windows app reference](app-reference.md#files-the-app-keeps) ·
[How Super Duper finds and reviews duplicates](how-it-works.md#everything-stays-on-this-pc)
