# Autonomous usability and polish execution plan

Status: accepted; implementation authorized after the operator interview and return to Default mode.
Branch: `codex/ui-redesign`. Preserve `wpf-poc`; no merge, push or release implied.

## Outcome

A person can choose locations, scan, understand results, compare copies, mark a safe plan,
check it and return later without learning engine terminology. Frequent work occupies the
screen. Expert detail remains available on demand. All current capabilities remain reachable.
Readiness means the scope explicitly chosen by the operator, not an implied production release.

## Accepted interaction and visual contract

- Equal emphasis on file cleanup and backup folders; flexible navigation; quiet Windows-native UI.
- Retain 900×600 via adaptive layouts. Remember individual section disclosure across restarts using
  a small Core preference contract and Infrastructure JSON store, isolated from worker databases.
  Missing/corrupt settings fall back safely; errors remain visible regardless of preferences.
- On successful completion, open the remembered Files/Folders view only while watching that exact
  active scan and no modal is open. Otherwise retain navigation and show a completion notice.
- Mixed actual archive data and a final direct native mouse/keyboard pass are mandatory; locked
  desktop availability is an external prerequisite only for that final pass, not implementation.

- Keep WPF and the Rust worker boundary. Use restrained Windows-native typography, system
  theme/contrast support and semantic accent/success/warning/error brushes.
- Four destinations remain recognizable: Scan, Results, Review, History. Remove decorative
  nested-tab chrome; use a compact Files/Folders switch within Results. Scan shows setup or
  current status as appropriate; settings and prior-run summary remain directly reachable.
- Saved scans live in a collapsible rail/selector. At narrow widths and enlarged text, collapse
  it by default and retain an accessible named selector. Do not obscure active-vs-opened run identity.
- One short header: saved-scan name, dated run/status and one contextual primary action. Active
  monitoring outside Scan uses a compact strip with a link back. No persistent protocol/version footer
  during healthy operation; connection failures still announce themselves and offer recovery.
- Primary controls use a distinct accent; secondary controls are quiet; destructive operations
  remain explicit and separated. Labels stay on Start, Stop, Keep, Mark for removal, Check and Apply.
- Icons: add (+), refresh, search, filter, copy path, open location, back, paging chevrons,
  reorder arrows, dismiss and overflow. Use a shared vector/glyph resource, not emoji or raster
  approximations. Every icon button has an accessible name, keyboard focus, tooltip, disabled
  explanation when needed, and at least a 32-DIP target. Never rely on color or an unfamiliar icon.
- Spacing tokens: 4/8/12/16/24 DIPs; shared input/button heights and corner radii. Type roles:
  page title, section, body, caption; all respect Windows text scale. Avoid nested outlined cards
  unless the border communicates grouping, selection or status.
- Default copy is a label or one short sentence. IDs, protocol, revisions, snapshot mechanics,
  physical-vs-logical accounting and log paths belong in Details. Keep user-critical scope,
  incomplete work, stale checks and unavailable actions visible.
- Full paths are copyable and inspectable. List rows emphasize filename and meaningful location;
  do not arbitrarily truncate the segment distinguishing two copies. Selection alone never marks.
- Setup: choose locations → Start. Summarize exclusion/cloud/reuse policy; disclose advanced
  settings. Keep saved configuration separate from the immutable settings of an old run.
- Results: search + Filters, three concise totals, meaningful group list, comparison area, then
  clear Keep/Mark states. Avoid stacked nested scroll panes; preserve bounded data virtualization.
- Review: marked totals and check/readiness first; switch between Files/Folders details; rules
  open a focused flow. Rule preview is visibly tentative; apply and reverse retain exact scope.
- History: date, outcome, result summary and Open. Warnings/performance open contextually and
  return to the originating control without changing the workspace run.
- Standard states on every screen: initial/empty, loading, success, stale, partial, error and
  retry. Use specific empty-state next steps. Keep accepted data on a failed refresh.

## Finite gates

