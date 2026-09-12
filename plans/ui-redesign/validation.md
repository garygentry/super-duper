# Validation and acceptance

This checklist specifies future product validation. No unchecked item is a pass. Prototype
verification establishes only that the design artifact works; it cannot accept WPF, worker,
performance, native accessibility, or deletion safety.

## Acceptance matrix

| ID | User-visible acceptance criterion | Findings | Main evidence / gate |
|---|---|---|---|
| A01 | Every Results/Review/detail screen names its selected scan/date/state; viewing old results while another scan runs cannot retarget commands | F01,F04 | Core context tests + desktop walkthrough; UIR-03 |
| A02 | Late loads/errors never mix sessions, runs or query generations; a slow optional pane does not block ready primary content | F05,F13 | Injected delayed/out-of-order responses, worker error, navigation during loading; UIR-03 |
| A03 | At 1180x760 normal layout, >=60% usable content height serves list/detail; at 900x600 essential paths and decisions remain accessible without horizontal scrolling | F02,F03 | WPF layout assertions + screenshots of populated long-path states; UIR-05 |
| A04 | Row selection is harmless; Keep/Mark/Reset target the named copy; rejected/late decisions cannot appear saved; manual reset and rule reversal have truthful semantics | F03,F10 | Existing review contracts plus focused Core and desktop tests; UIR-05/06 |
| A05 | No scan, no duplicates, no filter matches, cancelled, failed, loading and unavailable are distinct; no overlapping message/table header or fabricated zero-result claim | F06 | State fixtures + WPF render review; UIR-04/05 |
| A06 | Every field has a visible label; everyday text explains user consequences; exact paths and technical detail remain available separately | F07,F11,F14 | Copy review, long/UNC/extended paths, input validation; UIR-04/05 |
| A07 | History opens the selected run explicitly; edited setup cannot change historical parameters, results or decisions | F04 | History/open/restart tests; UIR-03/07 |
| A08 | Long scans show phase/activity/elapsed/warnings; ETA never implies unknown work is measurable; terminal states stop animation and retain historical metrics honestly | F08 | Fake-clock progress/lifecycle tests + fixture-driven WPF; UIR-04 |
| A09 | Typography, spacing, primary/secondary actions, status and focus treatments are consistent across all primary screens and recovery | F09 | Theme/resource review and desktop capture matrix; UIR-03/08 |
| A10 | Review shows worker-owned combined totals; file/folder overlap and hard-link aliases cannot double-count; changed review revision invalidates validation | F10 | Review/preflight/overlap/survivor/revision tests; UIR-06 |
| A11 | Keyboard completes core journey and returns focus after panels/paging; screen readers receive meaningful coalesced announcements; DPI/text scaling/high contrast preserve access | F11 | Loaded-STA tests plus real keyboard/Narrator/NVDA/contrast/DPI evidence; UIR-08 |
| A12 | Performance is reachable in one contextual action; unavailable/comparison-qualified summaries stay truthful; warning links retain exact run and bounded pages | F12 | Performance/warning component tests + desktop; UIR-07 |
| A13 | Existing query/page/cache/update ceilings remain; result size does not grow WPF collections; no I/O on dispatcher or unnecessary eager background panes | F13 | Instrumented collection/query tests and named existing scale verifier; UIR-08 |
| A14 | Choosing locations, exclusions, explicit saving and Start match the existing safe policy; dirty navigation is predictable; only one run starts | F14 | Setup/Shell tests including unreachable roots, failed cloud detection, save/start races; UIR-04 |
| A15 | Production execution remains disabled; no action, shortcut, context menu or false outcome implies files can be recycled | Boundary | Source/contract assertions + full regression; every integration gate |
| A16 | Current activity, measured phase bars and expandable exact diagnostics remain usable over multi-day runs; freshness, no-progress and worker failure stay distinct | D14 | Controlled clock/progress sequences, stale frames, restore/minimize, UIA cadence/focus and unavailable states; UIR-04/07/08 |
| A17 | Scan again performs fresh discovery with persistent qualified cache reuse; added/deleted/changed files affect only the new run; repeat policy and prior run context remain clear | D15 | Small isolated real-worker rescan/restart fixture, existing hash-signature regressions and Shell/history tests; UIR-04/07/08 |

