# Prepared A01/A02/A09 desktop walkthrough

Status: **prepared, not performed**. First fix the UIR-03c viewport limitations listed in
[the evidence record](uir-03c-populated-shell.md). This is a fictional shell walkthrough, not a
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

| Step | Reviewer action and expected observation | Record |
|---|---|---|
| A01 context | In initial Files, identify completed run 1/date/state/two roots, then identify active run 2 independently. Full long names remain available by tooltip/automation. | Interpretation, clipping, assistance |
| A01 selection | In History highlight run 2, then return to Files without Open scan: run 1 remains the workspace. Open run 2 explicitly: its summary opens; View progress still owns the active run. Open run 1 again. | Named run before/after each action |
| A02 optional load | From a fresh fixture, choose toolbar Delay folders. Navigate to Files or Scan while it waits. Choose Fail pending folders. Files/context remain usable and free of that pane's error; Folders reports its own failure. | Input responsiveness, error scope |
| Warning return | View progress, Review warnings, Close: focus returns to History. View progress returns to the active Progress tab. On completed run 1, its warning action opens immutable Files and focuses the group grid. | Actual focus after every action |
| Retention | Scroll the file group grid and choose a group; visit Review and return. Repeat with Progress's vertical scroll. Both retain same-run position; no new run is opened by this navigation. | Selected group and offsets/visible rows |
| A09 shared UI | Inspect Setup, Progress, Files, Folders, Review, History and Performance/recovery states. Compare heading/body/field/button/row sizes, disabled reasons and focus outlines. Expand filters and totals; exact field semantics remain available. | Per-screen differences and unreachable controls |
| Layout | At 1180x760 and 900x600 repeat context, paging, warnings and return. Use long fixture names. Essential actions and rows must remain reachable; record the currently known clipping as failures. | Requested/client size, screenshots, defects |
| Keyboard | Repeat navigation, filters, paging and warning return with physical Tab/Shift+Tab, native arrows, Enter/Space and existing access keys. Selection alone must not mark removal. No Delete execution shortcut exists. | Key sequence and actual focused control |
| Accessibility | Repeat with Narrator and NVDA, light/dark/high contrast, Windows text enlargement and authorized physical 100/150/200% monitor moves. Confirm announcements, selected-row text and focus remain perceivable. | Reader/version, OS/theme/scaling/monitor, pass/fail/unrun |

For each step record observations and unexpected scope changes, not merely clicks or test success.
Automation currently establishes programmatic focus only. The fake data do not establish worker
performance, screen-reader cadence over a real long scan, representative result scale or A17 rescan
acceptance. Those requirements retain their later gates and separately isolated procedures.
