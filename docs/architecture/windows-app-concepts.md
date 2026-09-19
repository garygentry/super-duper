# Windows app conventions

## Purpose

These are the crosscutting rules every change to the Windows app must keep. Each rule names where
it is enforced or stated, so you can check a change against the code rather than against this page.
Repository-wide rules in `AGENTS.md` (Architecture Rules, Safety Invariants) take precedence. Paths
are relative to `apps/windows/src/` unless they start with another top-level folder.

## Layering

- **Product logic stays in Rust.** Duplicate detection, counts and bytes, survivor protection,
  overlap and revision checks, and the database schema belong in `super-duper-core`, reached only
  through the worker. The C# projects have no SQLite dependency and never open a product database
  (`AGENTS.md`, [ADR-0001](decisions/0001-worker-process-boundary.md)).
- **Core has no platform code.** `SuperDuper.Windows.Core` targets plain `net10.0` with only
  CommunityToolkit.Mvvm, so WPF, WinRT and Windows-only APIs do not compile there. View models,
  navigation, validation and contracts go here; process, file-system watching, Shell and dialog
  work go behind an interface.
- **Infrastructure owns the outside world.** The worker process and protocol, file-system
  watchers, Shell and Cloud Files calls, and persisted preferences live in
  `SuperDuper.Windows.Infrastructure`. Add native functions through `NativeMethods.txt` (CsWin32)
  rather than hand-written declarations.
- **The WPF project only presents.** `SuperDuper.Windows` holds XAML, the composition root, WPF
  service adapters and view-only code-behind (layout, focus, key handling). Screen behavior belongs
  in the view model so Core tests can cover it.
- **The protocol is a contract.** Additive fields are allowed within v1; any protocol change
  updates [worker protocol v1](../worker-protocol-v1.md) (`AGENTS.md`).
- **Development seams stay out of Release.** The `.uidev` sidecar and `SUPER_DUPER_WORKER_PATH`
  are honored only in Debug builds (`SuperDuper.Windows/App.xaml.cs`,
  `SuperDuper.Windows.Infrastructure/WorkerExecutableLocator.cs`).

## Threading

- **Nothing blocking on the dispatcher.** No database, process-stream, Shell or potentially slow
  file-system call runs on the WPF UI thread. Explorer, cloud detection and the start-time root
  reachability check run on the thread pool (`WindowsExplorerService.cs`,
  `WindowsCloudLocationService.cs`, `SessionSetupViewModel.EnsureSavedAsync`); worker I/O is
  asynchronous on dedicated pumps. The one exception is Setup validation, which reads each root's
  drive type with `DriveInfo` and checks local and removable roots with `Directory.Exists` whenever
  Setup is validated; it deliberately skips the existence check for mapped and UNC roots
  (`SuperDuper.Windows.Core/Validation/SessionDefinitionValidator.cs`).
- **View models run on the UI thread.** Their `await`s resume on the UI context; Core uses
  `ConfigureAwait(false)` only inside the progress scheduler.
- **Worker events arrive off the UI thread.** `RunProgress`, `RunLifecycleChanged`,
  `ResultStateChanged` and `UnexpectedExit` are raised on the stdout pump or exit-monitor thread.
  A handler must hand work to the UI through `IUiDispatcher.Post` or the progress gate, and must
  not throw: `WorkerClient.DispatchEvent` treats any exception as a protocol failure and stops the
  worker.
- **Progress goes through the gate.** Raw progress frames are never bound directly.
  `LatestProgressApplicationGate` admits only well-ordered frames for the active run and applies
  the newest at most every 100 ms, with one dispatcher post outstanding.
- **Stale results are discarded.** Long operations capture a generation counter or cancellation
  token and check it after each `await` before touching state (for example `_startGeneration` and
  `_navigationGeneration` in `ShellViewModel.cs`). Follow the same pattern for new async commands.

## Bounded data

- **The worker pages; the app never downloads a whole run.** Filtering, sorting and the Review
  overview are server-side queries. No list is built by fetching every result.
