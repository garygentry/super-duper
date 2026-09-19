# Isolated Windows UI development sessions

Use `scripts/Start-WindowsUiDev.ps1` to run the real WPF app and worker against disposable state,
so scans and review decisions never touch a real database or hash cache. When the interactive
desktop is unavailable, iterate with the in-process WPF surface tests instead.

## Run the app with disposable state

From the repository root:

```powershell
./scripts/Start-WindowsUiDev.ps1 -CreateFixture
```

The script:

1. Builds the worker (`cargo build -p super-duper-worker --locked --jobs 2`, plus `--release` for
   Release) and the solution (`dotnet build --disable-build-servers -m:1`). If `LIBCLANG_PATH` is
   not set in the session, it borrows the User-scope value.
2. Creates a new state directory, `artifacts/ui-dev-session/<id>`.
3. With `-CreateFixture`, writes a five-file test root under `<state>/fixture`: `Originals/trip`
   and `Copies/trip` each hold the same `photo.txt` and `notes.txt`, and `Copies/unique.txt` has
   unique content. A scan of that root finds two duplicate-file sets and one exact duplicate-folder
   set.
4. Launches the app with `SUPER_DUPER_WORKER_PATH` set to `target/<profile>/super-duper-worker.exe`
   (a Release app ignores it and starts the worker beside it) and the state variables pointed into the state directory: `SUPER_DUPER_DB_PATH`
   (`super_duper.db`), `SUPER_DUPER_STATUS_DB_PATH` (`scan_status.db`), `HASH_CACHE_PATH`
   (`hash-cache`) and `LOG_FILE_PATH` (`app.log`).

It prints:

```text
APP_PID=<process id>
STATE_DIRECTORY=<state directory>
FIXTURE_ROOT=<fixture root>
```

`FIXTURE_ROOT` appears only with `-CreateFixture`. In the app, create a saved scan, choose
**Enter path**, enter the printed fixture root and start the scan. Close the app normally; its
worker exits with it.

| Parameter | Effect |
|---|---|
| `-Configuration Debug\|Release` | Builds and launches that configuration. Default `Debug`. |
| `-SkipBuild` | Uses the existing app and worker builds. The script still fails if either is missing. |
| `-CreateFixture` | Writes the five-file fixture. Without it the state starts empty. |
| `-PrepareControlLaunch` | Writes a `.uidev` launch file instead of starting the app (Debug only). |
| `-StateDirectory <path>` | Reuses a state directory. It must be under `artifacts/ui-dev-session/`. |

To reopen the same completed scan after closing the app, pass the printed state directory back:

```powershell
./scripts/Start-WindowsUiDev.ps1 -SkipBuild -StateDirectory 'artifacts/ui-dev-session/<id>'
```

Do not run two app processes against one state directory. The app is single-instance per state
folder, and the worker refuses a database another worker holds.

## Prepare a launch for a desktop-control tool

Some desktop-control tools start an app themselves and cannot pass environment variables. For
them, prepare the same private state without launching:

```powershell
./scripts/Start-WindowsUiDev.ps1 -CreateFixture -PrepareControlLaunch
```

The script writes `SuperDuper.Windows.uidev` beside the Debug app executable and prints:

```text
CONTROL_APP=<Debug SuperDuper.Windows.exe>
STATE_DIRECTORY=<state directory>
FIXTURE_ROOT=<fixture root>
```

Launch the printed `CONTROL_APP` with the tool. The `.uidev` file holds two lines, the worker path
and the state directory. Only a Debug app reads it (Release builds do not contain this code), and
only when `SUPER_DUPER_DB_PATH` is not set. The app then requires:

- the worker to be the repository's `target/debug/super-duper-worker.exe`, and to exist;
- the state directory to exist under the repository's `artifacts/ui-dev-session/`.

If either check fails, the app stops at startup instead of falling back to the default state.
When both pass, it sets the same five variables as a direct launch. Close the app before reusing
the state, and delete the `.uidev` file after the last launch so an ordinary Debug start does not
reopen the test state.

Desktop-control tools need the interactive desktop. Native input can fail while the remote
session is backgrounded, minimized or locked. When that happens, stop sending input, record any
required native check as unrun, and continue with the work below.

## Iterate when the desktop is unavailable

`SuperDuper.Windows.Smoke.Tests` loads the shipping `App`, `MainWindow` markup and XAML views on an
STA thread in the test process, with in-memory test doubles instead of a worker. It exercises tabs,
disclosure, scrolling and decision controls, needs no worker and no interactive desktop, and can
write PNG captures of what it renders. Set one or more capture variables to a folder under
`artifacts/`, then run the project:

```powershell
$env:SUPER_DUPER_UIR05C_CAPTURES = "$PWD\artifacts\ui-captures"
dotnet test apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj --configuration Debug -m:1
```

| Variable | Fixture | Captures |
|---|---|---|
| `SUPER_DUPER_UIR03_CAPTURES` | `RedesignShellSurfaceTests` | The empty shell at 1180 × 760 and 900 × 600 |
| `SUPER_DUPER_UIR04_CAPTURES` | `SetupWorkflowFixture` | Setup, its advanced options and the prompt for leaving Setup with unsaved changes |
| `SUPER_DUPER_UIR04B_CAPTURES` | `LongScanMonitoringFixture` | Scan progress: stale, terminal and compact states |
| `SUPER_DUPER_UIR05C_CAPTURES` | `PopulatedShellFixture`, `FileQueryLayoutFixture` | Populated Files, Folders, Review, History and Performance views, themes and text scaling, and the file filter editor |

An unset variable writes nothing. Inspect the PNGs that cover your change. These captures do not
replace a check that must be done with real mouse and keyboard input on the desktop; record such a
check as unrun until the desktop is available.

## Build hygiene

- Keep builds serialized. The script disables shared .NET build servers, builds with `-m:1`, and
  limits the Rust build to two jobs to avoid build-server stalls and native compile load.
- Do not rebuild assemblies while a test host or the app is using them.
- A `--no-build` test run uses the binaries already on disk. Rebuild before using it to verify
  edited XAML or Core code.
