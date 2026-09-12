# Prepared A01/A02/A09 desktop walkthrough

Status: **nine scoped checks, scrollbar correction and Narrator walkthrough passed; Dark, Desert/empty-folder and live text-size rechecks passed; display-DPI pending**. UIR-03d's local viewport prerequisite
is verified in [the viewport evidence](uir-03d-viewport-access.md). This is a fictional shell walkthrough, not a
real scan, deletion campaign, release-smoke substitute or physical acceptance result.

From the repository on `codex/ui-redesign`:

```powershell
./scripts/Invoke-UiRedesignFixture.ps1       # build only
./scripts/Invoke-UiRedesignFixture.ps1 -Show # explicitly open the interactive fixture
```

The title starts `FICTIONAL FIXTURE`. Output is `artifacts/uir03-desktop-fixture`. This independent
project constructs a plain WPF Application with System theme and shared resources, never shipping
App.OnStartup. All external services are in-memory fakes; it cannot start a real worker, read a
production database, open Explorer or delete files. Close its window normally. Leave the operator's
other Super Duper window and worker untouched. The fixture toolbar consumes extra vertical space;
record that when comparing it with the automation captures, which have no toolbar.

UIR-03d supplies an interim scrolling Files/History layout. Scroll the page vertically to move
between sections, and use each bounded grid's native scrollbars for its rows/technical columns.
The former Files split adjustment is deferred to the full UIR-05 list/detail redesign. Complete
buttons must remain reachable; horizontal scrolling here does not pass A03's final layout criterion.
The automated minimum-size test also reserves 80 DIPs for toolbar overhead, but physical input,
themes, text enlargement and DPI still require the observations below.

| Step | Reviewer action and expected observation | Record |
|---|---|---|
| A01 context | In initial Files, identify completed run 1/date/state/two roots, then identify active run 2 independently. Full long names remain available by tooltip/automation. | Interpretation, clipping, assistance |
| A01 selection | In History highlight run 2, then return to Files without Open scan: run 1 remains the workspace. Open run 2 explicitly: its summary opens; View progress still owns the active run. Open run 1 again. | Named run before/after each action |
| A02 optional load | From a fresh fixture, choose toolbar Delay folders. Navigate to Files or Scan while it waits. Choose Fail pending folders. Files/context remain usable and free of that pane's error; Folders reports its own failure. | Input responsiveness, error scope |
| Warning return | View progress, Review warnings, Close: focus returns to History. View progress returns to the active Progress tab. On completed run 1, its warning action opens immutable Files and focuses the group grid. | Actual focus after every action |
| Retention | Scroll the file group grid and choose a group; visit Review and return. Repeat with Progress's vertical scroll. Both retain same-run position; no new run is opened by this navigation. | Selected group and offsets/visible rows |
| A09 shared UI | Inspect Setup, Progress, Files, Folders, Review, History and Performance/recovery states. Compare heading/body/field/button/row sizes, disabled reasons and focus outlines. Expand filters and totals; exact field semantics remain available. | Per-screen differences and unreachable controls |
| Layout | At 1180x760 and 900x600 repeat context, paging, warnings and return. Use long fixture names; expand both Files disclosures. Scroll to group/copy rows, complete decision/path actions, warning actions and Close. Report any unreachable control or clipped button as a defect. | Requested/client size, page/grid scroll, screenshots, defects |
| Keyboard | Repeat navigation, filters, paging and warning return with physical Tab/Shift+Tab, native arrows, Enter/Space and existing access keys. Selection alone must not mark removal. No Delete execution shortcut exists. | Key sequence and actual focused control |
| Accessibility | Repeat with Narrator and NVDA, light/dark/high contrast, Windows text enlargement and authorized physical 100/150/200% monitor moves. Confirm announcements, selected-row text and focus remain perceivable. | Reader/version, OS/theme/scaling/monitor, pass/fail/unrun |

For each step record observations and unexpected scope changes, not merely clicks or test success.
Automation currently establishes programmatic focus only. The fake data do not establish worker
performance, screen-reader cadence over a real long scan, representative result scale or A17 rescan
acceptance. Those requirements retain their later gates and separately isolated procedures.

## 2026-09-12 display scales confirmed: 150% and 175%

The operator clarifies **"150 and 175. Windows didn't offer 200"**. Associate the preceding
successful scaled/unscaled movement and no-noticeable-clipping report with both 150% and 175%.
Record those scoped physical transition/layout observations as passed; 175% is additional coverage.
The requested 200% case is `unrun_unavailable`, not passed or waived. Only one monitor accepts
scaling changes, and the operator excludes Windows troubleshooting. Do not force custom scaling,
change drivers/settings or request another monitor merely to fill this row.

