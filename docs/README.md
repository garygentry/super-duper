# Super Duper documentation

Super Duper is a review-only duplicate file finder: a Rust engine and worker process behind a
Windows 11 WPF app, plus a headless CLI. Start with the section for what you are doing.

## Using the Windows app

For people who run the released app. Read the tutorial first, then use the guides as needed.

| Document | What it is for |
| --- | --- |
| [Find your first duplicates](user-guide/getting-started.md) | Tutorial: from the release zip to a checked review plan on a practice folder |
| [Set up and run scans](user-guide/scan-folders.md) | Saved scans, locations, exclusions, cloud folders, repeat scans, cancel and delete |
| [Review duplicate files and folders](user-guide/review-duplicates.md) | Filter results, compare copies, record Keep and Remove decisions |
| [Apply location preferences](user-guide/location-preferences.md) | Rank preferred locations, preview, apply and reverse rule decisions |
| [Check marked copies](user-guide/check-marked-copies.md) | Confirm a plan still matches the files on disk |
| [Look into a past scan](user-guide/past-scans.md) | Reopen earlier scans, their settings, warnings and performance |
| [Fix startup and saved-data problems](user-guide/troubleshooting.md) | What to do for each startup, connection and saved-data message |
| [Windows app reference](user-guide/app-reference.md) | Screens, commands, shortcuts, statuses, limits and files |
| [How Super Duper finds and reviews duplicates](user-guide/how-it-works.md) | Why results can be trusted and why the app never deletes |
| [Changelog](../CHANGELOG.md) | Release notes and known limitations |

## Architecture

For maintainers changing the app, the worker or the engine.

| Document | What it is for |
| --- | --- |
| [System overview](architecture/overview.md) | Containers, stores and how they communicate |
| [Windows app components](architecture/windows-app-components.md) | Projects, composition root, shell, screens, worker boundary, services |
| [Windows app runtime scenarios](architecture/windows-app-runtime.md) | Startup, scans, live review state, worker recovery, shutdown |
| [Windows app conventions](architecture/windows-app-concepts.md) | Rules every change keeps: layering, threading, bounded data, safety, accessibility |
| [Windows app risks and technical debt](architecture/windows-app-risks.md) | Known weaknesses to weigh before changing nearby code |
| [Architecture decisions](architecture/decisions/README.md) | ADRs: worker process, review-only, database ownership, WPF, state location |
| [Rust crates](../crates/CLAUDE.md) | Engine modules, pipeline and concurrency model |

## Develop, test and release

| Document | What it is for |
| --- | --- |
| [Windows build](windows-build.md) | Prerequisites, build, test and run from source |
| [Isolated UI development sessions](windows-ui-dev-session.md) | Run the real app against disposable state; iterate without a desktop |
| [Windows test suites](windows-testing.md) | What each test project covers, needs and skips; opt-in categories |
| [Windows smoke workflow](windows-smoke.md) | Run and read the worker and WPF smoke |
| [Diagnostics and recovery](windows-recovery.md) | Logs, limitations, worker and database failures, interrupted work |
| [Release checklist](release-checklist.md) | Prepare, verify and publish a release |

## Contracts and storage

| Document | What it is for |
| --- | --- |
| [Worker protocol v1](worker-protocol-v1.md) | JSONL wire contract between the app and the worker |
| [Scan progress contract v1](scan-progress-contract-v1.md) | Progress payload, constants and how the app applies it |
| [Scan status database](scan-status-database.md) | Worker-owned telemetry database |
| [Export format v1](export-format-v1.md) | JSON and CSV shape of the CLI's `export` subcommand |
| [Storage schema v3](storage-schema-v3.md) … [v15](storage-schema-v15.md) | What each schema version added; `schema.sql` is authoritative |

## About this documentation

[`docplan.json`](docplan.json) records what each document covers, its sources of truth in the code,
and the known gaps; [`docplan.schema.json`](docplan.schema.json) validates it. Update the user guide
when user-visible labels, messages or limits change, and the architecture notes when layering,
runtime behavior or conventions change.
