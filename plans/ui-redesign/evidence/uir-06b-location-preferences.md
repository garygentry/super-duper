# UIR-06b focused Location preferences evidence

Date: 2026-09-13
Disposition: locally implemented; UIR-06 remains `in_progress` for later integrated/native acceptance

## Outcome

The existing preference-rule workflow now opens from a discoverable **Location preferences** panel
inside Review instead of occupying Files. One shared `PreferenceRulesViewModel` still owns saved
ordered roots, the current complete Files filter/selected-set scope, worker rule revisions, virtual
preview pages/signatures, applications and reversals. Review initializes that shared workflow and
synchronizes the worker-owned review revision even when the Files workspace has never been opened.

The focused panel separates three meanings in its headings and copy:

- **Saved preference** edits reusable ordered-root configuration only.
- **Virtual preview — not saved decisions** identifies the exact scope plus rule/review revisions;
  proposed outcomes remain virtual until the explicit confirmation.
- **Saved decisions from rule applications** shows application provenance and offers the exact
  operation **Reverse rule application**. Reversal states that later manual choices are preserved.

Application confirmation names the previewed scope, affected-set and affected-copy counts, proposed
Keep/Remove counts, blocked sets, physical items and bytes. Any later plan revision clears a stale
preview, so the worker signature/revision pair must be freshly obtained before Apply can enable.
Manual **Reset decision** remains the existing return-to-undecided action; no undo stack was added.

## Local acceptance coverage

- **A04:** saving, previewing, applying and reversing remain distinct; same-run navigation preserves
  unsaved rule edits, while a changed review revision invalidates only the stale virtual preview.
  Existing storage/protocol coverage still proves application provenance and reversal of only
  rule-produced decisions while later manual overrides survive.
- **A10:** Review supplies the current worker plan revision to preview/application. Preview requests
  retain rule revision, review revision and worker preview signature ownership; Review totals and
  validation remain the unchanged worker-owned UIR-06a surfaces.
- **A11 (local):** the focused panel is discoverable by keyboard/UI Automation, restores focus for
  both confirmations, uses a recycling virtualized preview list, disables horizontal scrolling and
  keeps editor, preview and reversal actions vertically reachable at supported fixture widths.
- **A15:** the copy consistently distinguishes configuration, virtual preview and saved review intent.
  Nothing validates or deletes files here, and Review still contains no execution action.

## Preserved contracts

- Preview pages remain 100 rows with a five-page cache; the rules list remains capped at 200.
  UIR-06a Files/Folders Review pages remain separate 200-row queries with independent five-page caches.
- UIR-05a/b/c filters, comparisons, decisions, navigation, reveal and focus behavior remain covered
  by the full Core/WPF passes. Current-filter and selected-set scopes still come from Files without
  loading a global plan or moving worker ownership into WPF.
- Apply still sends the worker-issued preview signature plus exact scope/rule/review revisions.
  Reverse still uses application identity and current expected review revision. The database,
  worker, protocol, survivor/overlap protections, manual provenance and Reset semantics did not change.
- `DisabledRecycleOperationCapabilityExecutor`, `CanSubmit=false`, `executorEnabled=false`, the
  absent execution action, production state, recovery-resolution boundary and all SOP limits remain.

## Verification and retained corrections

| Check | Result |
|---|---|
| `dotnet test ...SuperDuper.Windows.Core.Tests.csproj --artifacts-path artifacts/uir06b/core-accepted-final` | **216 passed**, `artifacts/uir06b/results/core-accepted-final.trx` |
| `dotnet test ...SuperDuper.Windows.Smoke.Tests.csproj --artifacts-path artifacts/uir06b/wpf-accepted-reviewed-final` | **3 passed**, `artifacts/uir06b/results/wpf-accepted-reviewed-final.trx` |
| `dotnet build ...SuperDuper.Windows.RedesignFixture.csproj --artifacts-path artifacts/uir06b/fixture-accepted-final` | passed, **0 warnings / 0 errors** |
| Production-lock source audit and `git diff --check` | passed before commit |

The 214-test Core baseline gained two focused contracts: Review can initialize preferences without
opening the Files workspace, and same-run composition plus worker revision synchronization preserves edits while
rejecting stale previews. The three loaded-STA WPF methods retain every prior Review/Results/theme/
text/focus check and now verify the new panel, exact reversal label, virtual-versus-saved headings,
recycling list and narrow reflow. There are 111 PNGs under
`artifacts/uir06b/captures-accepted-reviewed-final`, including explicit preview/application views at
1180x760, 900x600 and toolbar-stress minimum. Representative captures were visually reviewed.

The retained probe evidence includes an initial fixture assertion that expected the entire expanded
panel to fit one viewport; it was corrected to verify each essential control through the vertical
scroll path. A later Core pass caught the generated plural “copys”; the copy and regression were
corrected. Passing evidence uses the final isolated paths above.

## Runtime and remaining scope

PID 67748 was re-audited during final verification. It remains the responsive older UIR-03f fixture
at its exact `artifacts/uir03f/fixture` executable and was not reused, focused, closed or overwritten.
No other app/worker ran after test windows closed. UIR-06b outputs were built but not launched; no
production database, cache, log, scan, user file or execution state was touched.

UIR-06a/b are locally implemented and UIR-06 remains `in_progress` for later integrated/native
acceptance. The next dependency-ready local work is UIR-07a: compose the existing History/open-run
and contextual warning journey under S05/A07/A12/A15, preserving selected-versus-active run identity,
bounded warning pages and exact return focus. Performance detail, recovery resolution, execution,
full A17 and integrated/native/operator acceptance remain later.