The remaining available observation is whether file selection and keyboard focus survived monitor
moves; that was requested but not explicitly reported. Confirm it, then assess UIR-03 with 200%
and NVDA accurately retained as unavailable/unrun. Do not silently equate 175% with 200% or claim
the full physical matrix passed. No new app defect is reported. This update is documentation only;
no code/build/test/runtime or OS-setting action. Retain all earlier scoped acceptance.

## 2026-09-12 cross-monitor adjustment observed; scale unspecified

The operator reports that Windows allows scaling only one of the three monitors and explicitly
excludes troubleshooting that Windows behavior for now. Moving the fictional fixture between the
scaled and unscaled monitors **"seems to adjust fine"**, with **"Not noticable clipping"**.
Record this as a positive physical transition/layout observation for the tested configuration.
The actual scaled percentage, whether both 150% and 200% were exercised, and keyboard-focus/
selection retention are not explicitly reported. Do not mark the full display-DPI matrix passed.

Next: confirm the tested percentage(s). If one requested scale remains, it can be tested separately
on the same monitor that accepts scaling; configuring all three monitors differently is unnecessary.
Do not troubleshoot Windows or infer a fixture defect from the OS limitation. Retain earlier
accepted Dark/Desert/text-size and other scoped checks. UIR-03 stays in progress, NVDA unavailable/
unrun. This evidence update performs no code/build/test/runtime or OS-setting actions.

## 2026-09-12 monitor setup confirmed; display-DPI check prepared

The operator reports **"3 monitors, 100% scaling now"**. Monitor resolutions and the physical
DPI transition results are not yet reported. Read-only process verification finds fictional
PID 67748/session 1 responsive at its exact UIR-03f executable path; it was left untouched.

Requested next walkthrough, with ordinary theme and Windows Accessibility text size at 100%:
leave one monitor at 100% display scaling, set another to 150% and the third to 200% where offered.
Move the open fictional fixture across 100% -> 150% -> 200% -> 100%. On each monitor inspect Files
rows/actions, Progress/Summary and warnings for readable text, clipping and reachable controls;
exercise keyboard navigation and verify selected group/focus survive the move. Return all displays
to their original 100% setting. If the window cannot fit, record monitor resolution and scale;
do not infer application acceptance from a desktop too small for its minimum window size.

This is preparation, not a pass. Record observations before assessing UIR-03. NVDA stays
unrun/unavailable. No build/test, app reset/launch, OS-setting mutation or production action ran.

## 2026-09-12 all three corrected visual checks passed

