# Super Duper

A high-performance duplicate file detector written in Rust. Scan terabytes of files, confirm
duplicates by content (never by name), identify near-duplicate directory trees, and stage a
reviewed deletion plan — all locally, with nothing sent to the cloud.

The engine is driven today through a headless **command-line interface**. A native Windows UI
(WinUI 3) also exists, but it is an early-stage experiment — see
[Windows UI](#windows-ui-early-stage--experimental) at the end.

---

## Features

- **Two-tier hashing** — files are grouped by exact size, then a partial 1 KB XxHash64 filters
  candidates; the full-content XxHash64 only runs on files that survive both filters, keeping
  scan times low even across millions of files
- **Persistent hash cache** — RocksDB stores computed hashes keyed on path + modification
  timestamp (sub-second precision) so re-scans skip unchanged files entirely
- **Directory similarity** — Jaccard-index comparison of directory content-hash sets detects
  exact duplicates, subsets, and near-matches across folder trees, regardless of filenames
- **Session history** — every scan is stored as a session; the same set of root paths reuses
  its session rather than accumulating duplicates
- **Reviewed deletion** — files are staged in a deletion plan; nothing is removed until the plan
  is executed
- **Headless CLI** — every pipeline stage is available as a command-line subcommand for
  scripting and automation
- **Embedded SQLite storage** — zero-configuration; results survive restarts and are queryable
  with any SQLite tool

---

## Architecture

Super Duper is a Cargo workspace. The Rust **core library** is the product — it owns all
scanning, hashing, analysis, and storage logic. The FFI crate and the Windows UI are optional
consumers layered on top of it.

```
super-duper/
  Config.toml             # Scan targets and ignore patterns (CLI)
  crates/
    super-duper-core/     # rlib — all business logic (scan, hash, analysis, storage)
    super-duper-cli/      # binary — headless CLI (primary entry point today)
    super-duper-ffi/      # cdylib — C-compatible FFI for native UIs
  ui/
    windows/              # WinUI 3 C#/.NET app — early-stage, experimental
```

### How the pieces fit together

```
┌─────────────────────────────────┐
│  WinUI 3 (C# / .NET 10)         │   ← experimental, optional
│  P/Invoke via EngineWrapper.cs  │
└──────────────┬──────────────────┘
               │  C ABI (u64 handles)
┌──────────────▼──────────────────┐
│  super-duper-ffi  (cdylib)      │   ← optional FFI boundary
│  Handle table · Callbacks       │
└──────────────┬──────────────────┘
               │  Rust function calls
┌──────────────▼──────────────────┐
│  super-duper-core  (rlib)       │   ← the engine; also driven directly by the CLI
│  Scanner · Hasher · Engine      │
│  SQLite · RocksDB · Analysis    │
└─────────────────────────────────┘
```

The CLI links the core crate directly and is the recommended way to run the pipeline.

---

## How Duplicate Detection Works

### Stage 1 — File Discovery

The scanner walks every configured root path in parallel using Rayon. For each file it:

1. Skips symlinks and zero-byte files
2. Tests the path against every configured glob ignore pattern (e.g. `**/node_modules/**`)
3. Inserts the file into a concurrent hash map keyed by **exact byte size**

Files that do not share a size with any other file are provably unique — they are dropped here
without ever being read. This single filter typically eliminates the majority of candidates.

### Stage 2 — Partial Hashing (1 KB)

For each size bucket containing two or more files, Super Duper reads the **first 1,024 bytes** of
each file and computes an XxHash64 digest. Files whose 1 KB digest is unique within their size
bucket are again provably non-duplicate and are dropped.

The 1 KB partial hash is fast enough that even large video files or disk images are dismissed in
microseconds if their openings differ.

### Stage 3 — Full Content Hashing

Only files that survive the partial-hash filter — those sharing both exact size and an identical
1 KB opening — are read in full and hashed with XxHash64.

Before reading, Super Duper checks a **RocksDB hash cache**. The cache key is:

```
"{canonical_path}|{modified_secs}.{modified_subsec_nanos}"
```

Including sub-second precision in the key means that a file touched between two scans is never
served a stale cached hash.

A cache hit returns the stored digest instantly. A cache miss reads the file, computes the hash,
stores it, and continues. Because RocksDB persists across runs, re-scanning a large unchanged
library takes a fraction of the original time.

Files that survive all three stages and share a full-content hash are **confirmed duplicates**.
The wasted-space figure for each group is:

```
wasted_bytes = file_size × (copies − 1)
```

### Stage 4 — Database Write

All confirmed duplicates are written to SQLite in a single transaction:

- A `scan_session` row records the run, its root paths, and final counts
- Each file gets an upserted `scanned_file` row (keyed on canonical path so repeated scans update
  rather than duplicate)
- `duplicate_group` rows, scoped to the session, record the hash, size, and per-group wasted bytes
- `duplicate_group_member` join rows link each group to its constituent files

If the same set of root paths is scanned again, the existing session is reused and its groups are
replaced rather than accumulated.

### Stage 5 — Directory Fingerprinting

After file-level analysis, Super Duper builds a hierarchical tree of every directory encountered
during the scan. Working **bottom-up** (deepest directories first):

1. Collect the XxHash64 content hashes of every file directly in the directory
2. Union that set with the full hash sets already computed for all child directories
3. Sort and deduplicate the combined hash list
4. Hash the sorted list again with XxHash64 to produce a single **content fingerprint**

Two directories with identical fingerprints contain exactly the same files regardless of
filenames or internal layout.

### Stage 6 — Directory Similarity (Jaccard Index)

To detect *near-duplicate* directories Super Duper uses the Jaccard similarity coefficient:

```
Jaccard(A, B) = |A ∩ B| / |A ∪ B|
```

where A and B are each directory's full set of content hashes (files anywhere beneath it).

Rather than comparing every pair of directories — O(n²) — Super Duper builds an **inverted index**
mapping each hash to the directories that contain it. Only directories sharing at least one hash
become candidates, and hashes that appear in more than 50 directories are treated as noise and
skipped. This keeps the comparison space tractable even across large file trees.

Each candidate pair is classified:

| Classification | Condition |
|---|---|
| `exact` | Jaccard = 1.0 (or matching fingerprint) |
| `subset` | One directory's hash set is fully contained in the other |
| `threshold` | Jaccard ≥ configured minimum (default 0.5) |

Results are stored in the `directory_similarity` table.

---

## Getting Started

### Prerequisites

| Tool | Notes |
|---|---|
| Rust toolchain | `rustup` recommended, stable channel |
| `libclang-dev` | Required by RocksDB's bindgen step (Linux) |

The Windows UI has additional prerequisites — see that [section](#windows-ui-early-stage--experimental).

### Building the Rust workspace

```bash
# Debug build (all crates)
cargo build --workspace

# Release build
cargo build --release --workspace
```

### Configuring scan targets

Edit `Config.toml` to set the paths you want to scan and any patterns to ignore:

```toml
root_paths = [
    "C:/Users/you/Documents",
    "D:/Archive",
]
ignore_patterns = [
    "**/node_modules/**",
    "**/.git/**",
    "*/$RECYCLE.BIN",
]
```

### Running the CLI

The CLI is the primary way to drive the engine. Each pipeline stage is a subcommand:

```bash
# Full duplicate detection pipeline (scan → hash → db write → directory analysis)
cargo run -p super-duper-cli -- process

# Re-run directory analysis only (fingerprints + similarity)
cargo run -p super-duper-cli -- analyze-directories

# Inspect the hash cache (entry count)
cargo run -p super-duper-cli -- count-hash-cache

# Print the loaded configuration
cargo run -p super-duper-cli -- print-config

# Wipe all SQLite tables (with confirmation prompt)
cargo run -p super-duper-cli -- truncate-db
```

---

## Environment Variables

Configured via a `.env` file in the working directory.

| Variable | Default | Description |
|---|---|---|
| `TRACING_LEVEL` | `info` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |
| `LOG_FILE_PATH` | `./logs/sd.log` | File log output path |
| `HASH_CACHE_PATH` | `content_hash_cache.db` | RocksDB hash cache location |

---

## Database

Super Duper uses an embedded SQLite database (`super_duper.db` in the working directory). No
server or setup required. The schema is applied automatically on first run.

### Key tables

| Table | Purpose |
|---|---|
| `scan_session` | One row per scan run; tracks root paths, status, and aggregate counts |
| `scanned_file` | Global file index; upserted on every scan; tracks hashes and deletion flag |
| `duplicate_group` | Confirmed duplicate sets, scoped to a session |
| `duplicate_group_member` | Junction table linking files to their duplicate group |
| `directory_node` | Directory tree with size and file-count aggregates |
| `directory_fingerprint` | Per-directory content fingerprint and full hash set |
| `directory_similarity` | Pre-computed Jaccard pairs with score and match type |
| `deletion_plan` | Files staged for deletion with execution history |

### Performance pragmas

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous  = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA cache_size   = -64000;       -- 64 MB page cache
PRAGMA mmap_size    = 268435456;    -- 256 MB memory-mapped I/O
PRAGMA busy_timeout = 5000;         -- 5 second lock timeout
```

---

## FFI Design

The `super-duper-ffi` crate exposes a stable C ABI so a native UI can consume the core. It is
optional — the CLI does not use it. Key design principles:

- **Handle-based** — the caller holds opaque `u64` handles; raw pointers never cross the boundary
- **Rust allocates, Rust frees** — every returned buffer has a matching `sd_free_*()` function
- **Thread-local errors** — `sd_last_error_message()` returns a description of the last failure on
  the calling thread
- **Paginated queries** — all list queries accept `offset` and `limit`; responses include a
  `total` count for virtual scrolling
- **Synchronous scan** — `sd_scan_start()` blocks the calling thread; a caller is expected to run
  it on a worker thread and marshal progress callbacks itself

---

## Windows UI (Early-Stage / Experimental)

A WinUI 3 (C# / .NET 10) app lives under `ui/windows`. It is a **proof-of-concept** that consumes
the Rust core through the FFI crate via P/Invoke. It demonstrates the engine behind a native
interface, but it is **not a finished or fully functional application** and is not recommended
for general use.

**Known limitations:**

- Advanced scan options (minimum file size, hash algorithm, thread count) are stubbed and
  disabled in the UI ("coming soon") — the FFI does not yet pass them to the engine
- Saved scan-profile management is modeled but not functional
- Requires `LangVersion=preview` and contains mandatory XAML compiler workarounds for the
  .NET 10 + Windows App SDK 1.8 combination, which may not be stable across SDK versions

**Prerequisites:** .NET 10 SDK, the Rust toolchain, and the Windows App SDK 1.8 runtime.

**Build:** open `ui/windows/SuperDuper.sln` in Visual Studio 2022 or later, or run
`dotnet build ui/windows/SuperDuper.sln`. The C# build automatically runs
`cargo build -p super-duper-ffi` first and copies `super_duper_ffi.dll` next to the executable.

---

## Project Status

The **Rust core and CLI are functional** and are the actively developed part of the project. The
Windows UI is experimental (see above). This is a personal tool; the API surface and database
schema may change between releases. See [ROADMAP.md](ROADMAP.md) for planned enhancements.

---

## Motivation

Accumulated over 20 years: countless machine builds, archives-of-archives, external drives with
copies of copies. The only reliable way to know two files are identical is to verify their
content — but hashing terabytes naively is prohibitively slow. Super Duper applies a cascade of
progressively more expensive filters (size → partial hash → full hash → cache) to make that
verification fast enough to actually run, and stores the results in a queryable database so the
data can be presented and acted on without re-scanning.

---

## License

MIT
