# Windows bounded warning-memory evidence

`WPM13-bounded-memory` closes the read-only scale bound for the existing schema-v14 warning
aggregate slice. It does not add a general Activity workspace, warning categories, navigation,
outcome audit, filesystem mutation, or production Recycle Bin execution.

Run the one named completion verifier from an interactive Windows 11 x64 checkout after building
the Rust worker and Windows solution:

```powershell
./scripts/Verify-WindowsBoundedMemory.ps1
```

The verifier creates one ignored bundle under `artifacts/windows-bounded-memory/<timestamp>/` with
`manifest.json`, `verifier.log`, and `warning-scale-evidence.json`. The Release fixture inserts
exactly 100,000 retained schema-v14 aggregate rows, then walks 500 pages of 200 without retaining
the complete result. It verifies exact warning accounting, stable server-owned Phase,
Occurrence-count, and Message ordering with ID tie-breaks, sort-bound opaque cursors, immutable
terminal rows, and unchanged `executorEnabled:false` responses. Focused Core and loaded-WPF tests
separately hold the cache to five 25-row pages and the virtualized binding to one 25-row page while
proving cancellation, stale-context rejection, dispatcher responsiveness, keyboard access, and
focus restoration.

The accepted 2026-08-24 bundle is
`artifacts/windows-bounded-memory/20260824-143725-472`. Its Release evidence records 100,000
aggregates, 5,050,000 exactly accounted occurrences, 500 query samples, 15.906 ms p95 query time,
and 4,096 bytes private-memory growth. These pass the unchanged 100 ms p95 and 32 MiB private-growth
guards; `fullHistoryMaterialized` is false. This retained pass is the completion evidence and is not
rerun merely to obtain a different measurement.

The final real non-mutating WPF smoke fixtures are retained at
`C:\Users\gary\AppData\Local\Temp\super-duper-windows-smoke-cf345216b63e495c80304993b147e203`
(Debug) and
`C:\Users\gary\AppData\Local\Temp\super-duper-windows-smoke-bd8a59190c714ba3a6692c1c94f78eee`
(Release). Both passed the complete worker/WPF workflow and every focus assertion.

Provider, physical-accessibility, Recycle Bin/Shell mutation, unrelated Activity/outcome, broader
performance, and later-gate campaigns are outside this verifier and were deliberately not run.