The operator reports **"all 3 pass"** for the requested UIR-03f checks at both fixture sizes:
Dark readability; Desert Results/Folders without empty/header overlap; and live Windows text
size 100% -> 150% -> 100% with app text changing and buttons reachable, display scale held at 100%.
Close those three reported defects. See [the scoped acceptance record](uir-03f-theme-text-and-empty-state.md#2026-09-12-operator-recheck-passed).
Keep the nine earlier passes, Narrator and accepted scrollbar correction. NVDA stays unavailable/
unrun. Next is monitor count/current display-scale information and the physical 100%/150%/200%
display-DPI/cross-monitor check. UIR-03 stays in progress; no later gate or release acceptance.
This confirmation changes documentation only; earlier automated tests and fixture launch are retained.

## 2026-09-11 theme, text enlargement and empty-folder defects

The operator supplied screenshots and reported: **"Dark mode completely off"**; the second
screenshot uses **Desert**; and **"I don't observe any changes with text size except for Window
title"**. The same report identifies **"No exact duplicate folders"** overlapping the table header.
Record failures for Dark and text enlargement, and an A05 empty-state overlap. Desert is observed,
not a general contrast-theme pass. The requested text setting was 150%, but its numeric value was
not independently restated. Earlier display-scale context was 100%.

[UIR-03f evidence](uir-03f-theme-text-and-empty-state.md) records the native Fluent/text-size and
layout fixes, isolated tests and corrected fixture runtime. Recheck Dark, Desert and live Windows
Accessibility text changes at both fixture sizes, including Files actions, Progress/Summary,
warnings and empty Folders; restore original settings. UIR-03 stays in progress. Nine unrelated
scoped passes, the accepted scrollbar correction and Narrator evidence remain retained. NVDA is
unrun/unavailable. Physical theme/text/contrast and monitor-DPI acceptance remain open.

## 2026-09-11 NVDA unavailable; visual accessibility next

The operator confirmed NVDA is **"not installed"**. Record its check as `unrun_unavailable`, not
passed or waived. No installation was requested or performed. This does not invalidate the
accepted Narrator fixture walkthrough or prevent independent visual accessibility checks.

Next requested batch: Light and Dark Windows color modes; one named Windows contrast theme;
then 150% Windows Accessibility Text size with display Scale kept at 100%, restoring original
settings afterward. In each state inspect Files rows/actions, Progress/Summary and warnings at
both fixture sizes for readable text/selection/focus and complete, unclipped actions. Record actual
theme names, percentages and failures, including if fixture text does not enlarge. Results remain
pending. These are fictional redesign checks, not the parked release high-contrast gate. Physical
multi-monitor 100/150/200% display-DPI evidence is still a separate remaining check.
UIR-03 remains in_progress; NVDA evidence remains outstanding. This update is documentation only.

## 2026-09-11 Narrator walkthrough passed

From `d1c440a`, the operator reported **"Checks pass"** for the requested Narrator keyboard
walkthrough: Files group/copy rows, View progress and Review warnings → Close, checking meaningful
labels/column names, selection and scan-status announcements, correct focus return and absence of
excessive repeated speech. Record this scoped Narrator walkthrough as passed on the corrected
`ae8d7d1` fictional fixture. Known environment remains standard Windows theme, 100% display scaling.
Read-only installed `System32/Narrator.exe` metadata reports file/product version `10.0.22621.5262`;
this is installed-executable metadata, not a separate operator-supplied version or OS build claim.

NVDA availability was not answered; do not infer its pass from this Narrator report. Next establish
NVDA availability and repeat the same reader journey if available, then collect remaining light/
dark/high-contrast, Windows text enlargement and physical monitor-DPI evidence. No real long-scan
announcement cadence or parked-release accessibility acceptance is claimed from this fake fixture.
UIR-03 remains in_progress. This update changes evidence/checkpoints only; no code/build/test or
fixture actions. Prior automated checks remain retained.

## 2026-09-11 remaining styling passed

After the scrollbar correction, the operator reported **"other styling looks good"** in response
to the interrupted shared-style/expanded-controls check at both fixture sizes. Record that scoped
check as passed, bringing the total to nine, plus the separately verified scrollbar correction.
The known environment is the standard Windows theme at 100% display scaling; no additional
theme/contrast/text enlargement or physical monitor-DPI combinations are inferred.
Next: Narrator, then NVDA, followed by remaining light/dark/high-contrast, text enlargement and
physical monitor-DPI observations. The operator has been asked to try Narrator on Files rows,
active Progress and warning Close/return, and to state NVDA availability. Results are pending.
UIR-03 remains in_progress; UIR-04 stays dependent. This update records operator evidence only;
no product/test changes, build/test reruns or fixture actions. Prior results remain retained.

## 2026-09-11 scrollbar correction verified

On the corrected `ae8d7d1` fixture, the operator reported **"Scroll overlap looks good now"**.
Close the reported Progress/Summary scrollbar defect as operator-verified. The earlier report
established both fixture sizes, standard Windows theme and 100% display scaling as the original
defect environment. This correction report does not complete the broader expanded-controls/shared-
style check or reader/theme/contrast/text enlargement/multi-monitor DPI checks. Those remain next;
the eight earlier scoped passes stand. No code, build/test or runtime actions in this confirmation
update; documentation/diff checks only. UIR-03 remains in progress and UIR-04 dependent.

## 2026-09-11 keyboard/result passes and scrollbar defect

From `8d2354c`, the operator reported **"1. Pass, 2. Pass"** for completed-run warning
**Open duplicate results** opening Files with group-grid focus and the explicitly requested
keyboard-only filters/paging/warning journey with visible focus and harmless copy selection.
Eight scoped checks have now passed. This supplies the keyboard evidence missing in the previous
batch; screen readers, themes/contrast, text enlargement and monitor DPI remain unrun.

For the third check, the operator reported **"On Scan/Progress, Scan/Summary tabs, main body
outer-most scroll bar overlaps right side of body content area"**. Record an A09 layout defect;
do not mark the expanded-control/shared-style check passed. The fix, before/after geometry
regression, integration results and runtime are in [UIR-03e evidence](uir-03e-scan-scrollbar-clearance.md).
Next is operator verification of that fix and completion of the interrupted third check, then
remaining physical accessibility observations. The operator clarified that the original overlap
occurred at both fixture sizes, with the standard Windows theme and 100% display scaling. Separate
Windows text-size and monitor details remain unspecified. This is original-defect context, not
confirmation of the corrected build. UIR-03 remains in progress; UIR-04 remains dependent. Earlier entries below
preserve the evidence available at each batch.

## 2026-09-11 navigation follow-up passed

From checkpoint `012573f`, the operator reported **"All 3 pass"** for the next numbered batch:

| Check presented | Operator result | Scope of evidence |
|---|---|---|
| Open run 2 from History, confirm View progress shows active run 2, then reopen run 1 | Pass | Explicit run opening and active progress ownership |
| View progress → Review warnings → Close returns focus to History | Pass | Physical warning Close/return focus reported correct |
| Scroll Progress, visit Files, return | Pass | Same-run Progress scroll position retained |

Together with the initial batch below, six scoped checks passed. No defect or missing focus outline
was reported. Keyboard use was requested where possible, but the report does not specify the input
method or key sequences; the full keyboard-only journey remains unverified. No environment/theme/
text-scale/DPI or screen-reader evidence was supplied. Completed-run warning **Open duplicate
results** focus, full expanded controls/shared styles/recovery, and keyboard/accessibility remain
pending. Those are the next observations; do not repeat the six passes without a reopen reason.
UIR-03 remains `in_progress`, with UIR-04 dependent on acceptance. This follow-up changes evidence
and checkpoints only; no product/test changes, build/test reruns or fixture/runtime actions.
Verification: documentation link targets, reviewed diff and `git diff --check`.

## 2026-09-11 initial operator checks

Implementation under review remains `b20341a`; the session started clean at `deeedb4` on
`codex/ui-redesign`. Read-only process checks found no matching app/worker/fixture. The existing
fictional executable was opened as PID 70232/session 1 without rebuilding. The operator requested
a reset after taking time to get oriented; another process check found no matching process, and
the existing executable was reopened fresh as PID 58144/session 1. No running process was stopped.
The fixture uses the isolated `artifacts/uir03-desktop-fixture` output and in-memory services.

After that reset, the operator reported **"All 3 checks pass"**, referring to the three numbered
checks presented in this task:

| Check presented | Operator result | Scope of evidence |
|---|---|---|
| Scan context: distinguish completed run 1 in Files from active run 2; highlight run 2 in History and return without Open scan | Pass | Reviewed versus active context and harmless History highlighting |
| Responsiveness: Delay folders, navigate to Files, Fail pending folders | Pass | Files remains usable and the failure belongs to Folders |
| Retention/layout: select and scroll a file group, visit Review and return; use both size buttons and report clipping/unreachable controls | Pass | File selection/scroll retained; no clipping or unreachable controls reported in this scoped check at 1180x760 and 900x600 |

The report establishes fixture visibility and these specific observations. Orientation assistance
included the initial instructions and a requested reset; no orientation-related defect was reported.
No timings, exact scroll offsets, screenshot matrix, OS/theme/text-scale or monitor-DPI measurements
were supplied. This report does not cover the full table above: explicit Open scan round trips,
warning-return/result focus, Progress scroll retention, the full expanded-disclosure/action matrix,
shared styles/recovery, physical keyboard and accessibility observations remain pending/unrun.
The next requested batch is explicit Open scan, active warning Close/return focus and Progress scroll
retention, using physical keys where possible and reporting actual focus.

UIR-03 remains `in_progress`; UIR-04 still depends on its acceptance. No product defect was reported
by these three checks, and no product/test code changed. Prior build/test results are retained,
not rerun. This evidence update is checked by documentation link validation and `git diff --check`.
Re-audit fixture processes before reuse or launch; old PIDs are historical observations only.
The sections below preserve the earlier pending state as dated history.

## 2026-09-08 operator session preparation

Implementation under review: `b20341a` (UIR-03d). Git started clean on `codex/ui-redesign`;
`wpf-poc` remained `deefa40ebe607b785b395a29a6282e8b417a9b14`. Latest implementation diff and the
selected walkthrough/fixture were reviewed. No product or test code changed in this preparation.

The default-sandbox build failed with MSB4184 because installed Windows SDK metadata under
`C:\Users\gary\AppData\Local\Microsoft SDKs` was inaccessible. SDK-enabled execution was approved;
`./scripts/Invoke-UiRedesignFixture.ps1` then passed with zero warnings/errors. The authorized
`./scripts/Invoke-UiRedesignFixture.ps1 -Show` also built with zero warnings/errors and launched
fixture PID 17052 in interactive session 1. Process presence is launch evidence, not proof that
the operator saw or tested the window. Operator visibility has not yet been confirmed.

Logs: ignored `artifacts/uir03-operator/fixture-build.log` and `fixture-show.log`; child TEMP/TMP:
`artifacts/uir03-operator/temp`. Build output remains `artifacts/uir03-desktop-fixture`, separate
from the operator's app. The initial CIM process query was denied; read-only `Get-Process`
established executable paths instead. Operator WPF PID 36316 and worker PID 17612 remained at
`artifacts/windows-x64`, untouched. The fixture is left available for the requested walkthrough;
re-audit processes before reuse or launch, and close only the fictional window normally.

The operator was asked in this task for context, delayed-pane responsiveness/error scope, actual
focus, same-run selection/page/grid/progress scroll, and reachable rows/actions/shared styles at
both toolbar sizes. Record the exact action, observed result and size for defects. No response or
operator observation is recorded yet; pending rows below are not passes or failures.

| Observation | Current evidence / disposition |
|---|---|
| Fixture visible to operator; environment, theme, text scale, DPI | Awaiting operator report |
| A01 selected run 1 versus active run 2; History highlight versus Open scan | Awaiting operator report |
| A02 Delay folders; navigation while pending; Fail pending folders error scope | Awaiting operator report |
| Warning Close/result navigation and actual focused control | Awaiting operator report |
| Same-run selected group, visible rows, page/grid and Progress scroll | Awaiting operator report |
| A09 shared styles, disabled reasons, rows/actions at 1180x760 and 900x600 | Awaiting operator report |
| Physical keyboard, Narrator/NVDA, theme/high contrast, text enlargement, physical DPI | Unrun; no acceptance inferred |

Verification in this session is fixture build/launch and documentation checks only: final diff
review, `git diff --check`, and 29 local documentation links passed. The prior
UIR-03d Debug/Release integration (170 Core, 75 Infrastructure with five operator-only skips,
four WPF methods each, paired worker builds) and UIR-03c Rust 226-passed/10-ignored baseline
are retained, not rerun. There is no newly evidenced product defect to fix yet. **UIR-03 remains
in_progress**; collect the requested observations, fix any evidenced defect with focused tests
and required integration, then assess desktop acceptance. UIR-04 remains dependent; full S03 stays
UIR-05. A08/A16/A17, disabled deletion, worker ownership, collection bounds and correctness
protections remain unchanged. No physical scan or consumed campaign ran.

## 2026-09-08 continuation audit

Latest recheck: 20:44 EDT, starting clean at `2239369` on `codex/ui-redesign`.
`wpf-poc` remains `deefa40`. Read-only `Get-Process` again found fixture 17052/session 1
and operator app/worker 36316/17612 at the paths recorded below; all remain untouched.
CIM access was denied and sandbox window handles/titles were again unavailable. Visibility and
the A01/A02/A09 observations were requested in the task; no operator report is available yet.
No new defect or acceptance result is established. No build, test or launch was repeated.
The same missing evidence now blocks consecutive continuations: resume substantive work when
the operator supplies observations or reports a visibility problem needing fixture assistance.
Another audit or documentation-only continuation cannot satisfy the walkthrough.

At 19:44 EDT, the continuation started from clean `86127a5` on `codex/ui-redesign`;
`wpf-poc` still resolved to `deefa40ebe607b785b395a29a6282e8b417a9b14`.
Command-scoped `safe.directory` resolved Git's sandbox ownership check without changing global
configuration. `Get-CimInstance Win32_Process` returned access denied; read-only `Get-Process`
confirmed fixture PID 17052 in session 1 at the expected `artifacts/uir03-desktop-fixture`
executable, and operator app/worker PIDs 36316/17612 at `artifacts/windows-x64`.
The process API returned zero window handles and empty window titles in this sandbox; that does
not establish either visibility or invisibility to the operator. All three processes were left
untouched. No duplicate fixture, build, test, scan or runtime-state access was started.

The task requested visibility confirmation and actual A01/A02/A09 observations using the prepared
walkthrough above. No operator response has been received in this continuation; all pending/unrun
rows remain unchanged. No product defect is evidenced. UIR-03 remains **in_progress**, with
desktop acceptance pending and UIR-04 still dependent. The exact next action is to collect the
operator report (including visibility), record observed behavior, fix reproducible defects using
focused tests and required integration, and assess the shell acceptance criteria. Do not repeat
this preparation or the verified viewport prerequisite merely to fill the missing operator evidence.

This continuation changes documentation only. Prior fixture builds, Windows Debug/Release tests
and Rust tests above are retained evidence, not rerun. Verification: documentation link targets,
final diff review and `git diff --check`.