| Gate | Dependencies | Work and exit evidence | State |
|---|---|---|---|
| P00 Review and plan | None | Inventory, baseline findings, real-file preparation, operator decisions captured | Complete |
| P01 Reliable real-app exercise | P00 | Real-worker WPF runner, independent state/data, native capability ledger; baseline journeys and issue backlog | Implemented; background-verified |
| P02 Shared shell and design system | P01 | Navigation, responsive selector, hierarchy/icons/tokens, startup/error states; representative actual WPF captures | Implemented; background-verified |
| P03 Setup, scanning and rescan | P02 | Simplified start path, settings disclosure, honest monitoring/stop/summary; rescan/restart/history journeys | Implemented; background-verified |
| P04 File and folder review | P03 | Comparable rows, unified filters/units, focused actions, paging/focus and narrow layouts; file/folder real-data decisions | Implemented; background-verified |
| P05 Review and preferences | P04 | Visible totals/check, staged rule flow, stale/error/readiness, contextual operation evidence; real preflight/rule checks | Implemented; background-verified |
| P06 History and diagnostics | P05 | Compact history, issues-first warnings, secondary performance/recovery details; exact navigation/context tests | Implemented; background-verified |
| P07 Integration and defect closure | P02–P06 | Full journey/theme/accessibility matrix, Rust/.NET Debug+Release, real-worker restart/edit/overlap cases | Background matrix complete; native acceptance pending |
| P08 Final quality audit | P07 | Independent fresh-state repeat of common journeys; all required findings closed with evidence, durable handoff | Fresh-state/ranking/confirmation/Stop and compact native checks pass; UX23 loading fix after-check, remaining keyboard/text/high contrast pending after Control Panel approval timeout |

Necessary engine/worker/infrastructure bug fixes run within the gate discovering them. Record
reproduction, expected/actual result, smallest compatible fix, targeted regression, and relevant
integration check. Do not expand into unrelated optimization or resurrect consumed performance
campaigns. Contract changes needed for a common workflow get an explicit plan amendment and tests;
routine bounded fixes need no further operator approval.

## P01: environment and test data

1. Use existing paired Debug build and `Start-WindowsUiDev.ps1`; use a stable private database,
   status database, hash cache and log directory per journey. Never point the test app at normal
   app state. Record build identity, process ownership, dataset identity and paths in each run.
2. Keep `New-WindowsPolishCorpus.ps1` as the quick real-document baseline. Extend with actual
   available media/archive/source files copied into a new disposable tree, manifest source/hash,
   add renamed/nested/unique copies and long Unicode paths. Never modify original source files.
   Expand above 200 distinct groups and 200 members where practical, plus an optional bounded
   larger corpus for monitoring. Do not pretend the current 7 MB corpus measures drive performance.
3. Add real file mutations only within disposable copies: changed bytes, missing files, new copies,
   file locks, exclusions, overlapping roots, and hard links where supported. Separate baseline
   and mutation corpora; regenerate a new tree rather than deleting unknown contents.
4. Add a test-only loaded-STA runner that instantiates the production shell and real Infrastructure
   worker client with private state. Drive actual controls/commands and dispatch/layout, capture
   production WPF at checkpoints, and assert persisted worker results. No fake worker, database
   seeding or fabricated results on the end-to-end path. Existing mocks remain for rare errors,
   timing and scale unit tests only; label that evidence separately.
5. Computer Use is the preferred direct exercise when the desktop is available: launch the exact
   pre-existing built app through `sky`, reacquire returned window, observe → one action → observe.
   Verify launch/select/type/click/scroll/dialog/resize/close on the real dataset; do not reuse
   stale indices. Use the Debug `.uidev` sidecar only for the session and remove it at cleanup.
6. When locked or native capture/input fails, record exact failure and stop native input calls.
   Continue the real-worker WPF runner, builds, analysis and screenshots. No authentication or
   security-setting changes, auto-login workaround, or unlocking requirement for independent work.
   Native tests remain pending unless actually run or explicitly removed from the chosen scope.
7. Avoid repeated helper debugging: one documented recovery attempt; then use the background lane.
   The runner must bound waits, drain worker stderr, close owned processes and report timeouts.
   Never kill unrelated processes. Test-only seams must not ship a production automation endpoint.

## Acceptance matrix

