# Prepared A01/A02/A09 desktop walkthrough

Status: **fixture launched; operator observations requested and pending**. UIR-03d's local viewport prerequisite
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
