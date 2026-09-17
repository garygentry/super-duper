# Initial end-to-end review

## Evidence and limits

Reviewed the current shell, every WPF view, feature commands and the existing capability map,
including setup, filters, file/folder decisions, validation, rules, history, warnings, performance,
worker recovery and operation/recovery evidence. Source baseline is `6206610`.

Fresh Debug worker/Windows build: success, zero Windows warnings/errors. Loaded-STA WPF smoke:
3 passed, 125 captures under `artifacts/polish-baseline-captures`. These render actual production
WPF views with fictional view-model data. They provide layout/state evidence, **not** a live
real-data UI walkthrough. Representative setup, progress, file comparison, folder empty,
Review, rule preview, History, warnings, populated folder comparison, Performance and enlarged Dark
screenshots were visually inspected.

Computer Use initialized through the bundled `@oai/sky` API. Its window list exposed only Codex;
the process audit found Windows `LogonUI` active. No native input was attempted through the lock
screen. Direct native real-app interaction remains unrun. Do not promise unattended native input
through Windows authentication; continue independent work and add real-worker-backed WPF automation.

`New-WindowsPolishCorpus.ps1` copied 87 actual tracked documents/assets into two locations: 174
manifested copies, 7,281,376 bytes, plus three real-document copies (one unique file and one
differently named duplicate pair). The library is small and single-volume, not a representative
large-drive or media-library performance campaign. It contains no fake worker results.

Corpus: `artifacts/ui-dev-session/polish-data-d3ecf6c8b9e54839b96646dba9afc04b`.
`Invoke-WindowsPolishBaseline.ps1` created private state at
`artifacts/ui-dev-session/polish-baseline-7c8647fd4d3a4c9dbe74604400f92433`.
Two real-worker scans completed; result queries found 88 file groups and 176 duplicate copies;
25-row paging reached a second page. `baseline.json`, `frames.json` and stderr retain evidence.
This baseline does not establish restart reuse, folder decisions, preflight or actual UI success.
The worker was closed through EOF. No production database or source file was modified.

## Feature inventory and proposed destination

| Capability | Current friction / review outcome | Proposed treatment |
|---|---|---|
| Create/select/refresh saved configurations | “Sessions”, “saved scans”, and “runs” require translation; fixed 250-DIP rail | Saved scans vocabulary; collapsible selector; refresh icon in overflow/toolbar |
| Name, choose folders/drives, enter/remove paths | Large form and instructions compete with folder selection | Locations first, sensible name, add-folder primary setup affordance; remove icon per row |
| Save/start, unsaved departure, delete saved history | Multiple Start actions; destructive history action beside Save | One contextual Start; retain save-before-start and Save/Discard/Stay; history deletion in menu |
| Exclusions, cloud detection, repeat-read policy | Healthy-policy explanations consume the default page | One concise policy summary; settings disclosure; detection failure remains prominent |
| Monitor/cancel, progress vs completed summary | Active banner, context and nested tabs repeat; technical phrasing | One scan status surface with clear phase, elapsed, freshness and Stop; completion offers Results |
| Repeat scan and historical snapshots | Start and Scan again are simultaneously presented | One context-sensitive rescan action; editing settings remains distinct; prior history preserved |
| File search/filter/sort, roots/drives and facets | Many advanced filters/pagers; draft vs applied state must remain truthful | Search and filter button with count/chips; coherent applied query and bounded facets retained |
| File comparison and Keep/Mark/Reset | Path prose, nested panes, status summaries and toolbars compete with copies | Compact comparable copy rows, explicit decisions, full path available, secondary action icons |
| Folder search/size/sort and exact relationship | Different size language (raw bytes), repeated exactness explanation | Same filter vocabulary/units as Files; concise relationship and descendant-scope disclosure |
| Copy path, Explorer reveal and next set/copy | Five text actions plus multiple pagers crowd detail | Familiar secondary icons/tooltips; retain labeled Keep/Mark and page scope |
| Filter totals, potential savings, combined plan totals | Savings and worker/internal totals have lengthy explanations | Clearly separate potential from marked savings; overlap-aware total authoritative |
| Page validation, dirty roots/reconciliation | Files says “Validate page”; Review explains “Check these copies” | Consistent “Check these copies” with explicit page scope; dirty-state action only when relevant |
| Review and whole-plan preflight | Check action below overview and tall paired lists | Summary + primary check first; readable readiness; compact Files/Folders selector |
| Preference rules, order, preview, apply/reverse | Long inline editor with save/preview/application/reversal all exposed | Focused staged panel: preferred locations → preview → apply; existing exact confirmations retained |
| History/open run and recorded settings | Highlight/open distinctions explained in multiple headers | Timestamp/status list with explicit Open; details attached to selection; separate active identity |
| Warnings, examples, result links and cancellation | Developer log block precedes actionable warnings | Plain-language issue/count/action first; codes, revisions and logs behind Details |
| Performance snapshots, phase/device/comparison | Identity and contract paragraphs dominate narrow screen | Contextual diagnostics page; compact summary, details and qualified comparison |
| Startup, unavailable worker, reconnect | Executable path and diagnostic recovery mixed with user guidance | Actionable error + Retry; technical paths in expandable details |
| Operation outcomes, recovery observations/corrections | Foundation/boundary language in ordinary Review | Contextual evidence only when present; concise removal-unavailable notice; preserve recovery truth |
| Keyboard, focus, themes, scaling, empty/error/loading | Existing tests cover reachability, not comfortable task completion | Visual occupancy and journey budgets alongside functional/accessibility regression |

