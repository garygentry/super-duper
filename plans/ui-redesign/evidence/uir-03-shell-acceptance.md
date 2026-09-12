# UIR-03 scoped shell acceptance

2026-09-12. Implementation: `5f83705` (UIR-03a through UIR-03f). **UIR-03 complete;
UIR-04 ready, not started.** This closes the shell implementation gate, not the redesign or final
native accessibility/release acceptance.

## Final operator confirmation

The operator replied **"yes"** to "Did the same file group also stay selected after moving between
monitors?" Record file-group selection retention as passed, alongside the preceding **"focus
remained"** confirmation. This completes the available scoped monitor-move observations at 150%
and 175%; Windows did not offer 200%. No new defect is reported.

## Gate assessment

| Shell requirement | Evidence / outcome |
|---|---|
| A01 selected versus active run context, harmless History highlight and explicit Open scan | Core/loaded-shell tests and operator passes |
| A02 delayed optional-pane responsiveness, scoped error and same-run reuse | Controlled pending/failing fixture plus operator pass |
| A09 shared controls, reachable content and scrollbar clearance at both supported sizes | Populated WPF geometry checks, nine scoped operator checks and corrected-scrollbar acceptance |
| Corrected Dark, Desert empty-folder layout and live Windows text-size behavior | Operator's three rechecks passed on UIR-03f |
| Available keyboard, Narrator and monitor-move checks | Operator passes; 100% baseline and 150%/175% transitions without noticeable clipping, focus and selection retained |
| Paired worker and Windows Debug/Release integration | Retained UIR-03f passes: 170 Core, 75 Infrastructure with five operator-only skips, three WPF methods each; original shell assertions share the themed STA |

The [walkthrough](uir-03-desktop-walkthrough.md) retains exact reports and their scope.
[UIR-03f](uir-03f-theme-text-and-empty-state.md) retains fixes, failed iterations, final automation
and fixture launch details. No build/test rerun is needed for this documentation-only acceptance.
Documentation verification: 55 local link targets, final gate/evidence review and `git diff --check`
passed. No application code, OS settings or runtime state changed.

## Explicit requirements carried to final integration

| Requirement | Current state | Remaining owner |
|---|---|---|
| NVDA physical walkthrough | `unrun_unavailable`: not installed; no installation requested | UIR-08 / A11 |
| 200% physical display-DPI case | `unrun_unavailable`: Windows did not offer it; 175% does not replace it | UIR-08 / A11 |
| Full redesigned-screen reader/theme/DPI matrix and real long-scan/rescan acceptance | Not completed by this fictional shell walkthrough | UIR-04 through UIR-08, final user acceptance UIR-09 |

These are not passes or waivers. They remain required before claiming the corresponding complete
native acceptance, and require actual evidence or an explicit later disposition. Do not force
custom scaling, troubleshoot the operator's Windows configuration or install NVDA under this
assessment. The operator explicitly excluded Windows scaling troubleshooting.

The gate distinction follows the existing [acceptance matrix](../validation.md): A01/A02 belong
to UIR-03, A09 spans UIR-03/08, and the complete A11 matrix belongs to UIR-08. The
[decision record](../decisions.md) already assigns full native/user acceptance to UIR-08/09.
Earlier in-progress notes conservatively held UIR-03 pending this assessment; this record now
separates the completed scoped shell evidence from those still-unrun final requirements. It does
not redefine an unavailable test as successful or remove it from the roadmap.

## Next bounded slice

UIR-04a: saved-scan setup and explicit **Scan again**, following
[the repeat-scan contract](../scan-and-rescan-experience.md). Open the current saved setup, explain
qualified persisted hash reuse versus re-reading candidate content, save valid edits on Start and
create a new dated run without changing the previously opened run's immutable results. Preserve
dirty-edit decisions, single-active-run enforcement and current worker/cache boundaries. Scope:
A06/A14/A17; subsequent UIR-04 slices implement long-scan monitoring/terminal A08/A16 behavior.

Read the selected screen specification and directly linked code/tests before implementation.
Use isolated outputs and fake-service tests; any real-worker rescan verification uses a small,
separately isolated fixture with persistent cache/history, never production state or SOP campaigns.
UIR-04 is ready but no UIR-04 code/build/runtime work was started by this acceptance update.
Keep `codex/ui-redesign`, preserve `wpf-poc` at `deefa40`, and keep production deletion disabled.
