# Current redesign session checkpoint

Updated 2026-09-08. Replace current facts here after each coherent slice; retain detailed evidence
in linked records and history in Git. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserved baseline: `wpf-poc` at `deefa40`.
- Prior planning commit: `0827b28`. This documentation update records UIR-02 direction acceptance
  and the operator's long-scan/rescan feedback; resolve its commit from Git history.
- Completed: UIR-00 preservation, UIR-01 findings/specifications/concept, UIR-02 direction feedback.
  “This looks good” accepts the high-level direction; it is not a claim of native usability testing.
- Next: **UIR-03, local code**. First characterize selected-run versus active-run context and delayed
  optional loads in Shell tests, then introduce semantic navigation/shared shell resources while
  preserving the existing destinations. A01/A02/A09/A15 define acceptance; split into bounded slices.
- Product implementation: not started. No WPF/Rust source changed in planning.
- New requirements: [long scans and repeat scans](scan-and-rescan-experience.md), A16/A17. Prioritize
  UIR-04 immediately after the shell. Existing qualified persistent cache is reused, not rebuilt.
- Verification this update: source/contract inspection; 15 Markdown files, 44 local links, A01-A17
  IDs and next-gate/branch references checked; Git whitespace check passed.
  Product tests and native acceptance not run. Prior concept checks are in
  [prototype verification](discovery/prototype-verification.md); the concept's progress is illustrative.
- Open local blockers: none for UIR-03. Exact visual details are normal implementation decisions.
- Runtime: re-audit app/worker and any concept preview server before builds/launches. Last recorded
  product process IDs are historical, not current authority. Use isolated outputs, fixture roots,
  result/status/cache/log paths; do not stop the operator's application.
- Boundaries: production deletion disabled; no pause/resume or run-content diff; SOP10 complete and
  consumed; prior release validation parked. Physical/provider/performance authority stays separate.
- Session method: [guide](codex-session-guide.md) and [copyable kickoff](session-kickoff-prompt.md).