## Prioritized findings

| ID | Priority | Evidence | Change / verification |
|---|---|---|---|
| UX01 | High | MainWindow: fixed sidebar, title, active banner, two tab rows, engine footer; setup/history captures | Reclaim content area; one navigation hierarchy; measure usable height |
| UX02 | High | MainWindow Start + Scan again; Setup also Start; resource primary style changes size only | One obvious primary per state; accent/style hierarchy; avoid duplicated actions |
| UX03 | High | Default Review screenshot: check action below viewport; paired list structure in PreflightView | Move check/readiness above detail; action visible without scrolling |
| UX04 | High | Dark 150%-text Files at 900×600: chrome/query consumes almost all visible workspace | Collapse sidebar, reflow query, preserve visible result and actionable comparison |
| UX05 | High | File selected-copy screenshot: copy list squeezed between headings, decisions and pagers | Allocate measurable comparison space; reduce repeated path/status text |
| UX06 | Medium | Review literally says “separate bounded worker query”; footer exposes protocol/engine versions | Remove implementation detail from routine flow; accessible technical disclosure |
| UX07 | Medium | SessionList/EmptyState vs setup/history use different nouns | Vocabulary table and screen-reader wording sweep |
| UX08 | Medium | Files “Validate page” vs Review “Check these copies”; folder min-size bytes | Consistent validation labels and binary-unit editor without changing semantics |
| UX09 | Medium | LocationPreferencesView long single StackPanel; 900×600 preview render | Focused staged editor, visible stage/next action, stable scope and reversal |
| UX10 | Medium | Performance 900×600 heading/identity/contracts occupy first viewport | Short identity; diagnostic details collapsed; summary visible on entry |
| UX11 | Medium | Warning view puts log paths/explanation above issue table | Issues first; actionable wording; preserve codes/examples/count truth in details |
| UX12 | Medium | Five full-text copy actions and repeated Previous/Next/Refresh | Consistent vector/glyph icons for secondary actions, names/tooltips/focus retained |
| UX13 | Medium | Setup cloud error hard-coded color, Review hard-coded 18-point summaries | Semantic brushes/type tokens across themes and text scaling |
| UX14 | High | VM lock prevents native exercise; existing rendered suite uses fake services | Real-worker-backed loaded WPF journey runner and separate native evidence ledger |

These are design findings and source/visual observations, not newly proven engine failures.
The small real-worker baseline surfaced no engine defect. Engine coverage remains to be expanded
through the planned real-data journeys; fix reproducible failures with regression evidence.

## What the next review must resolve