## UIR-04a local implementation evidence

[UIR-04a](evidence/uir-04a-saved-scan-setup.md) implements setup/Scan again for A06/A14 and the setup
portion of A17. Core, loaded-STA WPF and a small real-worker restart/repeat fixture pass. Full A17
membership/change/fallback coverage, final native integration and operator workflow acceptance remain
later gates; the evidence record states the exact retained checks and remaining cases.

## UIR-04b local implementation evidence

[UIR-04b](evidence/uir-04b-long-scan-monitoring.md) implements the multi-day elapsed, accepted-update
freshness and terminal activity portion of A08/A16. Full Core 193 passed; three WPF methods passed,
including controlled clocks, delayed/rejected progress, minimized/restored state, all terminal states,
focus/selection/scroll retention and MostRecent announcements. These contracts remain protected by
UIR-04c; full A16/native/operator acceptance is not claimed.

## UIR-04c local implementation evidence

[UIR-04c](evidence/uir-04c-compact-monitoring.md) implements compact sampled activity, measured phase
bars and expandable exact Work / Hash reuse / Diagnostics for A08/A16. Core 200 passed; loaded-STA
phase/layout and retained clock/terminal/UIA checks are recorded in the evidence. Unknown candidate
totals use the worker's explicit known-work signal; zero totals are not percentages. No global or
per-file progress is inferred. Disclosure, path focus/selection/scroll and same-run navigation are
covered. UIR-04 integrated/full A17/native/operator validation remains later.

## UIR-05a local implementation evidence

[UIR-05a](evidence/uir-05a-file-query-controls.md) implements compact exposed file query controls and
filtered totals for the header/query portion of A03/A06/A13. Core 207 and three WPF methods passed.
Delayed/out-of-order snapshots, exact binary conversion, chips/Clear, applied rule scope, bounded
paging/facets and focus are covered. Native fixtures retain long-path, minimum/toolbar and enlarged
text reachability. Full A03 adjustable comparison remains UIR-05b; final A13 scale/integration and
physical/operator acceptance remain UIR-08/09. No whole-gate acceptance is inferred.

## Carried native checks from scoped shell acceptance

