# Current redesign session checkpoint

Updated 2026-09-13. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: **UIR-07a, the commit containing this checkpoint**, based on `9b81c92`
  (UIR-06b). UIR-00/01/02/03 complete; **UIR-04, UIR-05, UIR-06 and UIR-07 in_progress**.
  UIR-04a/b/c, UIR-05a/b/c, UIR-06a/b and UIR-07a are locally implemented.
- [UIR-07a evidence](evidence/uir-07a-history-warnings.md): History now binds one explicit newest-first
  page of at most 500 runs, with Previous/Next paging, retained accepted data on failure and deterministic
  row focus. Highlighting only inspects; **Open scan** still changes the workspace. The card names exact
  scan/date/state, immutable recorded settings, any different opened workspace run and any active run.
- Contextual warnings retain exact run/date/state, 25-row pages, the five-page cache, at most three
  examples and worker-owned current/terminal revision identity. Active refresh replaces rather than mixes
  revisions and retains accepted data on failure. Close returns to the exact History, Progress or Summary
  origin; stable hash-warning navigation still opens the immutable completed run and restores focus.
- Verification: full Core **218 passed**; **three loaded-STA WPF methods passed**; isolated fixture build
  has zero warnings/errors. There are 114 visually reviewed captures under
  `artifacts/uir07a/captures-accepted-final`; final TRX files are under `artifacts/uir07a/results` and
  `artifacts/uir07a/results-accepted`. Retained probes include the corrected XAML binding/assertions and
  one sandbox-only Windows SDK read denial before the identical isolated WPF run passed with SDK access.
- **Exact next local slice: UIR-07b contextual Performance detail.** Read the Performance part of S05,
  capability/validation guidance and directly linked Performance, lazy-load, run-context, return-focus and
  WPF bounds tests. Preserve unavailable values, comparison qualifiers, exact selected run, 25 history
  rows, six phase rows, 64 device rows and no raw sample/time-series claim. Preserve every UIR-07a History/
  warning identity, paging, revision and return-focus contract. Do not pull recovery resolution, execution,
  full A17 or UIR-08 integration forward.
- Retain UIR-06a worker-owned combined totals, separate 200-row Files/Folders pages, independent five-page
  caches, exact links and validation states. **Check marked copies** remains whole-plan; **Check these
  copies** remains visible-page. Retain UIR-06b ordered roots, virtual preview scope/revision/signature,
  confirmation/provenance, fresh-preview enforcement, **Reverse rule application**, later manual overrides
  and Reset semantics. Retain all UIR-04/05 monitoring, rescan, query, comparison, decision, reveal and focus.
- Retain UIR-03 scoped acceptance, Narrator, Dark/Desert/live text size, 150%/175% transition/focus/selection
  and prior Debug/Release/Rust evidence. NVDA and physical 200% remain `unrun_unavailable` for UIR-08/A11;
  do not troubleshoot Windows, force scaling or install software. Full A17/native/operator acceptance remains later.
- Runtime: PID 67748/session 1 was re-audited responsive at the exact older UIR-03f fixture executable and
  left untouched. UIR-07a outputs are isolated and not running; test windows closed. No production app,
  worker, database, cache, log, user file or state was touched.
- Boundaries: production deletion disabled; `DisabledRecycleOperationCapabilityExecutor`, `CanSubmit=false`,
  `executorEnabled=false`, absent execution action, recovery/SOP boundaries and all safety locks unchanged.
  Update evidence/plan/checkpoint/kickoff/handoff, review/test and commit each coherent slice; print a complete
  updated copyable continuation prompt at every handoff.