| Journey / dimension | Required observations |
|---|---|
| First use | From empty state: create saved scan, choose two folders, start; no advanced setting required; invalid location has inline actionable error |
| Repeat use | Start a new run in one action from ready saved setup; changed settings saved correctly; prior results/decisions unchanged |
| Monitoring | Phase, elapsed, freshness and Stop visible; indeterminate vs known totals honest; cancellation and all terminal states clear |
| Files | Search, unit filter, root/drive/extension facets, clear, sort, page; inspect both locations; Keep/Mark/Reset; copy/reveal; retained focus and scroll |
| Folders | Exact contents and descendant scope clear; same filter/control conventions; safe overlap-aware decisions; reveal respects bounded selection |
| Preferences | Order preferred roots; save; scoped preview; apply confirmation; reverse same application; stale preview/manual overrides handled |
| Review | Correct combined marked counts/bytes; check every marked target/survivor; stale after edit/decision; blocked/missing/unavailable explained |
| Filesystem changes | Add/change/remove/lock disposable files; fresh rescan and validation accurately detect changes; cache fallback is safe |
| History | Restart app/worker, reopen earlier run, inspect recorded settings and exact warnings/performance; live scan stays separate |
| Resilience | Worker failure/restart, failed page refresh, rapid switching, draft navigation, unavailable paths; no mixed run/revision data |
| Secondary features | Empty/no-match, cloud/exclusion status, folder relationships, reconciliation, diagnostics and existing recovery evidence remain reachable |
| Accessibility | Keyboard-only common journeys, meaningful icon names, visible focus, Escape/Back restoration, no color-only decisions, contrast themes |
| Layout | 1180×760 and 900×600; Light/Dark/high contrast; 100% and 150% text; available DPI levels; no clipping/overlap/unreachable essential action |

### Measurable polish checks

- One visually dominant primary action per screen state; no repeated Start/Scan again controls.
- At 1180×760 and normal text, results list/comparison gets at least 60% of client content height
  (exclude only OS title bar, not app chrome); selected-copy decisions remain reachable without
  scrolling unrelated page headers. Measure actual geometry, not a loosely defined inner container.
- At 900×600 and 150% text, show at least one meaningful result row and an obvious comparison/back
  path. Selected-copy decisions and Review check remain visible or in the same short local scroll
  region; no blank sliver counted as usable content. Collapse/reflow instead of shrinking text.
- Review's marked total, current check status and Check action appear on entry at standard size.
- A known set can be opened and marked in at most three deliberate interactions after Results
  loads (select set, select copy if necessary, mark); switching mode must not reset accepted state.
- Advanced details closed: no raw protocol, worker-query, revision/signature or telemetry-contract
  explanation in the common workflow. Preserve these through an accessible Details action.
- No horizontal page scrolling; full path remains available. Avoid nested scroll regions with
  less than one usable row. Pagination remains explicit and no selection implies all-page scope.
- Every issue in `review.md` has an after capture/behavior check and a closure explanation.
  New issues get severity, reproduction, owner gate and status; nothing disappears into prose.

## Iteration and completion discipline

For each coherent journey: inspect baseline → implement → focused behavior test → real-worker UI
exercise → inspect screenshot at default and stressed viewport → fix → repeat changed cases.
At least one fresh-state pass after fixes must complete without another blocking/high/medium
usability finding. Do not set an arbitrary iteration count or treat passing tests as visual approval.

At integration, run Rust workspace and Windows solution build/tests in Debug and Release with
matching workers. Run relevant isolated smoke and real-data workflows; do not execute broad
physical/provider/release scripts without reading their scope. Retain expected skips explicitly.
Screenshot tests complement manual visual inspection by the agent. Native interaction, rendered
WPF, real-worker protocol and mocked timing tests are separate evidence categories.

P08 requires every in-scope issue resolved or a documented operator-agreed exclusion, all required
journeys passing, no leaked process/sidecar, clean committed work, and a concise final capability/
limitation statement. If native evidence is still unavailable, report that specific gap; do not
declare full native readiness. Missing external capability cannot be solved by waiting indefinitely.

Commit each coherent slice with checkpoint and shared-handoff updates. Continue autonomously after
plan decisions; ask only for a genuinely new product/safety boundary or unavailable required input.
The operator explicitly authorized subagents with task-appropriate models/effort. Assign exclusive
file ownership, review results centrally and coordinate shared builds. Independent P01 tooling and
P02 foundations may proceed concurrently; no gate passes before its dependencies. No new user tasks
or recurring automation are requested. At a session handoff include a copyable
prompt with committed state, exact next gate, verification and unresolved blockers.
