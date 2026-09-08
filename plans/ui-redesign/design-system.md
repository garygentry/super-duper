# Visual, layout and accessibility direction

Use WPF's built-in Fluent theme and system theme selection. The concept's colors illustrate the
hierarchy; WPF semantic resources and system brushes own the actual contrast behavior. No new UI
framework or third-party control library is selected by this plan.

## Starting tokens

All sizes below are device-independent pixels. These are design starting values, to verify with
real WPF text metrics; they are not a replacement for DPI or text-scaling testing.

| Role | Starting treatment |
|---|---|
| Application/page title | 24 / 20, semibold |
| Section title | 16, semibold |
| Body and controls | 14; secondary text 12 only when nonessential |
| Spacing | 4, 8, 12, 16, 24; standard content inset 24 |
| Control height | Minimum 32; primary task buttons 36 |
| Dense data row | Minimum 36; multi-line copy row grows to content |
| Corners | 6 for inputs/panels; use borders only to separate useful regions |
| Primary color | System accent; text/focus/selection use contrast-safe semantic brushes |
| Status | Success/warning/error paired with words and an icon, never color alone |
| Numeric values | Right-aligned, consistent units, tabular figures where available |

Create shared resources for page/section/body/caption text, spacing, fields, command bars, status
banners, empty states and data rows. Keep default focus visuals and keyboard behavior. Avoid global
opacity on essential text; contrast must be achieved by explicit theme-aware foreground/background.
Use the existing application icon, restrained native iconography and visible action labels.

## Layout rules

- Starting shell rail 208 DIPs, collapsible; context/navigation overhead is compact and stable.
- At roughly 1180 x 760 and larger, Results devotes at least 60% of usable content height to the
  set list and comparison. Measure after the normal collapsed-filter toolbar, not with every
  optional panel open. Users may expand advanced panels temporarily.
- Standard list/detail split: about 36/64, adjustable, with a useful minimum detail width.
- At the current 900 x 600 minimum window, collapse the saved-scan rail and use list/detail
  navigation if the split cannot fit. Do not raise minimum window size to hide layout problems.
- Keep essential filename/path, decision, selected state and primary actions available without
  horizontal scrolling. Secondary technical tables may scroll within their own region.
- Path display preserves discriminating segments and full selectable text. Ellipsis is a display
  choice, not data transformation. Very long names wrap in details without growing grid columns.
- Drawers/panels have a visible title and Close button. A modal owns focus and returns it on close;
  a nonmodal detail panel does not silently trap focus. Use one predictable pattern for each.
- Persistent review totals do not compete with filtered-result totals. Label both scopes.

## Input and focus

Keep native list/grid arrow navigation. Preserve existing access-key behavior (set navigation,
visible-page validation, cancellation and warnings) or document and test an intentional replacement.
Add Ctrl+F for path search where it does not conflict. F6 may move between the set list and comparison
if tested with assistive technology. Escape closes transient panels and restores their trigger.
Do not bind Delete to file execution. Space/Enter on a selected item must not mark it for removal
unless an explicit decision control owns focus.

Tab order: context/navigation, query controls, set list/paging, comparison/decisions, review shortcut.
On query completion preserve focused controls; on explicit set navigation move focus consistently
to the selected set or detail heading. Announce query count/status, not every row. Keep the existing
latest-only UIA delivery and separate slower progress-announcement cadence (ordinary updates at
most once per five seconds, meaningful lifecycle changes allowed immediately).

## Visual acceptance

Check light, dark and Windows high contrast with real controls. Verify keyboard focus against
selected rows, disabled controls with visible reasons, and accent/semantic brush contrast. Test
100%, 150%, 200% DPI and Windows text enlargement, including moving between monitors, without
clipping or inaccessible commands. Browser prototype checks are not WPF acceptance evidence.

Prefer practical density over decorative dashboards: a few useful summaries, generous space for
comparison, quiet separators, and one evident primary action for the current task.