Real-data UI walkthrough of all common tasks; actual populated folder comparison; preferences
preview/apply/reverse; stale validation after real filesystem edits; restart/cache/history;
long-running monitoring/cancel; empty, denied/locked/missing files; real paging over 200 groups;
worker recovery; keyboard/tooltip/icon accessibility. Do not label this initial review a complete
native acceptance of every feature. P01 closes that evidence gap before broad visual implementation.

## Findings from implementation journeys

| ID | Priority | Reproduction / evidence | Owner and status |
|---|---|---|---|
| ENG01 | High | Complete a plan check, change a marked copy, run Files validation: copy becomes changed/decision invalidated, but latest preflight still says current with unchanged manual revision. Real journey `1e421f2a889f49be9f7e406f89bcf3b2`. | P07: bounded immutable-history freshness fix and actual-file regressions under verification. |
| UX15 | High | At 900×600 Folder header's fixed 140-DIP scroll spends its viewport on explanation, clipping search/filter controls. | P04: corrected toolbar and selected-copy controls inspected in real-worker journey `2181f77bccde46c89880b4726590eb9b`, `05-folder-decisions-narrow.png`; enlarged-text regression still pending. |
| UX16 | Medium | Full roots in copy rows share a long prefix and trim away their distinguishing names. | P04: shortest unambiguous suffix from immutable run roots; full paths retained; focused label checks and new captures pending. |
| UX17 | Medium | Detail disclosure does not persist: a window-level Loaded handler misses descendant direct Loaded events. | P02: logical-tree tracking; actual JSON/disk restart journey passed for Setup and inactive History sections. |
| UX18 | Medium | Selected tab bolds all page content through inherited FontWeight. | P02: selected underline retained without inherited content weight; corrected real Setup/Results captures inspected. |
| UX19 | Medium | History repeats instructions above the list; Progress exposes ETA mechanics in the normal view. | P03/P06: list-first History and qualified concise estimate, detailed reason in Diagnostics; new captures pending. |
| QA01 | Verification gap | Restarted History PNG has an implausibly narrow first column before deferred layout settles. | Closed: application-idle capture and useful-column assertion passed in real-worker journey `2181f77bccde46c89880b4726590eb9b`; restarted History PNG inspected, full row readable. |
| UX20 | High | Expanding History's recorded locations instantiates a read-only TextBox with an implicit two-way whole-object binding and throws a XAML exception. | P07: explicitly one-way binding; populated regression now expands and checks the actual details. |

The full viewport check initially measured Files at 53.6% of client height while another scan was
active. Compact shell/banner spacing and hiding the redundant healthy filter notice raised it to
61.7%; loading, pending-filter and error notices remain visible. Folder spacing follows the same
16-DIP bottom inset. The 60% requirement remains unchanged.

The accepted plan permits independent tooling/foundations to proceed concurrently. Gate completion
still requires matching evidence. See [implementation evidence](implementation-evidence.md) and the
compact checkpoint for current verification; these findings are not silently considered closed.

## Verification disposition after `e4578ef`

Evidence labels: **WPF** is the passing production-view rendering/control regression under
`artifacts/ui-polish-verification` (mocked worker timing/data where documented); **Real** is
the passing production-worker journey `2a06e3b5820646c680fd5e7b5a4aa411` using actual file copies.
Neither label means native mouse/keyboard acceptance. Final Release Rust/Windows matrix and fresh-state Real journey `fe4440ba6f8a4767a7031676cde91baa` pass; see implementation evidence.

