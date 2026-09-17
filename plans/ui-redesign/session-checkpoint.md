# Current redesign session checkpoint

Final follow-on status, 2026-09-16 local / 2026-09-17 UTC: the separately requested
[polish stream](../ui-polish/session-checkpoint.md) is complete through P08 after native UX23
loading acceptance from `ecd8bc7`. No UI work remains. UIR-00–09 and all preservation/release
boundaries remain unchanged; the older checkpoint below records redesign verification.

Updated 2026-09-15. [Execution plan](execution-plan.md) owns gate status.

New operator-directed work on 2026-09-15 is owned by
[`plans/ui-polish`](../ui-polish/README.md). Its review/plan and initial decisions supersede the
old 'no next action' direction for new UI work. UIR-00–09 remain complete; do not reopen them.

- The operator now explicitly requests autonomous background work even when the remote VM session is
  locked. The loaded-STA WPF smoke suite passed all three methods in that state, produced 125 PNGs,
  and narrow Results/Review captures were inspected. The complete Debug Windows suite also passed:
  220 Core, 76 Infrastructure/five expected skips, three WPF. `AGENTS.md`, the session guide and
  `docs/windows-ui-dev-session.md` now route agents to keep coding, testing, driving the in-process
  fixture and inspecting renders without waiting for native desktop input. This is background UI
  iteration evidence, not physical native acceptance. See [VM background evidence](evidence/vm-background-ui-iteration.md).
- Current VM revalidation: checkout is clean on `codex/ui-redesign` before this slice; Rust 1.98.1,
  .NET 10.0.401/Desktop 10.0.12, `cargo test --workspace --locked`, Debug/Release Windows builds,
  and the fictional fixture build pass. The Debug Windows suite passes 220 Core, 76 Infrastructure
  (five expected skips), and three loaded WPF methods after stabilizing the Review preference scroll
  assertion. The one Recycle Bin eligibility test fails inside the filesystem sandbox but passes in
  the VM's normal development context; it does not execute deletion.
- Desktop control directly opened Review and switched the in-memory fixture to 900 × 600. A shell-
  launched worker-backed Debug app was visible but not input-accessible across the privilege context.
  `Start-WindowsUiDev.ps1 -PrepareControlLaunch -CreateFixture` now prepares a validated Debug-only
  sidecar for Computer Use to launch the existing app with private state. The prepared five-file state
  is `artifacts/ui-dev-session/4ee7c8b6781b49439949e6d6b72b2693`; no app/worker remains open.
  The temporary `.uidev` sidecar was removed before handoff. **Native real-app control is not yet
  revalidated:** Computer Use reported `GetCursorPos` access denied even for the fictional fixture,
  and Windows `LogonUI` was active. Continue authorized background work without waiting for unlock.
  If a later selected gate requires native input, reprepare the saved isolated state with `-SkipBuild`,
  launch `CONTROL_APP` when interactive access returns, inspect the private scan/Results/Review/History
  journey, close normally and remove the sidecar. Do not claim live native control while it is unrun.
- Required branch: `codex/ui-redesign`. Preserve `wpf-poc` at `deefa40`; remain on this branch without further branch changes, merge or push.
- Dedicated Windows 11 VM setup is complete: Rust stable MSVC, Visual Studio C++/Clang, and .NET 10
  are available. The lockfile selects `time` 0.3.36/`time-macros` 0.2.18 for current Rust.
  Debug Rust and Windows builds/tests pass, including three loaded WPF smoke methods. This
  environment work does not reopen UIR-00 through UIR-09 or authorize another roadmap stream.
- Latest slice: **dedicated Windows VM UI-control setup**. The isolated fixture and real Debug app
  are controllable through desktop screenshots, accessibility IDs and direct input. A disposable
  five-file scan produced two file sets and one exact-folder set; Results, Review, History and
  same-state restart passed. `Start-WindowsUiDev.ps1 -CreateFixture` rebuilds/launches with unique
  ignored state; all test app/worker/fixture processes were closed. The prior VM toolchain setup is
  `dfe75e5`; prior redesign delivery verification is `885675e`.
- **UIR-00 through UIR-09 are complete.** The operator explicitly stated “I accept” after review of
  the UIR-08 evidence found no open critical redesign defect. See
  [UIR-09 evidence](evidence/uir-09-final-acceptance.md).
- Accepted scope retains saved setup/Scan again and qualified reuse, honest long-scan state, bounded
  file/folder queries and decisions, worker-owned Review totals/revisions, Location preference
  provenance/reversal, highlighted/opened/active History identity, and exact contextual warning/
  Performance return focus.
- The UIR-08 automated baseline remains Rust Debug/Release 226 passed/10 ignored and Windows Debug/
  Release 220 Core, 76 Infrastructure and three loaded-STA WPF methods, with five explicit skips and
  191 captures per configuration. The corrected fixture build and fresh focused rerun passed.
- Fresh delivery verification builds the Rust workspace and Debug Windows solution with zero Windows
  warnings/errors. The repaired file-grid focus assertion uses the production asynchronous retry path,
  passes three consecutive focused runs, and the complete Debug suite passes 220 Core, 76 Infrastructure
  and three WPF methods with the same five explicit skips. Product behavior is unchanged.
- The available 1180x760 and 900x600 keyboard journeys passed after fixture-only file/folder decision,
  folder-data and Review-folder-link corrections. Narrator, Dark/available contrast/150%-text and the
  available 150%/175% monitor transitions passed; settings were restored.
- NVDA and physical 200% remain `unrun_unavailable`, not passed or waived. No shipping WPF smoke,
  real long-duration/full-drive, provider, representative-performance, recovery-resolution, deletion
  or production-state campaign ran for UIR-09.
- Production safety remains unchanged: `DisabledRecycleOperationCapabilityExecutor`, `CanSubmit=false`,
  `executorEnabled=false`, no production execution action, and all recovery/SOP boundaries remain.
- PID 15072 was re-audited read-only during finalization and remained responsive at the exact corrected
  fixture path. Its title/handle were hidden by the known cross-context limitation. It was not reused,
  stopped or modified; no worker or production app/state was touched.
- **Exact next action: none inside the redesign stream.** Remain on `codex/ui-redesign` and await
  explicit operator direction before merge, push, branch changes, release-validation resumption,
  physical/provider/performance campaigns, recovery work or production execution.