- **Page sizes are fixed in the view models.** Duplicate sets, copies, folder sets and Review
  overview pages: 200. Location facets and warnings: 25. Rule previews and whole-plan check items:
  100. Run history: 500. Performance comparison: 25 runs and up to 64 drives. Visited pages are
  kept in a least-recently-used cache of 5
  (`SuperDuper.Windows.Core/ViewModels/BoundedCursorCache.cs`).
- **Bulk actions are bounded.** Checking copies covers at most the 200 visible copies,
  reconciliation handles at most 200 files per batch, and Explorer selection takes 1 to 200 items.
- **Transport and diagnostics are bounded.** Frames are at most 1 MiB, the stderr tail 16 KiB, the
  worker log 5 MiB plus one previous file, preferences 16 KiB, live watching 64 roots and 200 paths
  per root per batch.
- **Filters are drafts until applied.** Query edits stay in `DuplicateFilesQueryEditor` until
  **Apply**; changing a filter never silently widens the scope of a rule or bulk action.
- **One exception.** The saved-scans list is loaded completely, 500 per request
  (`SessionListViewModel.cs`).

## Safety

- **Review-only.** Production registers `DisabledRecycleOperationCapabilityExecutor`, and
  `RecycleOperationViewModel.CanSubmit` is always false. Do not wire
  `WindowsRecycleOperationExecutor` or any other file-changing path into production without
  explicit operator direction ([ADR-0002](decisions/0002-review-only-windows-app.md), `AGENTS.md`).
- **Decisions record intent.** Selecting a row never changes a decision, and marking a copy for
  removal never touches the file. Wording must not promise reclaimed space.
- **Worker truth.** Counts, bytes and remaining-copy numbers come from worker responses; the app
  does not recompute them. Potential savings and marked-for-removal totals stay distinct.
- **The worker refuses unsafe decisions.** Survivor protection, folder overlap and revision checks
  are enforced by the worker. Mutations carry an operation ID and the expected revision, and the UI
  explains a refusal instead of working around it.
- **Historical and live state stay separate.** Scan results are immutable; live validation and
  reconciliation record current state beside them. Cancelled or failed runs show no partial
  duplicate results.
- **Execution flags are checked.** The client rejects `executorEnabled: true` in a live-state event.
- **Confirmations default to No.** `UserConfirmationService` proceeds only on an explicit Yes.
- **Tests use disposable state.** Smoke, acceptance and fault-injection runs set the database,
  status database and hash-cache variables to throwaway folders, never real user data (`AGENTS.md`).

## Accessibility

- **Names and help text.** Icon-only and ambiguous controls carry `AutomationProperties.Name`, and
  list items take their accessible name from the row text (for example
  `SuperDuper.Windows/Views/SessionListView.xaml`).
- **AutomationIds are a contract.** `scripts/Invoke-WindowsSmoke.ps1` finds about 80 distinct
  elements by `AutomationId`, the Smoke tests assert them, and expander `AutomationId`s are the keys
  for saved expansion state. Renaming one breaks the smoke and resets that preference; add new IDs
  rather than reusing old ones.
- **Announcements.** State changes that are not otherwise visible to a screen reader raise UI
  Automation notifications through
  `SuperDuper.Windows/Accessibility/AutomationNotificationBehavior.cs`: the view model increments an
  announcement version and the element's accessible name is announced. Scan progress
  announcements are throttled to one every 5 seconds.
- **Focus is restored deliberately.** Commands that move or rebuild content set a `FocusTarget`,
  which `MainWindow.xaml.cs` maps to a view's focus method; decision buttons keep focus through
  `SuperDuper.Windows/Views/DecisionActionFocus.cs`.
- **Access keys.** Labels use `_` access keys; a new one must not repeat a letter already used in
  the same scope. Existing duplicates are listed in [risks](windows-app-risks.md).
