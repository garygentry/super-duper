# UIR-04a saved setup and Scan again

2026-09-12. Baseline: scoped shell acceptance `84a4f55`, implementation `5f83705`.
UIR-04a is implemented and locally verified for A06/A14 and the setup portion of A17.
UIR-04 remains in progress; this is not full A17, native/user, or release acceptance.

## User journey and boundaries

- Scan again opens the selected saved scan's current setup without starting a run or replacing
  the opened historical run. Existing drafts survive reopening setup. The named action distinguishes
  the selected saved scan from a different historical workspace context.
- Setup shows folders/drives, location validation messages, exclusion counts and registered cloud
  exclusions. A single folder/drive picker and Enter path remain available. Saving normalizes
  overlapping locations; exact stored paths are preserved. New definitions receive unique readable
  suggested names. Advanced contains ignore patterns, manual exclusions and the content-read policy.
- Reuse verified hashes explains fresh discovery, qualifying identity/change metadata, persistent
  storage across restarts and conservative reads. Re-read candidate content maps to the existing
  `revalidate_content` policy and explicitly preserves normal candidate filtering. Loading another
  saved definition restores the reuse default; Save retains the selected next-run override.
- Start saves valid edits, snapshots the chosen policy through the existing worker request and opens
  the new dated run. Setup editing is locked through pending admission and restored on rejection or
  cancelled navigation. One-active-run enforcement and cloud detection remain fail closed.
- Leaving dirty setup or selecting another definition now requires Save / Discard / Stay. The prior
  implementation silently lost edits on definition changes. The focused choice disables the underlying
  workspace, cycles keyboard navigation and initially focuses Stay. Invalid Save retains the prompt
  and its error. Actual main-area and Scan-subtab bindings restore the retained destination.
- Production deletion remains disabled. No engine, protocol, cache implementation, query ceiling,
  survivor, overlap or revision contract changed. No production state or consumed campaign was used.

## Verification

All new outputs are under `artifacts/uir04a`. TEMP/TMP were isolated there for test children.
The sandbox could not read the user's NuGet configuration; the same isolated commands ran successfully
with approved access. No machine configuration or Windows accessibility settings were changed.

| Check | Result / retained output |
|---|---|
| Initial focused Core run | 33 passed, three old-copy/name assertions failed; retained `results/setup-initial.trx`. Updated those assertions to the specified labels and added behavior coverage |
| Setup/Shell behavior | 42 passed, `results/setup-behavior.trx` and `results/setup-final.trx`; one further deletion/draft regression passes in the final full Core run |
| Core regression after implementation review | 176 passed in `results/core-review.trx`; final 177 passed in `results/core-final.trx` after the definition-deletion guard |
| Loaded-STA WPF | Three methods passed; existing shared shell assertions plus new SetupWorkflowFixture. `results/wpf-initial.trx`, `wpf-review.trx`, `wpf-final.trx` |
| Small real-worker repeat/restart | One passed, `results/repeat-initial.trx`, four completed runs with retained history/cache |
| Fictional desktop fixture | Built in `artifacts/uir04a/fixture`; not substituted for the running operator fixture |
| Review | Changed code, test assertions, screenshots, document links and `git diff --check` reviewed |

Core tests cover valid edited Start, unchanged historical run identity/parameters, policy snapshots,
reopening old results while another run is active, dirty choices, invalid exclusions, pending-start
failure, stale-start navigation and confirmed definition deletion without a stale-draft prompt. Existing unreachable-root and failed-detection tests remain.
WPF verification covers actual Scan again focus, collapsed/expanded setup, reachable advanced controls
and Start at 900x600/1180x760, dirty prompt focus and actual two-way tab bindings. Captures under
`captures-final` were inspected for setup and dirty choice; all earlier physical UIR-03 passes remain
retained. The shared loaded test also rechecks existing geometry because the header now has Scan again.

The real-worker fixture uses two nonpersonal 256-KiB files, a separate result database and a stable
cache path. It completes a baseline scan and saves a keep decision, disposes the worker, then reopens
the same history/cache. The unchanged repeat records both partial and full cache hits. Removing one
fixture copy, adding another and editing exclusions changes only the new run's groups; old group/member
IDs and the keep decision remain. Removing the exclusion and using revalidation produces a fourth
dated run with the requested policy and zero full-cache hits. Cleanup touches only its generated
temporary fixture. The unchanged Debug worker was copied from retained UIR-03f output to
`artifacts/uir04a/target/debug`; no Rust rebuild or test rerun is claimed.

Representative commands (PowerShell, with TEMP/TMP under `artifacts/uir04a/temp`):

```powershell
dotnet test apps/windows/tests/SuperDuper.Windows.Core.Tests/SuperDuper.Windows.Core.Tests.csproj --artifacts-path artifacts/uir04a/dotnet --filter 'FullyQualifiedName~SessionSetupViewModelTests|FullyQualifiedName~ShellSessionWorkflowTests'
dotnet test apps/windows/tests/SuperDuper.Windows.Infrastructure.Tests/SuperDuper.Windows.Infrastructure.Tests.csproj --artifacts-path artifacts/uir04a/dotnet --filter 'FullyQualifiedName~SavedScanRepeatTests'
dotnet test apps/windows/tests/SuperDuper.Windows.Smoke.Tests/SuperDuper.Windows.Smoke.Tests.csproj --artifacts-path artifacts/uir04a/dotnet
dotnet build apps/windows/tools/SuperDuper.Windows.RedesignFixture/SuperDuper.Windows.RedesignFixture.csproj --artifacts-path artifacts/uir04a/fixture
```

## Remaining evidence and exact next slice

UIR-04b: multi-day elapsed time, update receipt freshness and terminal activity presentation
(A08/A16), using controlled clock/progress/lifecycle tests and the existing detailed counters.
Full A17 combined coverage remains UIR-07/08: exact membership comparison under identical policies,
same-size/preserved-mtime content changes, missing roots, conservative fallback and interrupted reuse.
No full-drive, provider or performance campaign is authorized. Debug-only scoped checks here retain
the prior paired Debug/Release baseline; the full integration matrix remains UIR-08.

Operator setup/Scan again usability acceptance remains pending in the redesigned-workflow walkthrough.
NVDA and physical 200% remain `unrun_unavailable` for UIR-08/A11; do not troubleshoot Windows.
Startup re-audit found PID 67748/session 1 responsive at the exact UIR-03f fictional executable path.
It was left untouched. Do not overwrite its output or assume it contains this slice.
