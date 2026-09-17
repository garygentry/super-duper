# Polish checkpoint

Updated 2026-09-17 local time. Branch `codex/ui-redesign`; pre-review baseline `6206610`.

## Current state

- P00 complete. All interview recommendations accepted; implementation and subagents authorized.
- P01–P06 implemented and background-verified. P07 background matrix complete. P08 fresh-state
  Release journey passes; physical 150% text/Desert and compact keyboard/focus acceptance pass.
  UX23's brief native loading frame remains unobserved; P08 stays open for that precise evidence.
  Do not claim full readiness or restart the completed redesign/release campaigns.
- Product changes through `e4578ef`: quiet responsive shell, shared icons/styles, remembered mode
  and disclosures, locations-first setup, concise progress, adaptive Files/Folders comparison,
  unified filters/units, checks-first Review, staged rules, list-first History and optional diagnostics.
- Engine freshness fix covers newer target/survivor/reconciliation/overflow evidence and operation
  prepare/confirm admission. History recorded-location XAML crash and narrow layout defects fixed.
- Final verification slice corrects two machine-speed-dependent Rust test assumptions; no production
  behavior changes. Heartbeat samples and valid intermediate coalesced frames remain permitted,
  while exact phase/counter/revision/order and bounded-frame checks remain enforced.

## Retained verification baseline (before the current P08 fixes)

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

## Retained P08 verification through 654648d (2026-09-16)

Current verification: focused Rust storage 4/4 plus boundary unit test 1/1 in both profiles;
full Windows Debug/Release builds have zero warnings/errors; Core 228/228 in each. Release WPF 3/3
passes. Fresh Debug real-worker journey passes 1/1 (25s); final Release journey passes 1/1 (21s) at
`artifacts/ui-dev-session/polish-journey-f97c5e0da8234bb38bf50bef4f2915f7` (includes UX23 code).
Core/WPF TRX files: `artifacts/ui-polish-verification/p08-fixes`. Release worker standalone/packaged
SHA256: `583C33CC6F18D26F1171A668AA89D8EB5525EB6935D1DA4701A70A0C97831883`.
The historical full Rust/Infrastructure baseline above is retained; it was not replayed this slice.

Resumed from `d784f48` plus preserved pending changes. Reviewed/fixed preference-root equivalence,
shared native confirmation Escape, inline Apply/Reverse Escape and focus. Independent review caught
and corrected repeated trailing separators on drive roots. ENG03, UX21 and UX22 native after-checks
pass; see implementation evidence. Native state was the fresh Debug journey
`artifacts/ui-dev-session/polish-journey-e4149564922d459cb1761b190b1d8a09`.

Native acceptance now includes folder selection, clipboard/reveal, manual Files/Folders decisions,
Review check, root ranking, apply/reverse/inline Escape, native Check Escape, History/context return,
Alt+S/H/O navigation, Alt+C cancellation and 900×600 resizing/rail collapse/usable Results.
Prior native successes are recorded in the continuation evidence and must not be replayed wholesale.
The 240 large disposable worker copies used to give native Stop enough time have been removed;
scans 7/8 in this state intentionally reference removed test copies. Use scan 4 or a fresh small
real-file journey for further Results checks; do not infer stale historical copies are still present.

A brief loading/empty message overlap during native historical Results opening exposed UX23:
IsDetailLoading did not notify the derived IsDetailEmpty binding. The narrow notification fix and
bounded delayed-response regression are included. New native sampling is recorded below.

## Latest P08 continuation from 654648d

Physical Windows 150% text and Desert were applied through native Settings and inspected in the
real-worker app. Standard enlarged Setup/History/Files and compact Files/Folders/Review pass:
meaningful rows, readable decision/scope, Back and primary actions remain reachable. Compact
Review Check comes into view on first Tab at 150%; confirmation Escape restores focus. OS appearance
was restored to 100% text/contrast None, verified in Settings and the running app.

UX24 exposed local focus loss after async Keep/Mark/Reset. Files and Folders now preserve a stable
focus anchor and restore the originating action only for unchanged context without stealing focus
after navigation. Independent review corrected the compact folder anchor and approved the final fix.
Native Files Keep/Mark/Tab and Folders Keep/Mark/Reset/Tab after-checks pass at 900×600/150%
(Folders in Desert).
Debug/Release Windows builds pass with zero warnings/errors; full WPF smoke passes 4/4 in each.
Fresh Release real-worker journey passes 1/1 (23s) at
`artifacts/ui-dev-session/polish-journey-c18581a7c6ff4e1d9d48bc8ebe65f02a`.
TRX evidence: `artifacts/ui-polish-verification/p08-focus`. Earlier failures are retained and
explained in implementation evidence; prior Rust/Core/Infrastructure baselines remain unchanged.

Standing computer-control approval for this dedicated VM is now explicit in AGENTS.md and the
UI development guide. The operator approved Settings after a tool timeout; fresh discovery and
activation worked. Do not ask for renewed authorization. Tool access must still actually succeed.
Both native app sessions are closed, their workers exited/stopped and the Debug sidecar removed.

## Exact next step

Latest attempt from `2f9679a` on 2026-09-17 regained native launch, discovery, capture and input
without recovery. Immediate captures after scan 4 historical opening and scan 6 next-member-page
loading showed settled members without overlap; the loading frame remained unobserved.
Sampling stopped after those bounded attempts. Owned app 5520/worker 2668 were path/parent-
verified; native close exited both, normal-context audit was empty and the Debug sidecar removed.
Scan 4/6 settled captures remain valid but do not establish the transient after-check.
P08 remains open solely for that observation; native availability succeeded in this attempt.
No source/build/test changes, corpus mutation, decision changes or accepted appearance/focus
rechecks. See the latest implementation evidence for final cleanup and closure assessment.
Native PNGs are saved under `artifacts/ui-polish-verification/ux23-2f9679a`.
Do not repeat helper troubleshooting or an indefinite settled-frame sampling loop.
Another identical settled capture does not advance acceptance; closure needs the native loading
frame itself or an explicit operator-agreed exclusion. No exclusion has been granted.

Complete UX23's precise native loading-state observation, then review P08 closure. Historical open
and the 208-copy set's next-page load were exercised without overlap in sampled settled captures,
but no transient loading frame was captured. Do not substitute the passing delayed-response test
for that observation or invent a pass. Do not replay completed appearance/focus/compact checks.
Use isolated real-file state (scan 4 for small Files, scan 6 for populated Files/Folders in the prior
Debug journey); scans 7/8 intentionally refer to removed Stop copies. Audit processes/sidecar first.

Keep `codex/ui-redesign` and preserve `origin/wpf-poc` at `deefa40`. No merge/push/release/deletion
activation. Build serially, Cargo jobs 2, .NET build servers disabled; no compilation during UI
journeys. Update findings/evidence/checkpoint/handoff, review and commit each coherent slice, and
print an updated continuation prompt if any work remains.