UIR-03's [scoped assessment](evidence/uir-03-shell-acceptance.md) does not complete A11. At UIR-08,
retain NVDA (`unrun_unavailable`, not installed) and physical 200% display scaling
(`unrun_unavailable`, not offered by the operator's Windows setup) as explicit requirements.
The 150% and 175% transition/focus/selection passes do not replace 200%. Obtain actual evidence
or record an explicit later disposition before claiming full corresponding native acceptance.
Do not force custom scaling or troubleshoot Windows against the operator's stated constraint.

## Scripted design/user walkthrough

Use fictional local folders and a small disposable fixture. Start with the prototype to evaluate
information architecture, then repeat against the real WPF app when implemented. The reviewer
should describe what they expect before activating decisions. Record observations, not just clicks.

1. New user: identify the purpose, choose two locations, find exclusions and repeat policy, explain
   what Start will do. Find and correct an invalid location.
2. Returning user: open a saved scan and an earlier completed run. Identify its date without History.
   Start a new scan only in the fixture; keep reviewing the earlier run and find active progress.
3. File review: find a large duplicate set, compare paths, select a copy without changing its decision,
   Keep one, mark another, inspect review totals, reset it and confirm the total changes.
4. Folder review: compare roots with different names but equal contents. Explain descendant scope and
   overlapping file decisions. Reveal only the intended fixture folder/page.
5. Rules: order preferred locations, preview the exact scope, apply once, make a manual override,
   reverse the application and confirm that override is preserved.
6. Validation: check the current plan, change it, recognize stale validation, then simulate a missing
   or changed copy. Find the reason and return to the affected set. Attempt an unsafe last-survivor
   decision and verify the existing protection prevents it.
7. Monitoring: inspect discovery, hashing and each folder substage with controlled fixtures; show
   unavailable ETA, cancellation acknowledgment, completion with warnings and interruption.
8. Warning/history: open current warnings and return to progress; open old warnings without changing
   the active run; compare qualified retained performance summaries.
9. Failure: inject a delayed page, query failure and worker exit; navigate during them. Explain what
   remains usable, retry the correct operation and verify no mixed-context result or false success.
10. Accessibility: repeat the key file-review path entirely by keyboard and with each supported screen
    reader; inspect long-path/narrow/theme/DPI states and focus restoration.
11. Long duration: inject more than 48 hours of elapsed time, a large-file read with an unchanged path,
    a stable-rate phase, unknown totals, absent updates and a real worker exit. Explain each state;
    details remain readable, focus stays put and lifecycle completion stops animation.
12. Rescan: complete a small fixture scan, restart the isolated worker with the same cache, then scan
    unchanged files, add a duplicate, remove a copy and change content. Verify fresh membership and
    qualified hit/read outcomes without editing the earlier run's results or carrying decisions.

For A17, preserve result history and the cache across runs: the existing engine deletion-rescan test
truncates the result database and does not establish this combined Windows workflow. Retain a known
fixture manifest and compare exact result membership with fresh revalidation
under identical parameters. Include missing roots, change-token invalidation with same size/preserved
modified time, conservative cache fallback, and cancellation followed by a new run. Reuse existing
signature tests for platform edge cases; add missing UI/worker integration coverage instead of
duplicating engine tests. An unchanged repeat may still perform discovery/analysis reads; do not
assert whole-scan zero I/O or guaranteed wall-time savings. Full coverage is pending implementation.

Record task completion, misinterpretations, assistance required and unexpected scope changes.
No pass if the user cannot distinguish selection, marked intent, validation, and execution status.
Completion time can guide iteration, but do not invent measured baselines or treat a fast click
through as a usability pass.

## Test layers and execution order

1. Planning checks now: local links, source mapping, consistent IDs/states, fictional prototype
   totals, navigation/decision/reset/filter behaviors, narrow layout, no runtime/API side effects.
2. Per implementation slice: focused Core tests for changed state behavior; loaded-STA WPF tests
   for the affected controls/layout/focus. Add regression tests only for real changed behavior.
3. Coherent integration boundaries: build paired Rust worker first, build/test Windows Debug and
   Release; run relevant Rust regressions when a worker/shared contract is touched.
4. Final integration: `cargo build --workspace`, `cargo test --workspace`, and Release equivalents;
   `dotnet build/test apps/windows/SuperDuper.Windows.sln` in Debug and Release. Preserve existing
   intentional operator-only skips as explicit unrun evidence.
5. Desktop acceptance: inspect each script before using `scripts/Invoke-WindowsSmoke.ps1` or
   `scripts/Verify-WindowsRelease.ps1`; establish its process, fixture and state scope. A script name
   is not blanket permission to stop the user's app or reuse a consumed campaign.
6. Existing scale guards: retain the published query/memory thresholds (including the historical
   warm query target of 100 ms and established large-result regression). Use fresh, separately scoped
   synthetic evidence only when the changed query/UI behavior warrants it. No full-drive campaign
   is needed to iterate on layout. Never rerun consumed SOP identities or rerun until favorable.

## Runtime isolation

The user's app/worker were open during discovery. Before any future build or runtime verification,
re-audit processes and choose outputs that cannot overwrite in-use artifacts. Prefer an isolated
fixture-backed WPF test host and separate result/status/cache/log paths. Use generated nonpersonal
files under a verified task-owned fixture root. Native provider/deletion/physical campaigns keep
their existing distinct authorization. Do not open production databases in a new writer, mutate
live review state for a design demo, stop the user's scan, or infer permission from the parked plan.

Physical keyboard/screen-reader/high-contrast/multi-monitor verification is still required for native
acceptance. The user can perform the necessary interactive steps when fixtures and procedure are
ready. Ask only for the exact missing authority or action at that boundary, after preparing it.

## Evidence record format

Each implementation gate records commit/build, fixture identity, commands and exit status, checks
passed/failed/skipped, capture dimensions/theme/DPI where applicable, observed defects and fixes,
remaining limitations, and the next gate. Retain failed evidence. Do not record personal file paths
or put raw production screenshots into the repository. Use `evidence/` for concise scoped records
once actual implementation validation starts.
