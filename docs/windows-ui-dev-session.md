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
direct Debug starts do not reopen the last test state. Computer Use native input depends on access to
the interactive desktop; it may fail while the remote session is backgrounded or locked.

## Background UI iteration

Continue local UI implementation and automated verification when native input is unavailable. The
loaded-STA WPF smoke fixture drives real WPF views, tabs, disclosure, scrolling and decision controls
in process. It can render screen captures without a targetable interactive desktop. Set
`SUPER_DUPER_UIR05C_CAPTURES` to an ignored folder under `artifacts/`, then run the WPF smoke project:

```powershell
$env:SUPER_DUPER_UIR05C_CAPTURES = 'C:\Users\gary\workspace\super-duper\artifacts\vm-background-captures'
dotnet test apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj --configuration Debug --no-build -m:1
```

Inspect representative PNGs after a change. Run the Rust and Windows suites and disposable worker
checks as the selected slice requires. On 2026-09-15, the WPF smoke suite passed all three methods
and produced 125 captures while Computer Use native input was denied; narrow Results and Review PNGs
were inspected. The complete Debug Windows suite also passed in that state: 220 Core, 76 Infrastructure,
three WPF methods, with five expected physical/provider/deletion skips. The read-only Recycle Bin root
eligibility test needs the VM's normal development context because the filesystem sandbox blocks its
query; it does not execute deletion.

If Computer Use reports an inaccessible desktop, stop native input calls. Record any specifically
required native check as unrun and continue independent authorized work. Revisit that check when
interactive access returns or before a gate that explicitly requires physical desktop evidence.
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

## Real-file polish journeys

For unattended production-WPF and real-worker exercise, run:

```powershell
./scripts/Invoke-WindowsPolishJourney.ps1
# After building matching binaries for the current source:
./scripts/Invoke-WindowsPolishJourney.ps1 -SkipBuild
```

The script creates a fresh corpus from actual tracked documents/media and a ZIP of actual documents,
records source and copy hashes, and gives the worker independent databases, cache and logs beneath
`artifacts/ui-dev-session`. It exercises the shipping views and commands on a loaded STA dispatcher;
it does not install an automation endpoint in the shipping app. Mutations use a separate disposable
copy tree. Original sources and baseline copies are checked for unchanged hashes. The evidence
directory contains PNGs, a TRX result and a journey ledger with worker/UI binary hashes and owned PIDs.
Use the matching `-Configuration Release` for Release verification.

Builds are serialized for .NET with shared build servers disabled, and Rust build parallelism is
bounded to two jobs. This avoids the shared-server stalls and excessive native compilation load
observed during polish integration. Coordinate builds across agents; do not rebuild assemblies while
a test host is using them. A no-dependencies test build must not be used to verify newly edited
product XAML or Core code against stale product binaries.

The opt-in real-worker journey runs in its own test host. The existing fictional WPF smoke tests
remain useful for deterministic timing, themes and error states, but their captures are a separate
evidence category. Background real-worker WPF exercise also does not replace a required final native
mouse/keyboard pass. When capture/input for the selected active desktop is unavailable, record that check
as unrun and continue the independent work above.

## VM verification on 2026-09-15

The fixture built with zero warnings and exposed screenshot and accessibility controls. Direct input
opened Review and changed the viewport to 900 × 600. The real Debug app and worker launched with an
isolated database/cache, accepted a saved scan and root path through direct input, completed the five-file
fixture, and displayed two file sets and one exact folder set. Results, Review and History opened. Closing
the app exited the worker; relaunching with the same state restored the saved session and file results.

The resumed setup displayed the canonical long-path root as a filesystem type not classified as fixed,
removable or network. This did not block the disposable scan or result restoration; revisit that root
classification if a later UI task depends on drive-type messaging.

## Native recovery on 2026-09-16

The operator has standing approval for computer control in this dedicated VM/sandbox and explicitly
reconfirmed it after approving the Settings tool prompt. Do not request repeated computer-control
permission. A tool approval timeout does not revoke that authorization; when approval arrives,
resume from fresh discovery and observations. Tool-enforced access still has to succeed before
claiming native evidence. Keep task scope, isolated state and deletion restrictions unchanged.

Do not infer desktop lock from the presence of a LogonUI process alone. In this VM, LogonUI PID 1220
belonged to session 1 while the app and working native input ran in session 2. Judge availability
from actual selected-session capture/input. Prefer fresh `sky.list_apps()` selection if
`sky.list_windows()` omits the app; this occurred even while app capture/input worked. Never invent
window handles from process inspection. Follow the skill's bounded recovery for actual timeouts,
then continue independent work. Operator authorization remains in force; availability is a technical
prerequisite, not a request for renewed permission or an obligation to unlock for background work.

The native pass opened and cancelled the Windows folder picker, started a real rescan, observed
completion navigation to Results, compared copies, marked one for removal, clicked Copy path, and
launched Explorer at the containing folder. Explorer capture hit an app-approval timeout; no claim
is made that selection/clipboard contents were verified. Subsequent app capture/discovery timed out,
and a kernel reset plus one discovery retry also timed out. Native acceptance remains partial.
The owned app/worker were stopped and the Debug launch sidecar removed after recovery failed.

This pass reproduced canonical local-path misclassification in Setup. The classifier now converts
only an extended drive root for DriveInfo lookup, preserving the stored path, and recognizes extended
UNC roots before any network reachability check. See the polish evidence for test results.
