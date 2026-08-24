# Windows Developer Smoke Workflow

`scripts/Invoke-WindowsSmoke.ps1` creates a disposable deterministic filesystem fixture and drives
the real worker protocol. On an interactive desktop it also launches the real WPF application and
uses stable UI Automation IDs to exercise both result surfaces, bounded Explorer reveal, and
current-page parent-grouped Explorer selection.

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
- schema-v12 one-ID external-deletion validation, working Remove invalidation, immutable member
  metadata, restart reconstruction, restored-file `present` state with sticky prior intent, and no
  cursor expansion or product mutation;
- a deterministic 1,000-event worker hint aggregate plus a real disposable non-result-file watcher
  burst;
  one 100 ms global coalescer, at most 200 distinct paths per frame, one current-run Core/WPF
  binding/automation update per frame, and schema-v13 overflow fallback without authoritative hint
  persistence;
- schema-v10 non-mutating operation preparation and exact replay, bounded item paging, explicit
  `executorEnabled:false`, injected `non_recyclable/executor_disabled` whole-plan failure, durable
  summary counts, and unchanged disposable files; no Shell or Recycle Bin API is invoked;
- fixed-drive scanning, a path longer than 260 characters, a locked file, and a skipped junction;
- all five scan-phase timings and all five result-query timings on stderr;
- WPF startup/restoration, duplicate-file and exact-folder tabs, grid sorting, paging, filtering,
  row selection, exact-path and any/all extension/no-extension filtering, next/previous-set keyboard focus restoration, accessible selected-root and drive
  facets plus 1 GB-or-larger/minimum-copy-count/
  across-drives/review-summary/aggregate-location/set-explanation/location-span text, completed ordinary/
  long-path file reveal; single-folder keyboard Explorer reveal success/actionable missing-location
  failure; and bounded three-item/two-parent grouped folder selection with Alt+G, aggregate success,
  actionable partial failure, stable selection/focus, and restored disposable fixtures, and
  deterministic result-loaded, repeated idle, startup-failure, and database-failure shutdown;
- WPF bounded visible-set validation over an externally modified reviewed copy, actionable changed
  and invalidated-decision state, immutable-history disclosure, restored copy revalidation, required
  fresh review intent, stable validation/cancellation automation, and copy-grid focus restoration;
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

## Explicit real Recycle Bin acceptance

The real Shell adapter is tested separately because it intentionally mutates only its own uniquely
named disposable fixtures. It is never part of `Invoke-WindowsSmoke.ps1` and does not enable the
WPF action. Start with the non-mutating evidence collector:

```powershell
./scripts/Invoke-WindowsRecycleBinAcceptance.ps1 -Configuration Release
```

It writes a machine-readable matrix, Markdown report, command logs, and TRX under the ignored
`artifacts/windows-recycle-bin-acceptance/` tree. Unavailable provider/physical prerequisites stay
open rather than being reported as passes. See
[`windows-recycle-bin-acceptance.md`](windows-recycle-bin-acceptance.md) for the full operator,
provider, performance, constants, Windows Undo, TOCTOU/recovery, and physical-accessibility
procedures.

Run the explicit mutation slice only on an interactive Windows 11 desktop with an available local
Recycle Bin:

```powershell
./scripts/Invoke-WindowsRecycleBinAcceptance.ps1 -Configuration Release -ConfirmRecycleBinMutation
```

The acceptance proves one dedicated STA owns COM; successful local-root capability requires a
non-opening ordinary-item classification plus successful `SHQueryRecycleBinW`; durable-start
acknowledgement occurs before `PerformOperations`; positive `PostDeleteItem` recycled-item,
`FinishOperations`, outer HRESULT, and abort evidence are retained; cancellation after durable
start stops at `PreDeleteItem`; and an independent hard-link alias and exact-folder copy survive
byte-identically. The success fixtures remain recoverable in the current user's Recycle Bin. The
test deliberately implements no permanent cleanup.

On the 2026-08-20 development host, success returned `PerformOperations=0`,
`FinishOperations=0`, positive recycled items, and `GetAnyOperationsAborted=false`. Returning the
cancellation HRESULT from `PreDeleteItem` kept the source unchanged and produced the expected
per-item cancellation while the aggregate abort flag also remained false. A source opened without
delete sharing produced Windows copy-engine HRESULT `0x80270027`, mapped to `sharing_violation`,
and remained byte-identical. This is host evidence, not a provider-wide contract.

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

The script prints `Windows smoke passed`. It sends one deterministic bounded worker hint frame with
an aggregate count of 1,000, and the real WPF pass rapidly rewrites one disposable non-result file
under the selected root, observes one coalesced/bounded live-hint status, removes that file, and
leaves the scan fixtures unchanged. It also injects one watcher overflow without changing a fixture,
proves the dirty root survives a worker restart, and requires every hint/overflow/list response to
retain `executorEnabled:false`. With WPF enabled it proves the durable root appears as a visible
dirty/reconciliation-required warning, invokes one explicit at-most-200-item batch, keeps a partial
root visibly dirty or clears only the final batch, restores copy-grid focus, and leaves immutable
scan history and fixtures unchanged. WPF automation also fails if an admitted reveal or
grouped selection lacks terminal aggregate success or actionable failure state, verifies that owned
workers do not survive the app, and may leave
Explorer windows showing selected disposable fixture items grouped by parent. By
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
4. On one disposable visible copy, rapidly change only its last-write timestamp and restore the
   exact original timestamp. Confirm one polite status says the filesystem events were coalesced
   into bounded path hints and the matching visible row is validation pending. Do not expect a hint
   to become durable truth; choose **Validate page** for an authoritative observation.
5. Before changing a file, confirm the injected watcher overflow appears as a dirty/
   reconciliation-required root warning. Choose **Reconcile next batch** (Alt+X), confirm the action
   reports at most 200 checked copies, and confirm focus returns to the current copy grid. If more
   work remains, the warning must remain; otherwise the final status must explicitly say the dirty
   marker was cleared. No file should move or disappear.
6. On a disposable reviewed copy, change its length outside the app, choose **Validate page**, and
   confirm the row reports Changed and its prior decision is invalidated without deleting the file.
   Restore the exact bytes and timestamp, validate again, confirm Present retains the prior-intent
   warning, then record a fresh decision. Do not use a provider placeholder for this manual step.
7. Close and reopen the app and confirm completed/cancelled history, completed results, and the
   latest live-validation overlay restore.
