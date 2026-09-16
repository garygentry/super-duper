# Polish checkpoint

Updated 2026-09-15 local time. Branch `codex/ui-redesign`; pre-review baseline `6206610`.

- P00 complete; operator accepted all interview recommendations and explicitly authorized
  autonomous implementation with subagents. No design decisions remain pending.
- P01 real-worker WPF runner and P02–P06 product changes implemented; integration is active.
  See [implementation evidence](implementation-evidence.md) for exact scope and caught defects.
- Source work includes responsive shell, native shared styling/icons, persisted display settings,
  simplified setup/monitoring, file/folder filters and decisions, Review checks first, staged
  preferences, compact History/warnings/Performance and recovery details. Removal stays disabled.
- Real production worker/WPF baseline exercised >200 groups and members, file/folder decisions,
  preflight and restart. Latest extended runner adds real JSON preferences, rule application and
  reversal, separate-corpus changed/locked/missing/added files, overlapping roots and Stop/rescan.
  The extended run is not yet a passing acceptance result; wait for its final evidence.
- Actual narrow screenshots revealed a copies-grid sliver and clipped Folder filters. Responsive
  single-pane comparisons and consistent compact filters are implemented; verify selected-copy
  captures, not just the set list. History is list-first and Progress separates concise/technical ETA.
- Extended real journey passed disk preferences and rules preview/apply/reverse, then reproduced
  a real engine bug: changed-file validation invalidates a working decision, but older preflight
  still reports current because manual revision is unchanged. Engine agent owns a bounded history-
  intersection correction and regression tests. Root UI now distinguishes this stale reason.
- Rust Debug tests passed. Core Debug 223 passed; Infrastructure 79 plus the isolated read-only
  eligibility check passed, with five expected skips. Full WPF regression is being updated for
  intentional icon/staged-tab behavior; do not claim its final pass yet.
- Rust Release workspace tests passed after resuming with two jobs (45m59 including native build).
  This precedes the final operation-admission freshness guard; rerun affected tests after that fix.
  Matching Debug/Release final matrix remains pending. Disable .NET shared build servers and serialize
  builds: this resolved unreliable/stalled build-server retries and exposes compiler errors.
- Corpus: 671 actual document/media/archive copies with source hashes and ZIP-entry provenance.
  Worker state and mutations are private under ignored `artifacts/ui-dev-session` directories.
- Native Computer Use previously returned only Codex with LogonUI active. Native final pass is
  unrun, not waived; do not repeatedly probe input while locked or block independent work.
- Next: finish narrow comparison correction, definitive smoke/real-data run, inspect standard,
  narrow and theme captures, fix remaining findings, finish matrix, update issue closure ledger.
  Only after all independent work is complete, resolve/report the final native input prerequisite.
- Preserve `codex/ui-redesign` and `wpf-poc`; no merge/push/release/provider/deletion authority.
  UIR-00–09 remain complete; do not restart historical gates.
- Subagents hit the account usage limit during verification. Root continued locally. Latest engine
  source also invalidates affected watcher-overflow history and guards operation preparation and
  confirmation within their transactions. Seven targeted storage tests passed after these additions;
  the timestamp unit test passed before the operation guards (timestamp code unchanged).
- Core rerun includes deterministic elapsed clocks in two dispatcher-queue tests; real progress
  delivery scheduling and exact queue-count assertions remain unchanged. Stale-check UI covers
  uncertain filesystem changes without claiming that every case was a confirmed changed file.
- Latest Core Debug rerun passed 224/224. WPF retry reached a stale simultaneous-editor/preview
  layout assertion; it now verifies the accepted one-stage-at-a-time flow and is rerunning.

## Latest verified slice (after implementation commit `10fbc0f`)

- Full Debug Rust workspace passed with the freshness and operation-admission guard.
- Debug Windows solution built without warnings/errors. Core 224 passed; Infrastructure 79
  passed/five expected skips plus normal-context read-only eligibility 1/1 passed.
- Full WPF smoke matrix passed 3/3 in 44 seconds with actual Light/Dark, 100%/150% text,
  standard/narrow windows, disclosure/focus/scroll checks and monitoring states. Captures:
  `artifacts/ui-polish-verification/{layout-captures,progress-captures,shell-captures}`.
- Actual-file journey passed 1/1 in 73 seconds:
  `artifacts/ui-dev-session/polish-journey-2a06e3b5820646c680fd5e7b5a4aa411`.
  Includes Stop/cancel/rescan, changes/locks/missing/new/overlapping paths, original-state
  preservation, disk preferences/restart, and rule preview/apply/reverse.
- Layout regressions drove compact shell/banner margins and removal of a redundant healthy
  filter notice. Files comparison receives 61.7% of client height while another scan is active;
  Folder comparison also passes the unchanged 60% requirement. Narrow/enlarged decisions pass.
- Fixed a real History recorded-path XAML crash with an explicit one-way read-only binding.
  Test navigation now opens staged/technical disclosures before inspecting their contents.
- Rendering helpers use render-priority geometry sampling to avoid idle starvation; the expanded
  complete suite has a two-minute watchdog, retaining three-second layout-stability limits.
- Only the final small Review singular/plural/success-copy edit postdates the passing Debug
  journey; validate it in the matching Release matrix. Next: Release build/tests and journey,
  final fresh-state/native acceptance and issue-ledger closure. Native input remains unrun.