- **Not yet verified.** High contrast, Narrator and NVDA, and multi-monitor DPI behavior have not
  been checked on a real desktop (issue #23).

## Appearance

- **Follow Windows.** `SuperDuper.Windows/App.xaml` sets `ThemeMode="System"`, WPF's Fluent theme
  following Windows light or dark mode. Styles are `BasedOn` Fluent defaults and colors come from
  `DynamicResource` theme brushes; no view hard-codes a color.
- **Use the shell tokens.** `SuperDuper.Windows/Resources/ShellResources.xaml` defines the font-size
  tokens, insets, 16-DIP stroked icon geometries and shared styles (`ShellAction`,
  `ShellPrimaryAction`, `SharedCard`, `ShellStatusBanner` and others). Use them rather than literal
  sizes.
- **Text scales with Windows.** `SuperDuper.Windows/Accessibility/WindowTextScale.cs` multiplies
  the five font-size tokens by the Windows text-size factor and updates them live. Always size text
  through the tokens so it scales; hit targets and icons deliberately do not.
- **Breakpoints.** The window's minimum size is 900 by 600. The saved-scans pane closes below
  1100 pixels wide or when the scaled body font exceeds 18 (`MainWindow.xaml.cs`). Results › Files
  and Folders show one pane at a time below 960 by 500 (`NarrowWorkspaceWidth` and
  `NarrowWorkspaceHeight` in the two views).
- **Show plain paths.** The worker stores verbatim `\\?\` paths; bind them through
  `PlainPathConverter`, which is display-only and never rewrites the stored value.

## Wording

The UI uses the owner's vocabulary; code and protocol keep the technical terms. Use the left column
in anything a user reads and the right column in code, protocol and maintainer docs.

| The UI says | Code and protocol say |
|---|---|
| saved scan | session (`session.*`, `SessionSetupViewModel`) |
| scan, as in "Scan 12" | run (`run.*`) |
| duplicate set, set | duplicate file group (`duplicate_file_group.*`) |
| copy | member |
| exact-folder set, folder copy | duplicate folder group, folder member |
| **Keep copy**, **Mark copy for removal**, **Reset copy** | review decision `keep`, `remove`, `undecided` |
| **Check these copies** | live validation (`review_live_validation.run`) |
| **Reconcile next batch** | dirty root reconciliation (`review_live_root.reconcile`) |
| **Check marked copies**, whole-plan check | preflight (`preflight.*`) |
| Location preferences | preference rules (`preference_rule.*`) |

A few strings still show the technical term: validation messages and the delete confirmation say
"session" (`SuperDuper.Windows.Core/Validation/SessionDefinitionValidator.cs`,
`SessionSetupViewModel.cs`), the close prompt says "Cancel preflight and exit?"
(`ShellViewModel.cs`), and the watcher notice says "Validate page" (see
[risks](windows-app-risks.md)). New text should use the left column.

## Presentation preferences

- **Only presentation.** `presentation-preferences.json` in the state folder holds the
  saved-scans pane state, the last results mode (Files or Folders) and expander expansion
  (`SuperDuper.Windows.Core/Services/PresentationPreferences.cs`). It must never hold scan data,
  paths, decisions, or status and error text.
- **Separate from the worker.** The file is written by the app, never by the worker, so a
  presentation change cannot affect scan or decision data (`JsonPresentationPreferencesStore.cs`).
- **Bounded and forgiving.** Version 1, at most 16 KiB, at most 128 expander keys of up to 128
  characters, written by atomic replace. Any read failure falls back to defaults, and a save that
  fails with an I/O or access error is only traced (`MainWindow.xaml.cs`).
- **No theme or layout settings.** Appearance follows Windows; there is no in-app theme, font or
  layout preference.

## Related decisions

- [ADR-0001: The Windows app reaches the engine through a worker process](decisions/0001-worker-process-boundary.md)
- [ADR-0002: The Windows app is review-only](decisions/0002-review-only-windows-app.md)
- [ADR-0004: Keep WPF and recompose the app instead of migrating](decisions/0004-keep-wpf-redesign-in-place.md)
