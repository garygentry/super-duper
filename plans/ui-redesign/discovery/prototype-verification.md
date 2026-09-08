# Planning-package verification

Date: 2026-09-08. Product source baseline: `deefa40`. This record applies to the design artifacts
committed with this file, not to a redesigned WPF executable.

## Checks performed

- JavaScript syntax: `node --check plans/ui-redesign/prototype/app.js` passed.
- Local Markdown and HTML asset references resolved within the repository; missing references
  were checked before commit. Findings F01-F14 map to acceptance criteria A01-A15 and finite gates.
- Browser: local concept served only from `plans/ui-redesign/prototype` on loopback. Separate
  background test tab used after the initially visible tab's navigation changed between observations.
  The cause of that initial navigation change is not asserted; the separate tab avoided interference.
- Mark a fictional file: plan changed from zero to one distinct file / 1.5 GiB; Review displayed
  that set and total. Demo check completed for the current revision.
- Mark the containing duplicate folder: total became two distinct files / 1.55 GiB, not the sum of
  overlapping file and folder marks. Prior demo validation became 'Plan changed — check again'.
- Reset the folder mark: total returned to one file / 1.5 GiB. Reset the file mark: zero / 0 B.
- No-match scenario: distinct explanatory state appeared; Clear filters restored all three file sets.
- Applying the >=1 GiB filter left the one large file set. Marking its two backup copies produced
  two distinct files / 3.0 GiB; attempting to mark the remaining copy was rejected with a visible
  explanation and the two-copy total stayed unchanged. This is only a fictional concept guard,
  not validation of the production survivor contract.
- Running scenario: active run had a separate identity while Results retained the earlier completed
  run's date. Cancellation changed the demo terminal state; Results explicitly became unavailable.
- Warnings opened as a dialog and closed with Escape. Review caught and corrected the concept's
  initially ambiguous warning title: active-scan warnings now explicitly name the active date/time.
- Changed-copy scenario: the changed copy's Mark for removal control was disabled.
- First-use scenario displayed location setup; Start demo scan selected the static running scenario.
- Browser console reported no error/warning entries during the checked flows.

## Layout observations

Inspected the concept in the host's dark appearance at 1180x760, 900x600 and 680x760 CSS-pixel
viewports. At standard width the initial fixed-height comparison pushed Review below the viewport;
this was corrected by sizing the app to the viewport and using independently scrollable panes.

The 1180x760 inspected layout allocated about 297 pixels to list/detail within about 475 pixels of
usable content height (excluding content padding), approximately 62%. This is a concept observation,
not proof of the future native layout target. At 900x600, no document horizontal overflow was found
and the footer remained at the viewport bottom; vertical content/pane scrolling is needed. At
680x760, selecting a set revealed comparison with a visible Back to sets action; returning restored
the set list. That width is exploratory and below the current Windows minimum width.

The final viewport-sizing correction removed the extra outer-page height caused by a hard-coded
design-toolbar allowance. Temporary browser viewport overrides were reset after inspection.

## Limits and remaining evidence

No Rust/.NET builds or product test suites were run: this slice changes documentation, repository
scheduling/ignore rules and a standalone design concept only. No WPF source, schema, worker contract,
production runtime state or safety lock was edited. Browser interactions affect fictional in-memory
state only. No data is persisted, and no file/engine actions execute.

The concept intentionally does not implement the complete filter set, native saved-scan selector at
narrow widths, server paging, rule application, operation recovery, actual preflight, or native
focus/automation behavior. Layouts in light/high-contrast mode, physical DPI changes, Narrator/NVDA,
real path operations and large-result performance are not accepted by these checks. UIR-08 must
collect native evidence. A source review of color rules is not a physical contrast pass.

## Repository preservation checks

Before the redesign commit: `wpf-poc` points to `deefa40`; `codex/ui-redesign` descends from that
commit; the original README Git blob is unchanged between these branches. Only this branch includes
the redesign package and startup routing. `git diff --check` and scoped staged-file review are part
of the commit boundary. No branch was deleted, merged, rebased or remotely published.
