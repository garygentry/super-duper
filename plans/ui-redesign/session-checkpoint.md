# Current redesign session checkpoint

Updated 2026-09-12. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; no switch/worktree/merge/push.
- Latest implementation: **UIR-04a, the commit containing this checkpoint**, based on shell acceptance
  `84a4f55` and implementation `5f83705`. UIR-00/01/02/03 complete; **UIR-04 in_progress**.
- [UIR-04a evidence](evidence/uir-04a-saved-scan-setup.md): Scan again opens current saved setup and
  retains drafts/history. Setup explains qualifying persistent reuse, fresh discovery and the
  candidate-only revalidation override. Start saves valid edits and creates a new dated run.
  Save/Discard/Stay protects navigation; pending Start locks edits and restores them after failure.
- Verification: full Core regression 177 passed; including 43 setup/Shell cases; three loaded-STA
  WPF methods passed, including new setup focus/layout/dirty-tab-binding checks. One isolated real-worker
  test completed four runs with persistent cache/history across worker restart, both hash-stage hits,
  edited exclusions/membership, retained old IDs/keep decision and explicit revalidation policy.
  Fictional fixture built under `artifacts/uir04a/fixture`. Captures/results under `artifacts/uir04a`.
- Retain UIR-03 scoped operator passes: nine walkthrough checks, scrollbar correction, Narrator,
  corrected Dark/Desert/live text size, 150%/175% display transitions with focus/selection retained.
  Prior paired Debug/Release integration and Rust 226 passed/10 ignored remain retained, not rerun.
- **Exact next slice: UIR-04b multi-day elapsed/update freshness and terminal activity (A08/A16).**
  Read startup/current-control documents, execution plan, selected S02 and scan-and-rescan contract,
  then directly linked progress projection/view model/views/tests. Use controlled clock, progress and
  lifecycle fixtures. Preserve exact diagnostics, honest denominators/ETA and coalesced announcements.
- Full A17 same-size/preserved-mtime changes, exact membership comparison, missing roots, fallback and
  interrupted reuse remain UIR-07/08. UI workflow acceptance is pending; UIR-04a is local implementation.
- NVDA and physical 200% remain `unrun_unavailable` for UIR-08/A11, not passed or waived. Do not
  troubleshoot Windows, force scaling or install software. Full native/user acceptance remains later.
- Runtime: PID 67748/session 1 was re-audited responsive at the exact UIR-03f fixture path and left
  untouched. The new UIR-04a fixture was built, not left running. Re-audit before reuse/launch and never
  overwrite running outputs. No production app/worker was observed; production state was untouched.
- Boundaries: production deletion disabled; engine/cache/protocol/query ceilings and survivor/revision/
  overlap protections unchanged. SOP10 complete and Windows release stream parked; no campaign authority.
- Update evidence, plan/checkpoint/handoff, review/test and commit each coherent slice. At handoff,
  print a complete copyable continuation prompt tailored to the actual commit and next slice.
