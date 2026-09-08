# UIR-03c — shared resources and populated shell verification

2026-09-08. Baseline `c91c458`; implementation is this record's commit on
`codex/ui-redesign`. Scope: `local_code`, partial A01/A02/A09 and regression A15.
UIR-03 remains **in_progress**. This record does not accept the populated layout or physical UI.

## Implemented

- Primary views and recovery use shared page/section/caption typography, 14-DIP body/controls,
  32-DIP minimum fields/actions, 36-DIP data rows, content insets and progress/performance cards.
  Visible text no longer uses fractional opacity; error borders use system brushes. Native templates,
  focus visuals and inherited selected-row text colors remain. The shipping Fluent/System theme
  selection is unchanged.
- Long workspace/active names and the Start target have bounded displays and full tooltips;
  Start's automation name retains its full target. Active phase, elapsed time and warning count
  occupy a separate line so the name cannot truncate them. History copy wraps beside its actions.
- Files filters/rules and result totals/location coverage have native disclosures. The filter
  disclosure has a bounded scrolling area. This is an interim correction for crowded composed
  views, not acceptance of S03: the compact always-visible search/filtered figures and full
  responsive list/detail design still require implementation. No filters, rules or notices were
  removed; dirty-root warnings remain outside the totals disclosure.
- View progress now requests focus on its actual Scan tab. MainWindow snapshots focus request and
  navigation generations, including away-and-back navigation. File-grid focus retries stop when
  their originating request is stale. Closed windows detach the shell property listener.
- A test-only internal constructor lets the populated loaded-STA host manage fake service lifetime.
  The public production constructor always retains owned-worker shutdown. Production App startup,
  injection and cancellation/disposal logic are unchanged.

## Fixture and focused evidence

`ShellFixtureData` is shared by the WPF test and standalone desktop fixture. It uses the shipping
ShellViewModel/MainWindow/views with in-memory worker, folder-picker, clipboard, Explorer,
confirmation and cloud services. It contains a long-named saved scan, two fictional roots, an old
completed run, an independent active run, 25 file groups/two copies per group and one warning per
run. Result summary totals are supplied consistently by the fake service. Review has no decisions;
folder/performance representative datasets remain future coverage.

The populated scenario runs inside the existing WPF surface test's Application/STA; the suite still
reports four test methods. Its assertions now verify actual `IsKeyboardFocused`/`IsKeyboardFocusWithin`
through MainWindow and RunHistoryView for Open scan, View progress, warnings, Close warnings and
warning-result navigation. No direct FocusManager assignment establishes these success checks.
An away-and-back navigation test protects against stale tab focus. Native progress ScrollViewer and
file-grid nonzero scroll offsets, file selection and one-query same-run reuse survive pane reopening.
A delayed failing folder query leaves Files and the selected historical context usable.

The older shell-only reordering test remains separately useful; its empty slots/logical-focus
limitations are not upgraded by this record. Programmatic keyboard focus is not physical keyboard
or assistive-technology acceptance.

Requested window sizes 1180x760 and 900x600 were captured for Setup, Progress, Files, Review and
History using RenderTargetBitmap at 96 DPI. Final PNGs are local ignored evidence in
`artifacts/uir03c/captures-final`; earlier captures are in `captures`. The offscreen host supplies
a system window background because a bitmap does not include the native window background;
the shipping theme background is not overridden. These are system-brush fixture renders, not
qualifying Fluent light/dark/high-contrast or physical DPI captures.

## Integration and isolation

Git began clean at `c91c458`. `wpf-poc` remained `deefa40ebe607b785b395a29a6282e8b417a9b14`.
Command-scoped `safe.directory` handled sandbox ownership; no global Git changes. Operator WPF
36316 and worker 17612 at `artifacts/windows-x64` were observed repeatedly and left untouched.
No production state, real-drive scan, consumed campaign or provider opt-in was used.
Final process audit found the same operator app/worker and dotnet PIDs 14304/30856/43512/51420/52048;
no task worker, fixture window, cargo/rustc or testhost remained. The dotnet processes were left
alone. Re-audit rather than reusing these identities next session.

| Verification | Result |
|---|---|
| Rust workspace build, Debug and Release | Passed |
| Rust workspace tests, each configuration | 226 passed, 0 failed, 10 explicitly ignored |
| Windows solution build, Debug and Release | Passed; zero warnings/errors |
| Core tests, each configuration | 170 passed |
| Infrastructure tests, each configuration | 75 passed; five operator-only tests skipped |
| WPF tests, each configuration | Four passed, including the populated scenario |
| Standalone desktop fixture | Build passed; offscreen startup/shutdown `--verify`, PID 45952 exited 0 |
| Final whitespace/source review | `git diff --check`; no engine/protocol/production-executor changes |

Rust used `CARGO_TARGET_DIR=<repo>/artifacts/uir03c/target`, `cargo build/test --workspace --locked`
and Release equivalents. Logs are `rust-{debug,release}-{build,test}.log`. The default ignored
physical/performance tests were not enabled. `TEMP`/`TMP` for integration were task-owned
`artifacts/uir03c/temp`; the original shell environment was not modified globally.

Windows used `--artifacts-path <repo>/artifacts/uir03c/dotnet`, solution build then `test --no-build
--no-restore`, and `-c Release` for Release. Builds selected
`-p:WorkerExecutableSource=<repo>/artifacts/uir03c/target/<profile>/super-duper-worker.exe`.
The real-worker Infrastructure tests find that isolated target directory while walking ancestors
of their own output. They create and dispose only their own disposable workers/state. Three
physical/provider opt-in environment values were cleared for those runs. Logs are
`dotnet-{debug,release}-{build,test}.log`; TRX files under `results` use `debug-integration` and
`release-integration` prefixes. Final resource/focus review was followed by focused Debug/Release
WPF reruns (`wpf-final-reviewed.trx`, `wpf-release-reviewed.trx`).

The first sandbox build could not read installed SDK metadata; approved execution used installed
SDK/NuGet dependencies with isolated outputs. The first populated attempt failed because WPF BAML
cannot initialize a MainWindow subclass defined in the test assembly. The internal presentation
constructor fixed the host; failed `wpf-initial.trx` remains. Subsequent captures exposed excessive
header growth, transparent offscreen background, and crowded Files/History content. Header/background
and disclosure corrections improved the evidence; remaining layout failures are explicitly below.

## Open acceptance and exact next slice

**UIR-03d, local_code:** repair the composed shell's remaining viewport problems using this populated
fixture, then perform the A01/A02/A09 desktop walkthrough. At 900x600, Files can still have no usable
group/member grid viewport. At 1180x760 its comparison content is crowded/clipped. History's narrow
grid is cramped. Do not mark these renders passed, raise the minimum window size, reduce the new
text/control sizes to conceal the problem, or call the A03 60%/no-horizontal-scroll criterion passed.
Keep the full S03 results design in UIR-05; resolve the minimum shell composition prerequisite first.

The [desktop procedure](uir-03-desktop-walkthrough.md) and `Invoke-UiRedesignFixture.ps1` are ready.
Physical keyboard, Narrator/NVDA, light/dark/high contrast, Windows text enlargement and physical
100/150/200% multi-monitor DPI remain unrun. Resolve local layout blockers before requesting the
specific operator walkthrough. UIR-03 cannot close on the automated matrix alone.

Independent active progress/cancellation, worker ownership, survivor/revision/overlap protections,
collection ceilings, disabled production deletion and persistent rescan/long-scan A08/A16/A17 remain.
SOP10 stays consumed/complete and Windows post-MVP release validation stays parked.
