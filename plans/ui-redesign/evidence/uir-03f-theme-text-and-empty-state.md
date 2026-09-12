# UIR-03f — native themes, accessibility text size and empty folders

2026-09-11. Implementation commit: `5f83705`. Scope: reproduced A09/A11 presentation defects and the reported A05 empty-state
overlap. UIR-03 is now [complete within its shell scope](uir-03-shell-acceptance.md). This does not accept the full later screen/native matrix.

## Operator report and causes

The operator supplied three screenshots from the fictional fixture: unreadable dark-mode text
and classic gray controls; Desert contrast with "No exact duplicate folders" drawn across table
headers; and enlarged Windows title text with unchanged application text. The requested text
setting was 150%, but the report did not independently confirm its numeric value. Previous
display-scale context was 100%. Desert is observed, not a full contrast-theme pass.

Custom implicit styles replaced the Fluent styles without inheriting their templates. Explicit
classic SystemColors foregrounds also disagreed with the Fluent dark background. Typography was
fixed at 12/14/16/20/24 DIPs. The folder message shared a shrinking star-sized table row with
visible column headers.

## Implementation

- Shared controls and local item styles inherit the native Fluent default styles. Semantic,
  dynamically resolved Fluent brushes replace classic text/background/border overrides; native
  selection foregrounds remain inherited, and paired system highlight brushes remain paired.
- Windows UISettings supplies the current accessibility text factor and subsequent change events.
  Infrastructure owns the native subscription; the WPF adapter updates window typography resources
  on its dispatcher and unsubscribes when the window closes. Monitor DPI remains independent.
- The persistent selected/active summaries use ellipsis with full tooltips to preserve space for
  the workspace when text grows. Headings, fields, buttons, lists, disclosures and table text scale.
- Folders uses a scrolling page with bounded native result/card viewports. Empty/unavailable states
  suppress table headers and measure their own content. This interim stacked layout replaces the
  folder splitter; final adjustable comparison layout remains UIR-05, as with interim Files.
- File decision/path buttons wrap within their existing columns when text grows; the warning action column sizes to its label.

The worker protocol, scan/hash/cache behavior, result ceilings and production-deletion boundary
are unchanged. Fixture services remain fictional; no production worker or database is opened.

## Verification and limitations

Final verification passed:

| Check | Result |
|---|---|
| Focused theme/text/viewport matrix (`theme-text-wrap-verified.trx`) | Pass |
| Paired Rust worker builds, Debug/Release | Pass; `worker-debug.log`, `worker-release.log` |
| Windows Debug/Release builds | Pass, zero warnings/errors; `windows-*-complete-build.log` |
| Windows Debug/Release integration | Each: 170 Core, 75 Infrastructure with five operator-only skips, three WPF test methods; `windows-*-complete-test.log` and `*-complete-integration*.trx` |
| Standalone fixture build and `--verify` | Pass, exit 0; `fixture-build.log`, `fixture-launch.txt` |

All original shell-route/context/reordered-tab assertions remain: the former standalone shell test
now runs inside the existing themed App/STA smoke method, since native Fluent resource inheritance
requires that theme host. WPF test-method count is therefore three instead of four; no assertions
were removed. The shared STA timeout is 60 seconds for the added theme/text/viewport matrix.
No Rust implementation changed; UIR-03c's 226-passed/10-ignored Rust tests are retained, not rerun.
Outputs, captures, failed checks and build logs are isolated under `artifacts/uir03f`.
Regression coverage includes theme switching on an existing window, resolved normal-text
contrast, retained Fluent templates, 100%/150% text resources delivered from a background event,
focus/selection retention, actual native scroll-viewport bounds, table action reachability,
Progress/Summary scrollbar clearance, and empty-folder message/header separation at both sizes
with an 80-DIP fictional toolbar allowance. The existing navigation and delayed-pane checks remain.
Image review confirmed light text on dark controls, visibly enlarged app text, distinct empty-folder
copy and wrapped table actions. The stronger bounds check explicitly intersects native
ScrollContentPresenter viewports. Action clipping found during enlargement was corrected by wrapping.
Failed iterations are retained, including resource-identity/collapsed-visual-tree test corrections,
the prior 15-second harness timeout, action-margin accounting, ineffective auto-sized file-action
columns, and the theme-less standalone shell host. The final matrix uses the wrapping fix and shared
STA. These failures are not silently replaced by the final passed logs.

Commands used isolated `--artifacts-path artifacts/uir03f/dotnet` and explicit
`WorkerExecutableSource=artifacts/uir03f/target/{debug,release}/super-duper-worker.exe`; solution tests
used `--no-build --no-restore` only after paired builds. Child TEMP/TMP were isolated, and physical
Recycle Bin/provider/cloud opt-in variables were cleared. No physical scan/provider campaign ran.

After verification, old fictional PID 71248 was checked against its exact UIR-03e executable path
and closed normally. Corrected fixture **PID 67748/session 1** is open from
`artifacts/uir03f/fixture/bin/SuperDuper.Windows.RedesignFixture/debug_win-x64/`.
The new host uses actual Windows UISettings; `--verify` also exercises native construction/shutdown.
Re-audit before reuse or launch. Production runtime/state was untouched.

## 2026-09-12 operator recheck passed

On corrected implementation `5f83705`, the operator reported **"all 3 pass"** in response to:

1. Dark: readable text and controls at both fixture sizes.
2. Desert, Results/Folders: empty message no longer overlaps table headers.
3. Windows text size 100% -> 150% -> 100% with the window open: app text changes and buttons
   remain reachable, at both fixture sizes and with display scaling held at 100%.

Record these three scoped physical rechecks as passed and close the reported Dark, title-only
text enlargement and empty-folder overlap defects. Restoration of all original OS settings was
requested but not separately reported. Retain nine earlier scoped passes, Narrator and the accepted
scrollbar correction. No wider screen/contrast combination, physical display-DPI or later gate is
accepted by this report. NVDA remains unrun/unavailable because it is not installed.

The operator confirmed three monitors initially at 100%, then specified transitions at **150% and
175%** with no noticeable clipping. These scoped physical transition/layout checks pass. Windows
did not offer 200%; record `unrun_unavailable`, not passed or waived. Only one monitor accepts scale
changes; the operator excludes Windows troubleshooting. Do not force custom scaling. See
[the scale confirmation](uir-03-desktop-walkthrough.md#2026-09-12-display-scales-confirmed-150-and-175).
The operator subsequently confirms "focus remained": record keyboard-focus retention as passed.
The operator then confirms the same file group stayed selected. Record selection retention as
passed. [The scoped shell assessment](uir-03-shell-acceptance.md) closes UIR-03; UIR-04a setup/Scan
again is ready, not started. NVDA and 200% remain unavailable/unrun requirements for UIR-08/A11,
not passes or waivers. These updates change documentation only; no code/build/test rerun, fixture
reset/launch, OS-setting or production action. Retain prior verification and re-audit the last
recorded runtime before reuse. No full native accessibility/DPI or release acceptance is claimed.

## References

The implementation uses the supported WPF Fluent resources and Windows text setting:
[WPF Fluent usage](https://github.com/dotnet/wpf/blob/main/Documentation/docs/using-fluent.md),
[native control styles](https://github.com/dotnet/wpf/tree/main/src/Microsoft.DotNet.Wpf/src/Themes/PresentationFramework.Fluent/Styles),
[TextScaleFactor](https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings.textscalefactor),
[TextScaleFactorChanged](https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings.textscalefactorchanged).
