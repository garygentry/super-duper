# Windows Developer Smoke Workflow

`scripts/Invoke-WindowsSmoke.ps1` creates a disposable deterministic filesystem fixture and drives
the real worker protocol. On an interactive desktop it also launches the real WPF application and
uses stable UI Automation IDs to exercise both result surfaces and invoke Explorer reveal.

## Coverage

- session creation and protocol negotiation;
- active-run cancellation and durable `cancelled` state;
- a completed rerun and restoration after worker restart;
- more than one page of duplicate-file groups, sorting, filtering, forward cursor paging, and
  member browsing;
- exact duplicate-folder sorting, filtering, and member browsing;
- fixed-drive scanning, a path longer than 260 characters, a locked file, and a skipped junction;
- all five scan-phase timings and all four result-query timings on stderr;
- WPF startup/restoration, duplicate-file and exact-folder tabs, grid sorting, paging, filtering,
  row selection, completed ordinary/long-path file and folder Explorer reveal commands, and
  deterministic result-loaded, repeated idle, startup-failure, and database-failure shutdown.

Run Debug or Release:

```powershell
./scripts/Invoke-WindowsSmoke.ps1
./scripts/Invoke-WindowsSmoke.ps1 -Configuration Release
```

Useful options:

```powershell
# Protocol/worker smoke suitable for a headless agent
./scripts/Invoke-WindowsSmoke.ps1 -SkipWpf

# Use already-built binaries or retain the fixture for diagnosis
./scripts/Invoke-WindowsSmoke.ps1 -SkipBuild
./scripts/Invoke-WindowsSmoke.ps1 -KeepArtifacts

# Exercise real removable, mapped, or UNC test roots as best-effort additions
./scripts/Invoke-WindowsSmoke.ps1 -AdditionalRoot 'E:\Archive','Z:\Team','\\server\share'
```

The built-in fixture exercises the local fixed drive hosting `%TEMP%`. Removable media, mapped
drives, and UNC shares cannot be fabricated portably, so pass real, non-production test roots with
`-AdditionalRoot`. The fixed fixture root remains available, which lets an unavailable additional
root become a warning instead of preventing the run.

## Expected Result

The script prints `Windows smoke passed`. With WPF enabled it also prints that WPF automation
passed, fails if either reveal action reports a detail error, verifies that owned workers do not
survive the app, and may leave an Explorer window showing a selected disposable fixture item. By
default the fixture is removed after the app closes; `-KeepArtifacts` prints and retains its path.

If UI Automation is blocked by a locked session, elevation boundary, or headless runner, rerun with
`-SkipWpf`, then perform the WPF portion manually on an interactive Windows 11 desktop:

1. Select `Milestone 6 Smoke` and its completed run; confirm the cancelled run is also in history.
2. Open Duplicate Files, sort Group size, choose Next, filter for `group010`, select a group, and
   choose Show in Explorer.
3. Open Duplicate Folders, sort Representative folder, filter for `original-set`, select the group,
   and reveal a folder in Explorer.
4. Close and reopen the app and confirm completed/cancelled history and completed results restore.
