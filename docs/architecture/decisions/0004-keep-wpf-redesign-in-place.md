# ADR-0004: Keep WPF and recompose the app instead of migrating

- **Status:** Accepted
- **Date:** 2026-09-14 (redesign accepted in commit `73d71da`; direction first written on
  2026-09-08 in `0827b28`); recorded retrospectively on 2026-09-19

## Context

The first Windows app delivered the MVP capabilities as seven peer tabs over the worker. The
redesign's discovery review (`plans/ui-redesign/discovery/initial-findings.md`, preserved at the
`v0.1.0` tag) found that users had to discover the scan-to-review journey themselves; that filters
and summaries left the duplicate list a thin strip and pushed decision columns offscreen; that the
age and ownership of the displayed results were hard to see because run selection lived in
History; that loading could show two contexts at once; and that an unavailable result could be
mistaken for "no duplicates". The product is used for long and repeated scans of large drives with
many results and deep paths, which made these costs recurring.

The MVP plan had fixed C#, WPF and .NET 10 with WPF's built-in Fluent theme, CommunityToolkit.Mvvm
and the worker process ([ADR-0001](0001-worker-process-boundary.md)). The redesign's product
direction (`plans/ui-redesign/product-direction.md`, preserved at the `v0.1.0` tag) weighed these
alternatives:

| Alternative | Assessment recorded at the time |
|---|---|
| Theme and spacing cleanup only | Not enough; keeps the same navigation and space allocation |
| Mandatory scan-to-delete wizard | Clear first run, but poor for repeated comparison and history, and execution is disabled |
| Explorer-style folder tree as the main result model | Familiar, but duplicate sets cross folders and drives, and a tree hides those relationships |
| Migrating to another UI framework | No demonstrated need; would add unrelated native, accessibility and lifecycle work |
| Persistent list/detail workspace with guided transitions | Chosen |

## Decision

Keep WPF on .NET 10 with the Fluent theme and the long-lived worker. Recompose navigation, layout,
state presentation, commands and wording into a persistent workspace with four areas (Scan,
Results, Review, History), a collapsible saved-scan selector, and guided transitions between them.
Results remains the main working area after a scan; no sequential wizard is forced on repeat work,
and a scan finishing in the background does not pull the owner away from what they are doing.

The redesign kept the product's invariants: one active scan at a time; one clearly identified run
owns the displayed results and decisions; scan findings are historical and current validity is
separate; selecting never changes a decision and marking never deletes; the UI cannot override
survivor protection, hard-link identity, folder overlap, revision checks or cloud exclusions;
counts and bytes come from the worker; filters never silently widen a rule or bulk scope; server
paging bounds stay; and no file-system, database or native call moves onto the UI thread.

## Consequences

- **The structure in code.** `WorkspaceArea` and `WorkspaceDestination` model the four areas and
  their destinations, with per-area memory and lazy panes
  (`apps/windows/src/SuperDuper.Windows.Core/ViewModels/ShellViewModel.cs`); see
  [Windows app components](../windows-app-components.md#shell-and-navigation).
- **Completion does not interrupt.** The app moves to Results by itself only when the owner is
  watching that scan's progress; otherwise it shows a notice.
- **Contextual detail screens.** Performance and warnings open from the scan or history they
  describe and return there (`PerformanceReturnDestination`).
- **User vocabulary.** "Saved scan" names the reusable definition and "scan" one run, while code
  and protocol keep session and run ([conventions](../windows-app-concepts.md#wording)).
- **No new platform work.** The worker process, the three-project MVVM structure and the test
  layering carried over. WPF-specific accessibility checks (high contrast, screen readers,
  per-monitor DPI) remain open (issue #23).
- **Scope held.** Automatic deletion, Recycle Bin enablement, pause and resume, simultaneous scans,
  shell extensions, previews and export stayed out of scope.
