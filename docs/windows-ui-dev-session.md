# Isolated Windows UI Development Sessions

On the dedicated Windows 11 VM, use the real WPF app for worker-backed UI journeys and the fictional
redesign fixture for fast, in-memory screen exploration. Both can be inspected through Codex Computer
Use, which exposes the window screenshot and accessibility tree and can send direct mouse and keyboard
input. Select the exact `Super Duper` or `FICTIONAL FIXTURE` window after each launch; window IDs change
when a process restarts. Capture a fresh state after every UI action before using an element index or
screenshot coordinate again. A newly launched window can take a moment to appear in the desktop window
list; refresh the list before selecting it. Bring the chosen window to the foreground before
screenshot-backed input. If its geometry is temporarily unavailable, reacquire that returned window,
activate it, and capture a new state before retrying the input.

## Real app with disposable state

From the repository root:

```powershell
./scripts/Start-WindowsUiDev.ps1 -CreateFixture
```

The script builds the matching Rust worker and Windows solution, launches the Debug WPF app, and prints
`APP_PID`, `STATE_DIRECTORY`, and `FIXTURE_ROOT`. Its default state directory is a new ignored
`artifacts/ui-dev-session/<id>` folder, so scans and reviews do not use a production database or cache.
For Codex Computer Use on this VM, prepare the same private state without launching from the shell:

```powershell
./scripts/Start-WindowsUiDev.ps1 -CreateFixture -PrepareControlLaunch
```

The script prints `CONTROL_APP`, `STATE_DIRECTORY`, and `FIXTURE_ROOT`. Launch the printed existing
Debug app executable through Computer Use, then select its exact `Super Duper` window. The Debug app
reads the generated `.uidev` sidecar only when no explicit database environment variable is present.
It accepts only the matching Debug worker and a state directory under the ignored UI-development tree.
The worker/database/cache/log paths stay private. Close the
app before reusing the state; remove the generated `.uidev` file after the final launch so ordinary
direct Debug starts do not reopen the last test state. A locked Windows desktop must be unlocked before
Computer Use can capture or send input.
In the app, create a saved scan, choose **Enter path**, enter the printed fixture root, and start the
scan. The small fixture has five files: two duplicate-file sets and one exact duplicate-folder set.
Use Results, Review and History to inspect the completed run. Close the app normally; its private
worker should exit too.

For a quick relaunch after a successful matching build, use `-SkipBuild`. To restore the same completed
test run across a restart, pass the prior `STATE_DIRECTORY` through `-StateDirectory` after closing
the earlier app. The override must remain under the ignored `artifacts/ui-dev-session` tree. Do not
run two app processes against one state directory.

```powershell
./scripts/Start-WindowsUiDev.ps1 -SkipBuild -StateDirectory 'artifacts/ui-dev-session/<id>'
```

`-Configuration Release` builds and launches the Release pair. Without `-CreateFixture`, the new state
starts empty and no test files are written.

## Fictional fixture

```powershell
./scripts/Invoke-UiRedesignFixture.ps1
```

The printed executable can be launched for manual or Codex Computer Use inspection. The fixture uses
in-memory services and never calls the shipping app startup, worker, database, Explorer or deletion.
Its 900 × 600 and 1180 × 760 controls make viewport review repeatable. It is useful for the populated
Results/Review/History and delayed-response screens before repeating a change in the worker-backed app.

## VM verification on 2026-09-15

The fixture built with zero warnings and exposed screenshot and accessibility controls. Direct input
opened Review and changed the viewport to 900 × 600. The real Debug app and worker launched with an
isolated database/cache, accepted a saved scan and root path through direct input, completed the five-file
fixture, and displayed two file sets and one exact folder set. Results, Review and History opened. Closing
the app exited the worker; relaunching with the same state restored the saved session and file results.

The resumed setup displayed the canonical long-path root as a filesystem type not classified as fixed,
removable or network. This did not block the disposable scan or result restoration; revisit that root
classification if a later UI task depends on drive-type messaging.
