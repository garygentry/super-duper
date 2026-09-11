# UIR-03e — Progress/Summary scrollbar clearance

2026-09-11. Baseline `8d2354c`; implementation is `ae8d7d1` on
`codex/ui-redesign`. Scope: `local_code`, operator-reported A09 layout defect in the UIR-03
desktop walkthrough. UIR-03 remains `in_progress`; operator verification of the fix is pending.

## Operator evidence and defect

The operator passed completed-run warning **Open duplicate results** navigation/focus and the
requested keyboard-only filters/paging/warning journey, bringing the scoped pass count to eight.
The third check reported: "On Scan/Progress, Scan/Summary tabs, main body outer-most scroll bar
overlaps right side of body content area". Shared styling/expanded controls are not accepted by
that report. The operator subsequently confirmed the original overlap was visible at both fixture
sizes (1180x760 and 900x600), with the standard Windows theme and 100% display scaling. Separate
Windows text-size, OS build and monitor details were not supplied. This clarifies the original
defect environment; it is not a result for the corrected build, whose recheck remains pending.

Progress and Summary use the same `ScanProgressView`. Its 24-DIP page margin surrounded the
ScrollViewer, leaving the body flush with the viewer's right edge. The native Fluent scrollbar
overlays that edge. Files and History already put their page inset inside their scroll viewers.

The loaded-STA regression reproduced the defect before the fix at 1180x760: body bounds
`274,302.0233,866,842.58` end at x=1140; the vertical scrollbar begins at x=1128 with width 12.
The 12-DIP overlap is also visible in the retained before capture. Failure is retained in
`artifacts/uir03e/results/scroll-clearance-before.trx` and `regression-before.log`.

## Fix and regression coverage

- Move the shared page margin from the surrounding Grid onto the body inside the ScrollViewer.
  The native scrollbar now occupies the outer edge, clear of the body. Keep the empty-state margin.
  No control templates, scrollbar behavior, commands, view models or scan contracts changed.
- Extend the existing populated WPF scenario to measure body and native scrollbar bounds on both
  Progress and Summary, at the top and bottom of scrolling, at 1180x760 and 900x600, and at minimum
  size with an 80-DIP toolbar allowance. Require a visible scrollbar/nonzero scroll range and
  body-right <= scrollbar-left. This checks geometry, not a particular margin implementation.
- Existing same-run Progress scroll, file/history reachability, warning focus and context checks
  remain. The focused regression failed before the fix and passed afterward.

Captures are under ignored `artifacts/uir03e/captures-before`, `captures-after` and
`captures-integration-{debug,release}`. After captures for normal Progress/top and minimum-size
Summary/bottom with toolbar allowance were inspected. These are 96-DPI offscreen system-theme
renders, not physical theme/scaling acceptance.

## Verification and runtime isolation

| Check | Result |
|---|---|
| Focused WPF regression | One failed before, one passed after |
| Paired worker Debug / Release builds | Passed |
| Windows Debug / Release builds | Passed, zero warnings/errors |
| Core tests, each configuration | 170 passed |
| Infrastructure tests, each configuration | 75 passed; five operator-only skips |
| WPF tests, each configuration | Four passed, including the expanded populated geometry scenario |
| Separate corrected fictional fixture build | Passed, zero warnings/errors |

Build/test logs and TRX files remain under `artifacts/uir03e`; the full integration followed the
final empty-state inset preservation. Documentation links, final diff and whitespace are checked
before commit.

All new output is isolated under `artifacts/uir03e`. Worker Debug/Release builds reuse compiler
caches under `artifacts/uir03d/target` and `artifacts/uir03c/target`, then copy paired executables
into `uir03e/target/{debug,release}`. Windows build/test uses `--artifacts-path artifacts/uir03e/dotnet`
and the paired `WorkerExecutableSource`, then `--no-build --no-restore` tests with separate TRX
prefixes. TEMP/TMP are `uir03e/temp`; physical recycle/provider/cloud opt-in variables are cleared
only in child test environments. SDK-enabled execution is required to read installed SDK metadata.
No Rust/shared contract changed; the retained Rust test baseline is not rerun or relabeled.

The operator fixture PID 58144 was observed at the old `artifacts/uir03-desktop-fixture` path before
builds and left untouched during verification. After all checks passed, its exact executable path
was rechecked and the fictional window closed normally. The corrected fixture was launched as
PID 71248/session 1 from separate `artifacts/uir03e/fixture` output and left open for review.
The operator was asked to recheck Progress/Summary at both sizes, top and bottom, and finish the
interrupted styling check; response is pending. Re-audit before reuse or another launch.
No production app, database, scan or deletion action is involved.

## Next step

Have the operator recheck Progress and Summary right-edge clearance and finish the interrupted
expanded-controls/shared-style check in the corrected fixture. Record the exact size/environment
and result. The eight passed checks remain evidence for the unchanged workflows; the local
regression does not substitute for operator verification of this defect. Physical reader/theme/
contrast/text enlargement/multi-monitor DPI evidence remains open. UIR-04 depends on UIR-03 acceptance.