| Findings | Verified correction | Disposition |
|---|---|---|
| UX01, UX02 | Compact shell, selector, contextual primary action; Files comparison 61.7% client height with active scan; Folders also meets 60%. WPF standard/narrow matrix and Real captures. | Implemented and background-verified |
| UX03 | Marked totals, readiness and Check precede detail. Standard entry capture shows Check; narrow local scroll reaches it. WPF and Real checks. | Implemented and background-verified |
| UX04, UX05 | Adaptive one-pane comparison, selected-copy actions, Back, useful rows at 900×600/150% text. WPF Light/Dark and Real selected-copy captures. | Implemented and background-verified |
| UX06–UX08 | Technical disclosures, consistent saved-scan/check vocabulary and binary-unit filters. Core query tests; WPF explicitly opens technical details. | Implemented and background-verified |
| UX09 | One preference stage at a time, confirmation stays visible; Real ordered roots, preview, apply and reverse. | Implemented and background-verified |
| UX10, UX11 | Performance context, issues-first warnings and optional diagnostics; WPF bounded rows, exact-context return, reachability and focus. | Implemented and background-verified |
| UX12, UX13 | Native vector actions, accessible names/tooltips, focus and semantic theme resources; WPF Light/Dark and enlarged text pass. | Implemented; native/high-contrast physical check still pending |
| UX14 | Private production-worker runner, hashed mixed real corpus, mutation isolation and owned-process cleanup pass. Native input partially passed on 2026-09-16, then capture/discovery failed after helper recovery; LogonUI in another session is not a lock signal. | Background gap closed; native acceptance blocked |
| UX15, UX16 | Persistent Folder toolbar; distinct file-root labels; narrow selected-copy screenshots and Core root-label regression. | Implemented and background-verified |
| UX17, UX18 | Persist active/inactive disclosures and last mode through real app/worker restart; selected-tab underline without inherited bold content. | Verified |
| UX19 | History list-first layout and concise, qualified ETA; exact ETA remains tested inside Diagnostics. | Verified |
| UX20, QA01 | Recorded-path binding opens without XAML exception; restarted History captures after layout show a readable full-width row. | Verified |
| ENG01 | History-based freshness covers targets/survivors/overflow and operation admission; seven targeted storage tests, full Debug Rust suite and Real changed/locked-file checks pass. | Fixed; final Debug/Release and real-file verification pass |

## Native follow-up finding

Latest disposition (2026-09-16): the historical table above retains its original evidence stage.
UX12/UX13 physical 150%/Desert and keyboard/focus acceptance now pass; UX14's native journeys
are retained as accepted. Only UX23's transient loading-frame observation remains outstanding.
The continuation from `b0241a2` hit native launch and fresh app-discovery timeouts; no frame
was obtained. Native calls stopped without helper troubleshooting. Earlier scan 4 opening and
scan 6 paging captures remain settled-only evidence. P08 remains open for the brief loading
frame; this attempt adds a tool-availability failure, not a product defect or permission gap.
See the latest implementation evidence for cleanup and the precise closure assessment.

| ID | Priority | Reproduction | Disposition |
|---|---|---|---|
| ENG02 | Medium | Native Setup reopened canonical local roots as unknown filesystem type, filling the common screen with repeated technical warnings. | Closed: classifier correction, local/UNC regressions and native after-check from `adeb9f5` passed; see implementation evidence. |
| UX21 | Medium | Native Review: Escape leaves the Yes/No “Check marked copies?” dialog open. | Closed: Yes/No/Cancel retains default No and affirmative-only admission; native Escape dismissed Check and restored focus without starting a check. |
| ENG03 | High | Preferred roots entered in ordinary DOS spelling did not match canonical extended run roots, so ranking could choose the backup or report a preferred location missing. | Closed: normalized DOS/UNC comparison preserves spelling and first legacy rank. Debug/Release storage tests and fresh real-worker/native ordinary-root ranking, apply and reverse pass. |
| UX22 | Medium | Inline Apply/Reverse confirmations did not handle Escape. | Closed: routed Escape uses existing cancellation/focus restoration. Real-worker regression and native Apply/Reverse Escape preserve decisions/application and restore focus. |
| UX23 | Medium | Native 900×600 History Open briefly overlaid “No copies to display” on “Loading group members”. | Fixed with delayed-response regression. Native historical/page-load after-checks show no overlap in sampled settled frames; the brief loading frame remains unobserved. Keep that precise native evidence open. |
| UX24 | Medium | At native 900×600 selected-copy detail, keyboard Keep records the decision but loses local focus; next Tab starts again at Files. | Fixed: retain a stable focus anchor during async command execution, restore the originating action only in unchanged context. Delayed/synchronous/navigation regressions and native Files/Folders decision/Tab after-checks at 150% text (Folders also Desert) pass. |
