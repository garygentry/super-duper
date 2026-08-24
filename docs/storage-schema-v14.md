# Storage schema v14

Schema v14 adds `run_warning_aggregate`, a run-owned warning explanation table. Each row stores one
scan phase/category/code, severity `warning`, a bounded message, an exact positive occurrence count,
and JSON containing at most three bounded representative examples. `(run_id, code)` is unique.
Run/phase, run/occurrence-count, and run/message indexes support stable server-owned sorting; every
keyset order uses aggregate ID as its final tie-breaker. Schema version remains 14.

The engine replaces the current running run's aggregate set transactionally and sets
`scan_run.warning_count` to the exact sum. Throttled worker progress may publish a live warning count
but preserves the last durably accounted count. Terminal completion inserts an explicit bounded
fallback aggregate for any unclassified remainder, then makes the rows immutable with the run.

Migration from v13 is transactional. Existing positive warning counts become one
`legacy_unstructured_warning` aggregate whose example truthfully states that pre-v14 details were
not retained. Zero-warning runs receive no row. Restart reads only SQLite; paging performs no
filesystem, provider, Shell, or Recycle Bin access. Unknown versions newer than 14 remain rejected.

The retained Release scale fixture described in `windows-bounded-memory.md` pages 100,000 aggregate
rows without full-history materialization and proves the indexed query and memory guards.
