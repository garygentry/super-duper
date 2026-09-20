# ADR-0007: Reject directory-mtime incremental scanning; keep the repeat cache

- **Status:** Rejected
- **Date:** 2026-09-20

## Context

Issue #33's incremental-scan item, as filed, suggested "record directory modification times and
skip unchanged subtrees on repeat scans." Stage 13 of the issue #51 burn-down plan is a design gate
that must decide whether a correct version of that idea exists, and whether it is worth building,
before any schema work (a new table plus a `CURRENT_SCHEMA_VERSION` bump to 16) is started.

**The literal proposal is unsafe.** On NTFS, a directory's own modification time changes only when
an entry is added, removed, or renamed directly inside it. It does not change when a contained
file's *content* is edited in place, and it is not recursive — a change three levels down does not
touch the top-level directory's timestamp at all. A scan that skipped a subtree whenever its
directories' mtimes matched the previous run would silently miss every content edit that didn't
also add, remove, or rename a file. For a duplicate detector, that is a correctness regression, not
an optimization: a file could be edited, no longer be a duplicate of anything, and the tool would
keep reporting the stale duplicate group indefinitely.

**The next most obvious signal is off-limits.** The NTFS USN (Update Sequence Number) journal
records every change to every file and directory on a volume, including content edits, and would be
the technically correct way to answer "what changed since the last scan" without re-walking
anything. Reading it needs a handle to the volume itself (`\\.\C:`), which needs the caller to be
running elevated (or hold `SeBackupPrivilege`) on a standard install. This app runs unelevated and
is deliberately review-only (ADR-0002); requiring elevation for a performance optimization is a
regression in both trust boundary and UX, so the USN journal is out of scope here, matching the
issue's own suspicion.

That leaves the repeat cache — already implemented, not part of this gate's proposal, but the
existing mechanism that already avoids re-hashing an unchanged file. It uses a per-file signature
(stable file identity, size, modified time, and an NTFS change-time-derived token; see
`hasher/repeat_cache.rs` and `platform::content_signature_metadata`) that **does** detect content
edits correctly, because it's read per file, not inferred from a parent directory. It just doesn't
avoid the *enumeration* cost of walking every directory and reading every file's metadata each run.

## Measurement

Both runs below use the Release build with the same disposable database and hash cache, `process`,
and `RepeatCachePolicy::ReuseVerified` (the default). "Cold" is the first run against a fixture;
"warm" is a second, immediate rerun with nothing on disk changed in between. `Scan` is
`ScanEngine::scan`'s enumeration phase (directory walk, metadata only, no content access); `Hash` is
the partial/full content-hashing phase, which is where the repeat cache is consulted and updated.

**A synthetic fixture** (disposable, generated for this measurement and not committed): 621,133
files, 4,096 bytes each, spread across 2,000 directories. The pathological choice of "every file
exactly the same size" was deliberate — same-size files are exactly what forces the hasher to open
and hash every one of them to prove they aren't duplicates, so this is close to a worst case for the
hash phase relative to enumeration. (The plan's target was 1,000,000+ files; file creation on this
VM sustained only ~200–500 files/sec — apparently unrelated to this tool, since the same VM's `cargo`
and `dotnet` builds elsewhere in this session ran at their usual speed — so generation was stopped at
621k to keep this stage's wall-clock time reasonable. The measured ratios below are not sensitive to
exact file count.)

| Run  | Scan (enumeration) | Hash (screen + repeat cache) | DB write | Dir analysis |
|------|---------------------|-------------------------------|----------|--------------|
| Cold | 35.34s | 1006.86s | 36.66s | 3.49s |
| Warm | 32.10s | 38.48s | 38.10s | 3.41s |

**A real folder**, read-only: this repository's own `target/` build output (~32k files, ~43 GB,
genuinely mixed file sizes and a lot of same-size incremental-compilation artifacts).

| Run  | Scan (enumeration) | Hash (screen + repeat cache) | DB write | Dir analysis |
|------|---------------------|-------------------------------|----------|--------------|
| Cold | 6.92s | 327.10s | 22.65s | 180.63s |
| Warm | 2.06s | 3.21s | 3.61s | 2.07s |

(The warm real-folder run also found and exercised a genuine concurrency bug — see **Bug found
while measuring**, below — which is fixed on the branch that adds this ADR, and is what let the warm
numbers above be collected at all.)

**Reading the numbers:**

