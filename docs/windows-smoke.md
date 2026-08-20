# Windows Developer Smoke Workflow

`scripts/Invoke-WindowsSmoke.ps1` creates a disposable deterministic filesystem fixture and drives
the real worker protocol. On an interactive desktop it also launches the real WPF application and
uses stable UI Automation IDs to exercise both result surfaces and invoke Explorer reveal.

## Coverage

- session creation and protocol negotiation;
- active-run cancellation and durable `cancelled` state;
- a completed rerun and restoration after worker restart;
- more than one page of duplicate-file groups, sorting, filtering, forward cursor paging, and
  member browsing, including the worker-owned filtered review summary, bounded per-group
  selected-root/drive span, aggregate filtered location coverage, the worker-owned across-drives
  and minimum-copy-count filters, the one-copy-size minimum and accessible 1 GB-or-larger preset,
  exact canonical-member-path matching while preserving literal path substring search, indexed
  any-member and all-member filename-extension matching plus explicit no-extension matching without representative-name inference,
  worker-owned paged selected-root and drive facets with exact filters, and immutable selected-root,
  relative-path, and drive member context;
- exact duplicate-folder sorting, filtering, and member browsing;
- immutable review-revision preflight start and exact replay, structured ready observations,
  bounded detail paging, completed-generation reconstruction after worker restart, and assertions
  that reviewed disposable files remain present and byte-for-byte unchanged;
- schema-v10 non-mutating operation preparation and exact replay, bounded item paging, explicit
  `executorEnabled:false`, injected `non_recyclable/executor_disabled` whole-plan failure, durable
  summary counts, and unchanged disposable files; no Shell or Recycle Bin API is invoked;
- fixed-drive scanning, a path longer than 260 characters, a locked file, and a skipped junction;
- all five scan-phase timings and all five result-query timings on stderr;
- WPF startup/restoration, duplicate-file and exact-folder tabs, grid sorting, paging, filtering,
  row selection, exact-path and any/all extension/no-extension filtering, next/previous-set keyboard focus restoration, accessible selected-root and drive
  facets plus 1 GB-or-larger/minimum-copy-count/
  across-drives/review-summary/aggregate-location/set-explanation/location-span text, completed ordinary/
  long-path file and folder Explorer reveal commands, and
  deterministic result-loaded, repeated idle, startup-failure, and database-failure shutdown;
- WPF preflight plan summary, explicit non-deleting metadata/content-read confirmation, keyboard Yes
  action, terminal summary and focus movement, virtualized observation details, and unchanged
  disposable files, plus a focusable read-only operation heading and explicit disabled-executor,
  partial/ambiguous-risk disclosure with no submission action.
- Cloud locations setup accessibility, responsive registration refresh, Start scan becoming enabled
  after successful discovery, and a separate deterministic provider-unavailable launch where the
  default policy remains fail closed before and after refresh.

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

## Real Cloud Files acceptance

`Invoke-WindowsCloudPolicyAcceptance.ps1` is the separate operator gate for a registered OneDrive
or other Cloud Files root. It performs metadata-only fixture discovery, validates the root through
the real Infrastructure registration API, runs the worker against both the cloud root's broad
parent and the explicit cloud root, and compares logical size, allocated size, last-write time,
placeholder/pin attributes, and provider-process transfer counters before and after.

Run it while the provider is available and otherwise idle:

```powershell
./scripts/Invoke-WindowsCloudPolicyAcceptance.ps1 -Configuration Release
```

The script can auto-select a non-hidden locally available file and an offline placeholder, or the
operator can pass `-LocallyAvailableFile` and `-OfflinePlaceholder`. It excludes unrelated direct
children of the broad parent, creates only isolated temporary worker state, never reads fixture
content, and removes its state unless `-KeepArtifacts` is used.

Then intentionally pause or exit the provider using its supported UI and rerun without asking the
script to stop any process:

```powershell
./scripts/Invoke-WindowsCloudPolicyAcceptance.ps1 -Configuration Release -SkipBuild -ExpectProviderUnavailable
```

The unavailable mode requires all named provider processes to be absent. Restore the provider
normally after the run. Both modes must report zero discovered files for the broad and explicit
runs, unchanged file/placeholder state, and `PROVIDER_TRANSFER_COUNTERS_UNCHANGED=true`.

## Expected Result

The script prints `Windows smoke passed`. With WPF enabled it also prints that WPF automation
passed, fails if either reveal action reports a detail error, verifies that owned workers do not
survive the app, and may leave an Explorer window showing a selected disposable fixture item. By
default the fixture is removed after the app closes; `-KeepArtifacts` prints and retains its path.

If UI Automation is blocked by a locked session, elevation boundary, or headless runner, rerun with
`-SkipWpf`, then perform the WPF portion manually on an interactive Windows 11 desktop:

1. Select `Milestone 6 Smoke` and its completed run; confirm the cancelled run is also in history.
2. Open Duplicate Files, sort Group size, choose Next, filter for `group010`, select a group, and
   confirm the filtered summary and selected-root/drive detail. Use Next set and Previous set and
   confirm keyboard focus returns to the selected group row. Apply `1 GB or larger` and confirm
   the small fixture is empty, then clear it. Apply `Three or more copies`, then clear it. Filter
   one set, select `Exact path`, replace the search with a complete member path, and confirm exactly
   one set remains; then clear it. Enter `JPG` in Extension and confirm the mixed-extension set is
   isolated, then enable `All copies must match` and confirm it is excluded. Select `No extension`
   while all-copy matching remains enabled and confirm the all-extensionless set is isolated, then
   clear the filters. Choose and apply counted
   selected-root and drive facets, clear them, then choose Show in Explorer.
3. Open Duplicate Folders, sort Representative folder, filter for `original-set`, select the group,
   and reveal a folder in Explorer.
4. Close and reopen the app and confirm completed/cancelled history and completed results restore.
