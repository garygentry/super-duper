# Polish checkpoint

Updated 2026-09-16 local time. Branch `codex/ui-redesign`; pre-review baseline `6206610`.

## Current state

- P00 complete. All interview recommendations accepted; implementation and subagents authorized.
- P01–P06 implemented and background-verified. P07 background matrix complete. P08 fresh-state
  Release journey passes; final native mouse/keyboard and physical high-contrast acceptance blocked.
  Do not claim full readiness or restart the completed redesign/release campaigns.
- Product changes through `e4578ef`: quiet responsive shell, shared icons/styles, remembered mode
  and disclosures, locations-first setup, concise progress, adaptive Files/Folders comparison,
  unified filters/units, checks-first Review, staged rules, list-first History and optional diagnostics.
- Engine freshness fix covers newer target/survivor/reconciliation/overflow evidence and operation
  prepare/confirm admission. History recorded-location XAML crash and narrow layout defects fixed.
- Final verification slice corrects two machine-speed-dependent Rust test assumptions; no production
  behavior changes. Heartbeat samples and valid intermediate coalesced frames remain permitted,
  while exact phase/counter/revision/order and bounded-frame checks remain enforced.

## Verified evidence

- Full Rust Debug and Release workspace tests pass, including final engine admission guards.
- Windows Debug and Release builds: zero warnings/errors. Core 224 passed in each configuration.
  Infrastructure 80 passed/five expected skips in each (Debug eligibility checked in normal VM).
- WPF regression: Debug 3/3 (44s); Release 3/3 (74s). Light/Dark, 100%/150% text, standard/narrow,
  disclosure/focus/scroll and monitoring states pass. Files gets 61.7% client height with an active
  scan; Folders also meets unchanged 60% requirement. Actual stressed captures inspected.
- Debug actual-file journey: `artifacts/ui-dev-session/polish-journey-2a06e3b5820646c680fd5e7b5a4aa411`.
- Final Release actual-file journey: `artifacts/ui-dev-session/polish-journey-fe4440ba6f8a4767a7031676cde91baa`,
  passed 1/1 in 23s. Fresh corpus `polish-data-4ca8ef90e57e4966ad44f21cd111c2be`: 671 real copies,
  mixed documents/media/archive with provenance. Rules preview/apply/reverse, disk preferences,
  restart, changed/locked/missing/new/overlap cases, cancellation and rescan all pass.
- Standalone and packaged Release worker SHA256 both
  `F90D8A78565489E1C4F3B61BD7C85E0FFFC89E174A4E809D3BD049DC4F78A115`.
- Final cleanup: no app/worker process or `.uidev` sidecar found. Sources preserved; runtime ignored.
- See `implementation-evidence.md` and `review.md` for evidence categories and finding dispositions.

## Exact next step

Native input partially succeeded on 2026-09-16: folder picker opened/Escape returned, real rescan
completed and navigated to Results, comparison/mark/copy icon and Explorer launch worked. Clipboard
contents and Explorer selection were not verified: Explorer capture hit app-approval timeout.
Subsequent app capture/discovery timed out; helper reset plus one fresh discovery also timed out.
Stop further native calls in that failed session. The app/worker were stopped and sidecar removed.
LogonUI in session 1 does NOT establish session 2 is locked; actual input worked in session 2.
Operator reiterated permission to continue without unlock requests; no renewed authorization needed.

Native pass found canonical local paths misclassified as unknown drives. A focused classifier fix
preserves stored paths, normalizes only DriveInfo lookup and recognizes extended UNC. Focused tests
passed 8/8 in Debug and Release; both full Windows builds have zero warnings/errors. Detailed
results are recorded in implementation evidence. P08 remains open for a fresh native helper session:
recheck Setup warning removal, complete folder selection, clipboard/reveal verification, Stop,
Keep/Reset, Folders, Review/check/rules confirmation/Escape, History/context return, keyboard/focus,
resize/enlarged text and physical high contrast. Use fresh list_apps selection when list_windows
omits the app. Do not invent handles. Fix/retest any finding, clean owned state and close P08 only
with actual native evidence. Background Debug/Release evidence above predates only this classifier fix.

Keep `codex/ui-redesign` and `wpf-poc`; no merge/push/release/deletion activation. Subagents exhausted
account usage during verification; root finished locally. Build serially, Cargo jobs 2, .NET shared
build servers disabled. Do not compile concurrently with UI journeys. No more product decisions pending.