- The repeat cache the engine already has **already delivers the big win**: 26x on the synthetic
  fixture (1006.86s → 38.48s), 102x on the real folder (327.10s → 3.21s). This is the entire benefit
  a "skip work for unchanged files" feature was ever going to provide, and it's already shipped.
- Once that win is banked, **enumeration is not the dominant remaining cost** — it's comparable to
  or smaller than the hash phase on both fixtures (32.10s vs. 38.48s synthetic; 2.06s vs. 3.21s
  real). A feature that could skip enumeration *perfectly and for free* would only remove roughly a
  third to a half of the already-small warm-run time, not the multi-hundred-second cost that made
  the cold run slow.
- Some of even that residual enumeration cost is the OS's own file-system cache warming up, not
  anything this engine controls: the real folder's `Scan` phase alone dropped from 6.92s to 2.06s
  between cold and warm with no application change at all.
- Directory-mtime skipping specifically could only ever address the *enumeration* number (it's a
  walk-time optimization), and even then only for directories whose entry set is provably unchanged
  — it cannot skip the per-file check that content edits require, which is exactly the check the
  repeat cache already does. So the realistic ceiling for this specific idea is a fraction of an
  already-small number, in exchange for a new schema version, new per-directory state, and the
  correctness burden of getting the "provably unchanged" test right.

## Decision

**Reject** directory-modification-time incremental scanning, as proposed on issue #33. Stage 14
(implementation) is marked `skipped` in the issue #51 tracker. No schema change, no `docs/storage-schema-v16.md`, no `CURRENT_SCHEMA_VERSION` bump.

This is not a rejection of the underlying goal — making repeat scans of a stable tree faster — it's
a rejection of *this* mechanism for it, on both grounds the gate asked about:

1. **Correctness.** The literal design (skip a subtree when its directory mtimes match) misses
   in-place content edits by construction; no variant of "compare directory mtimes" can fix that
   without also touching every file, at which point it has stopped skipping anything.
2. **Measured benefit.** Even a perfect, elevation-free version of the idea would only ever save the
   enumeration share of a warm run, and that share is already small next to what the repeat cache
   has already captured.

If a future engine change makes enumeration itself expensive again (for example, a much larger
average directory depth or file count than these fixtures, or a slow network filesystem where the
per-entry `FindNextFile`/metadata cost dominates), this decision should be revisited with fresh
numbers for that regime — the two fixtures measured here are a same-size-heavy synthetic tree and a
local NTFS build directory, not a general claim about every filesystem shape.

## Consequences

- Issue #33's incremental-scan item is resolved as "evaluated, not implemented," not left silently
  incomplete. The other two items on #33 (hash cache trim, cancellable FFI scan) are already merged;
  this ADR's rejection is the resolution of the third.
- No new schema version, no new `directory_*` state table for tracking mtimes across runs, no
  change to `scanner/walk.rs` or `engine.rs`'s enumeration phase.
- The repeat cache remains the primary lever for repeat-scan performance. Any future work aimed at
  making repeat scans faster should start from what's actually still expensive in a warm run at
  scale — per this measurement, the hash phase's per-file signature check (a handful of Win32 calls
  per file even on a cache hit) and enumeration are now the same order of magnitude, so the next
  worthwhile target, if one is ever prioritized, is more likely to be reducing per-file syscalls in
  `platform::content_signature_metadata` than skipping directories.

## Bug found while measuring

The first attempt at the real-folder warm run aborted outright:

```
WARN Rejected internally produced scan progress observation:
  full-hash satisfied files must equal cache hits plus completed content reads
ERROR: IO error: scan engine rejected a hash progress observation
```

`hasher/xxhash.rs`'s full-hash path recorded a file's cache-lookup outcome
(`full_hash_cache_hits`/`_misses`/`_errors`) in its own immediate, separate call to the progress
batcher, while the same file's `full_hash_satisfied_files` was recorded later, in a second call,
once the whole file's outcome was known. Under real concurrency, with the vast majority of files
resolving as cache hits (exactly the warm-run case this stage needed to measure), another thread's
publish could observe the first counter incremented without the second, violating the progress
contract's exact-equality invariant and aborting the entire scan. `populate_partial_hash` already
built its whole delta locally before ever calling the batcher; `populate_full_hash` now does the
same, closing the gap. See the `concurrent_full_hash_cache_hits_never_publish_a_snapshot_with_mismatched_satisfied_files`
test.

This was filed and tracked separately as #65 and is fixed on the same branch as this ADR, since it
blocked the very measurement this stage exists to produce.
