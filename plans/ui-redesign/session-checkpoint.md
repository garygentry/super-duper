# Current redesign session checkpoint

Updated 2026-09-08. [Execution plan](execution-plan.md) owns gate status.

- Required branch: `codex/ui-redesign`. Preserved baseline: `wpf-poc` at `deefa40`.
- Implementation baseline: `422c3e7`; this UIR-03a commit follows it. Resolve the completed slice
  from Git history rather than treating the baseline as a reset target.
- Completed gates: UIR-00/01/02. **UIR-03 remains in_progress**, with UIR-03a implemented.
- UIR-03a: semantic destinations replace numeric routing; shared shell/recovery resources;
  dated selected/monitoring identities; independent global progress/cancellation; lifecycle
  does not select another historical run; setup readiness; latest-history generation guards;
  demand-loaded Performance; redundant shell-owned pane loads removed.
- Verification: 158 Core tests and four WPF tests pass; Debug WPF build succeeds with zero
  warnings/errors. Shell-only captures at 1180x760 and 900x600 inspected. See
  [evidence](evidence/uir-03a-shell-context.md) for commands, initial failures and limitations.
- **Exact next slice: UIR-03b, local_code** — compose Scan / Results / Review / History; separate
  highlighted History row from workspace run with explicit Open scan; load file/folder/review
  only when opened, retaining bounded same-run state. Add delayed Open scan/current-warning/
  lifecycle tests and loaded WPF focus/navigation checks. Complete A01/A02/A09 evidence before
  closing UIR-03. UIR-04 monitoring/Scan again follows UIR-03.
- Limits: seven tabs remain; History selection still directly selects the workspace run.
  File/folder/review loads remain eager once selected (they no longer block setup). Full visual
  resource adoption, four-area layout, physical keyboard/screen-reader/theme/DPI and integrated
  Debug/Release Rust/.NET acceptance remain pending. No native acceptance claimed.
- Runtime: operator WPF PID 36316 and worker PID 17612 observed at start/end, running from
  `artifacts/windows-x64`. No stop, attachment, production database access or scan performed.
  Test outputs/captures/TRX are in ignored `artifacts/uir03a`; fixture host has closed.
- Boundaries: production deletion disabled; worker/cache/protocol/collection ceilings unchanged;
  long-scan and persistent qualified rescan A08/A16/A17 retained; no pause/resume or run-content
  diff; SOP10 consumed and complete, prior Windows release validation parked.
- Open local blockers: none. Follow [guide](codex-session-guide.md) and
  [kickoff prompt](session-kickoff-prompt.md), continuing in this checkout and branch.
