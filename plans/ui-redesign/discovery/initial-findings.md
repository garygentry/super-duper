# Initial UI/UX findings

Date: 2026-09-08. Source baseline: `52126cd`, followed by preservation-only README commit `deefa40`.
Method: direct visual inspection of the running WPF app, UI Automation text, XAML review, and
targeted view-model/protocol review. This is a heuristic evaluation, not a moderated usability
study, benchmark, or physical accessibility acceptance campaign.

## Evidence limits

- Visually inspected Setup, Progress, Run history, Duplicate files, Duplicate folders, Preflight,
  and Performance at the existing window size, approximately 1298 x 928 capture pixels.
- The selected production session was cancelled. No scan, validation, rule application, review
  mutation, or file operation was intentionally requested. Navigation was restored to the original
  session and Progress tab. No app or worker process was stopped.
- Opening an existing completed session showed some populated result data but remained behind
  a loading overlay across several observations. This supports a loading-experience concern;
  it is not a measured latency result or proof of the cause. The fully interactive populated
  comparison workflow was not verified live.
- The running artifact's exact source identity was not independently established. Runtime
  observations and checked-out source findings are recorded separately rather than claiming an
  exact-build regression. No screenshot binaries containing personal paths are added to Git.
- A future isolated fixture-backed WPF walkthrough must cover populated results, rule application,
  preflight, long paths, narrow layouts, keyboard, and terminal/error states.

## Findings register

Priority: P1 materially obstructs understanding or the core workflow; P2 affects fluency/polish.
These are design priorities, not assertions that an accepted correctness gate failed.

| ID | Priority | Observation and evidence | Consequence | Direction / acceptance reference |
|---|---|---|---|---|
| F01 | P1 | Seven peer tabs in `MainWindow.xaml`; Setup, Progress, history, two result modes, Preflight, Performance | User must discover the scan-to-review journey | Four primary areas and explicit transitions; A01, A02 |
| F02 | P1 | File filters, root/drive controls, rule expander and summaries consume most of the inspected Results height | The actual duplicate list is reduced to a thin strip | Compact toolbar, disclosed filters, majority-height list/detail; A03 |
| F03 | P1 | File member columns total at least 1480 DIPs: fixed widths plus 220 minimum relative-path width | Decisions and path actions can sit far offscreen | Lean comparison rows, selected-copy command area; A03, A04 |
| F04 | P1 | Session header contains a name and generic slogan; run selection happens in History | Results' age and ownership are hard to see | Persistent scan identity and date; A01, A07 |
| F05 | P1 | During completed-session loading, content and header changed while the status bar retained the previous session's cancelled counts | Different contexts appear together | Atomic context labeling, scoped loading/errors; A02 |
| F06 | P1 | Cancelled Files view shows zero summaries and overlays 'Results unavailable' on a small table area/header | Unavailable can be mistaken for no duplicates; message layout is cramped | Distinct terminal/empty/filter states; A05 |
| F07 | P2 | 'server-paged', 'immutable', 'Normalize roots', 'operation foundation', and worker-owned telemetry text appear in UI | Technical precision creates user effort | Plain-language copy catalog and disclosed diagnostics; A06 |
| F08 | P1 | Progress is a long stack of pipeline counters; remaining-time and current activity follow multiple sections | Hard to answer 'what is happening and is it progressing?' | Phase/activity summary first, expandable detail; A08 |
| F09 | P2 | Repeated bordered cards, individually padded buttons and dim secondary text; shared resources in App.xaml only include a converter | Inconsistent emphasis and avoidable visual noise | Shared semantic resources, coherent command hierarchy; A09 |
| F10 | P1 | Keep/Remove/Undecided plus path actions occupy each file row; review plans and rules span separate surfaces | Selection, decision, and actual execution can be confused | Explicit decision controls and persistent review summary; A04, A10 |
| F11 | P2 | Folder search/minimum-size boxes have no visible labels in the inspected screen; file filters have visible labels | Meaning differs between otherwise similar modes | Parallel labels, keyboard and focus conventions; A06, A11 |
| F12 | P2 | Performance has a very wide device grid and large unavailable cards | Useful diagnostics become difficult to scan | Select a device, reveal current/peak detail; A12 |
| F13 | P1 | The completed session stayed behind a workspace-wide loading overlay even after groups began appearing; source awaits several workspace loads during session selection | One slow dependency can obscure otherwise available work | Independent loading regions with truthful context; A02, A13 |
| F14 | P2 | Setup exposes raw extended-length paths and large advanced-policy sections in the primary flow | Location choice and start action are harder to scan | Friendly display paths, keep exact stored paths for operations; A06, A14 |

## What should be retained

The engine/workspace separation, native WPF controls and Fluent theme, exact-only file/folder
matching, immutable scan history, bounded server paging, reversible durable decisions, preference
rule provenance, survivor checks, cloud exclusions, warning aggregation, and live-state invalidation
are valuable. Polish must preserve these behaviors rather than recreate them with local UI guesses.

The accepted historical plan already intended a trustworthy review journey. The redesign improves
its presentation and composition; it does not erase accepted engineering work or restart historical
acceptance campaigns. Some old narrative 'gaps' in the post-MVP document predate later implementation;
the current source and handoff take precedence when assessing present capability.

## Source evidence

- [Shell](../../../apps/windows/src/SuperDuper.Windows/MainWindow.xaml) and
  [selection/lifecycle orchestration](../../../apps/windows/src/SuperDuper.Windows.Core/ViewModels/ShellViewModel.cs).
- [File results](../../../apps/windows/src/SuperDuper.Windows/Views/DuplicateFilesView.xaml),
  [folder results](../../../apps/windows/src/SuperDuper.Windows/Views/DuplicateFoldersView.xaml).
- [Setup](../../../apps/windows/src/SuperDuper.Windows/Views/SessionSetupView.xaml),
  [progress](../../../apps/windows/src/SuperDuper.Windows/Views/ScanProgressView.xaml),
  [history](../../../apps/windows/src/SuperDuper.Windows/Views/RunHistoryView.xaml).
- [Preflight](../../../apps/windows/src/SuperDuper.Windows/Views/PreflightView.xaml),
  [performance](../../../apps/windows/src/SuperDuper.Windows/Views/PerformanceView.xaml),
  [application resources](../../../apps/windows/src/SuperDuper.Windows/App.xaml).
- [Worker client contracts](../../../apps/windows/src/SuperDuper.Windows.Core/Workers/IWorkerClient.cs).
- Microsoft [navigation design](https://learn.microsoft.com/en-us/windows/apps/design/basics/navigation-basics)
  and [commanding](https://learn.microsoft.com/en-us/windows/apps/design/basics/commanding-basics),
  consulted 2026-09-08 as general Windows design guidance, not proof of this application's usability.

## Evaluation conclusion

A material interaction redesign is justified by F01-F06 and F10-F13. A theme-only pass cannot
resolve the result-space, run-context, and review-flow problems. A framework replacement offers no
demonstrated benefit for those problems and would risk existing keyboard, lifecycle and native
integration behavior. Proceed with a WPF workspace redesign and a small shared visual system.
