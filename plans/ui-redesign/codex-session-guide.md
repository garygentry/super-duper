# Executing the redesign across Codex sessions

This is the recommended project procedure, not a requirement to keep a single conversation alive.
Use the same local checkout and `codex/ui-redesign` throughout. One task should own edits at a time.
When starting a fresh Codex task, explicitly choose this saved project/local checkout and use the
[kickoff prompt](session-kickoff-prompt.md); do not create another worktree/branch for each session.
No new tasks, automation or background implementation were started by this planning update.

## Durable context

| File | Responsibility |
|---|---|
| Root `AGENTS.md` | Startup route, branch rule, architecture and inherited boundaries |
| `docs/windows-roadmap-session-handoff.md` current-control sections | Cross-stream scheduling and authority; skip historical narratives unless directly cited |
| [Execution plan](execution-plan.md) | Authoritative gate status, dependencies and completion criteria |
| [Session checkpoint](session-checkpoint.md) | Small replace-in-place summary of the latest work, exact next slice, blockers and verification |
| [Decisions](decisions.md) | Accepted direction, constraints and material design changes |
| Screen/design/scan specifications and [validation](validation.md) | Behavior to implement and evidence required |
| `evidence/` records plus Git commits | Per-slice commands/results, captures, defects and limitations once implementation begins |

Keep the checkpoint around one page. Link evidence instead of pasting logs. At context compaction
or a task change, the next agent should recover from these files and Git, not reconstruct months of
chat or re-audit completed engine work. OpenAI documents that Codex reads project `AGENTS.md` at
startup; using it to point to a concise project handoff is this project's recommended application
of that capability. [Official AGENTS.md guidance](https://learn.chatgpt.com/docs/agent-configuration/agents-md)

## Session procedure

1. Audit branch, status, recent commits and the latest relevant diff. Read the startup documents
   and checkpoint. If branch or unexpected edits disagree, establish what changed before editing;
   preserve the work. An old checkpoint hash is a verification anchor, not a reset target.
2. Select one dependency-ready user journey from the gate ledger. A gate may take several sessions;
   choose a bounded sub-slice with an observable outcome and named A-criteria. Avoid a file-by-file
   rewrite or combining all screens into one unreviewable change.
3. Resolve that slice's layout/state details, using the concept as a discussion aid and the Markdown
   specification as authority. State a short plan and implement the authorized local work. Do not
   repeatedly request approval for routine layout, test or refactoring choices within the scope.
4. Verify changed behavior with focused tests and fixture-backed native inspection. For monitoring,
   use controlled progress/clock sequences to exercise multi-day duration and long no-progress
   intervals quickly; a test need not run for days. For reuse, use disposable files and stable
   isolated cache paths across repeated runs and process restarts. Preserve the production app.
5. Review the diff and evidence. Update the gate ledger and compact checkpoint, record only material
   decisions, and synchronize the shared handoff. Commit each coherent completed in-scope slice on
   `codex/ui-redesign`. Record partial gate progress explicitly; a commit does not imply acceptance.
6. Continue in the same task if useful, or start a fresh task with the same kickoff prompt after a
   coherent checkpoint. Before an intentional stop, record incomplete work, exact failing command,
   next action and live test processes. Never mark a gate passed to simplify a handoff.
7. At every session handoff with remaining work, print a complete, copyable continuation prompt
   in a fenced `text` block in the final response. A link to the checkpoint alone is insufficient.
   Tailor the [reusable kickoff](session-kickoff-prompt.md) to the final committed state: include
   checkout/branch and preservation rules, latest implementation commit, startup reading route,
   exact next bounded slice and acceptance criteria, verification/runtime isolation requirements,
   unresolved limitations or blockers, and the update/review/commit/report obligations. Require
   the following session to print its own updated continuation prompt. Do not start another task
   or perform the next slice merely because the user asks to print this prompt. If implementation
   is blocked, name the precise prerequisite instead of inventing further work.

## Recommended sequence

- **Complete: UIR-03** — scoped shell acceptance recorded in `evidence/uir-03-shell-acceptance.md`;
  native NVDA/200% requirements remain UIR-08. Shell, semantic navigation and selected/active run
  context are verified with delayed-response fixtures and scoped operator evidence.
- **Next: UIR-04a** — saved-scan setup and Scan again (A06/A14/A17), then UIR-04 long-scan monitoring and terminal states. Make this the first full
  workflow after the shell because scan duration and reuse are primary product concerns.
- **UIR-05/06** — results comparison, review decisions and non-deleting validation.
- **UIR-07** — history and contextual diagnostics, preserving the monitoring and rescan contract.
- **UIR-08/09** — integrated Rust/.NET Debug/Release checks, native layout/keyboard/accessibility
  and scale evidence, then operator workflow acceptance. Remain on the branch after completion;
  merge, publication and the parked deletion/release work need their own later instruction.

There is no fixed session count: use the gate's scope and evidence to decide when to checkpoint.
Most sessions should end with a demonstrable improvement, a commit and an exact next action.
Native acceptance and full-drive/provider campaigns retain their existing separate authority;
prepare the concrete fixture/procedure before requesting the specific missing operator action.
