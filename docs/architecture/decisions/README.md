# Architecture decision records

Each record states one decision about the Windows app and its boundary with the Rust engine: the
context, what was decided, and what follows from it. Records are not edited to change a decision;
a new record supersedes an old one. The first five were recorded retrospectively on 2026-09-19 from
commit history and the plans preserved at the `v0.1.0` tag.

| ADR | Decision | Status | Date |
|---|---|---|---|
| [0001](0001-worker-process-boundary.md) | The Windows app reaches the engine through a worker process | Accepted | 2026-08-15 |
| [0002](0002-review-only-windows-app.md) | The Windows app is review-only | Accepted | 2026-08-19 |
| [0003](0003-one-owner-per-database.md) | One worker owns a database, one window per data folder | Accepted | 2026-09-18 |
| [0004](0004-keep-wpf-redesign-in-place.md) | Keep WPF and recompose the app instead of migrating | Accepted | 2026-09-14 |
| [0005](0005-app-state-in-localappdata.md) | App state lives in %LOCALAPPDATA%\SuperDuper | Accepted | 2026-09-18 |
| [0006](0006-last-chance-exception-handling.md) | Last-chance exception handling logs, stops the worker, and exits | Accepted | 2026-09-20 |
| [0007](0007-incremental-scan-scope.md) | Reject directory-mtime incremental scanning; keep the repeat cache | Rejected | 2026-09-20 |

To add a record, copy the Status, Date, Context, Decision and Consequences sections of an existing
one, take the next number, and link it here and from the architecture chapters it affects.

The architecture chapters these decisions shape are the [system overview](../overview.md),
[Windows app components](../windows-app-components.md),
[runtime scenarios](../windows-app-runtime.md), [conventions](../windows-app-concepts.md) and
[risks and technical debt](../windows-app-risks.md).
