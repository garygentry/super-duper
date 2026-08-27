# Scan Status Database

The Rust worker owns a local SQLite status database for scan performance and health telemetry. It
is separate from the product-results database: status rows may reference a product run ID, but they
do not own duplicate results, review state, warning truth, preflight state, or operation evidence.
Deleting this database or its terminal history cannot delete or reinterpret product results.

## Location and lifecycle

The worker defaults to `scan_status.db` beside the configured product database. Tests and alternate
hosts can set `SUPER_DUPER_STATUS_DB_PATH` or use `WorkerOptions::with_status_database_path`.
`ScanEngine` telemetry remains opt-in through `with_status_db_path`; the worker enables it.

Opening the writer connection creates or transactionally migrates the schema, reconciles abandoned
running/cancelling rows to `interrupted`, applies default retention, and performs a passive WAL
checkpoint. An active run writes cumulative snapshots only at bounded phase/progress boundaries;
there are no per-file telemetry rows. Telemetry failures are logged and counted where possible but
do not make status storage product truth or fail an otherwise valid product scan.

Each atomic flush reads the committed 47-counter metrics-v3 summary once and uses one multi-row upsert after
validating every monotonic value. The first snapshot creates every fixed counter row; later snapshots
do not rewrite unchanged values, so `updated_sequence` identifies the flush that last changed that
counter. The exact full flush payload remains in the replay ledger while the run is writable.

## Bounds and retention

The default fixed-count policy retains the newest 50 terminal runs and at most 100,000 host samples
per retained or active run. Device samples share host sequence keys and cascade with trimmed host
samples. Terminal flush replay payloads are removed because terminal rows reject further writes;
their counters, phases, devices, and bounded samples remain. Active runs are never removed.

Retention runs at writer startup and after a terminal scan write. It is transactional and safe to
repeat. SQLite uses WAL, `synchronous=NORMAL`, a 5-second busy timeout, automatic checkpoints every
1,000 WAL pages, and a passive checkpoint after retention/history deletion. A busy passive
checkpoint is reported as health information rather than forcing readers out.

## Query contract

- Run history uses a strict descending `id < before_id` cursor and permits 1-100 rows.
- Host and per-device sample history use a strict ascending `sequence > after_sequence` cursor and
  permit 1-500 rows.
- Counters, phases, and device descriptors are fixed run-owned summaries; unknown run IDs fail
  explicitly.
- Missing or deleted status history is unavailable telemetry, never an empty or failed product run.

`StatusDatabase::delete_terminal_history` removes terminal status rows only. The product database is
not attached to the status connection and is covered by isolation tests.

## Platform sampler

The platform-neutral sampler owns cadence and cardinality only; it owns no database connection. The
engine integrates it through one mutex-serialized status writer and a five-second heartbeat. Phase
transitions and heartbeat samples therefore share one monotonic sequence and cannot race SQLite.
The heartbeat continues while a scan phase emits no progress callback, and is joined before terminal
state is written. A fake clock/platform contract proves interval suppression, maximum sample count,
delayed-interval loss, and explicit unavailable gauges.

On Windows, read-only native probes map target volume roots to physical disk numbers, record volume
GUID/filesystem/capacity/free space, and omit hardware serial numbers. Host samples use process
times, memory, I/O counters, system times, and memory status. Physical-device samples derive read
throughput, scaled IOPS, read latency, active time, and queue depth from cumulative disk-performance
counters. Permission-blocked or unsupported device counters remain unavailable with a count; they
do not become zero or fail the scan.
