# Super Duper

A high-performance duplicate file detector built in Rust with a native Windows UI. Scan terabytes of files in seconds, browse results interactively, identify near-duplicate directory trees, and safely execute a reviewed deletion plan — all without touching the cloud.

![Dashboard](ss-main.png)

---

## Features

- **Two-tier hashing** — partial 1 KB hash filters candidates; full XxHash64 only runs on matches, keeping scan times low even across millions of files
- **Persistent hash cache** — RocksDB stores computed hashes keyed on path + modification timestamp so re-scans skip unchanged files entirely
- **Directory similarity** — Jaccard-index comparison of directory content-hash sets detects exact duplicates, subsets, and near-matches across folder trees
- **Session history** — every scan is stored as a session; switch between past sessions from the dashboard without rescanning
- **Reviewed deletion** — mark files individually or auto-mark keeping one copy per group; review the plan before a single byte is deleted
- **Native Windows UI** — WinUI 3 app with Mica material, NavigationView shell, live progress card, and full dark/light theme support
- **Headless CLI** — all pipeline stages are available as command-line subcommands for scripting and automation
- **SQLite storage** — fully embedded, zero-configuration; results survive restarts and are queryable with any SQLite tool

---

## Screenshots

| Dashboard | Duplicate Groups |
|---|---|
| ![Dashboard](ss-main.png) | ![Duplicate Groups](ss-duplicate-groups.png) |

| Directory Comparison | Settings |
|---|---|
| ![Directory Comparison](ss-directory-comparison.png) | ![Settings](ss-settings.png) |

---

## Architecture

Super Duper is a Cargo workspace with three crates consumed by a WinUI 3 C# frontend.

```
super-duper/
  crates/
    super-duper-core/     # rlib — all business logic (scan, hash, analysis, storage)
    super-duper-ffi/      # cdylib — C-compatible FFI for the UI
    super-duper-cli/      # binary — headless CLI
  ui/
    windows/              # WinUI 3 C#/.NET project
```

### How the pieces fit together

```
┌─────────────────────────────────┐
│  WinUI 3 (C# / .NET 10)        │
│  ViewModels ↔ Views (XAML)      │
│  P/Invoke via EngineWrapper.cs  │
└──────────────┬──────────────────┘
               │  C ABI (u64 handles)
┌──────────────▼──────────────────┐
│  super-duper-ffi  (cdylib)      │
│  Handle table · Callbacks       │
│  Paginated query marshalling    │
└──────────────┬──────────────────┘
               │  Rust function calls
┌──────────────▼──────────────────┐
│  super-duper-core  (rlib)       │
│  Scanner · Hasher · Engine      │
│  SQLite · RocksDB · Analysis    │
└─────────────────────────────────┘
```

---

## How Duplicate Detection Works

### Stage 1 — File Discovery

The scanner walks every configured root path in parallel using Rayon. For each file it:

1. Skips symlinks and zero-byte files
2. Tests the canonical path against every configured glob ignore pattern (e.g. `**/node_modules/**`)
3. Inserts the file into a concurrent hash map keyed by **exact byte size**

Files that do not share a size with any other file are provably unique — they are dropped here without ever being read. This single filter typically eliminates the majority of candidates.

### Stage 2 — Partial Hashing (1 KB)

For each size bucket containing two or more files, Super Duper reads the **first 1,024 bytes** of each file and computes an XxHash64 digest. Files whose 1 KB digest is unique within their size bucket are again provably non-duplicate and are dropped.

The 1 KB partial hash is fast enough that even large video files or disk images are dismissed in microseconds if their openings differ.

### Stage 3 — Full Content Hashing

Only files that survive the partial-hash filter — those sharing both exact size and an identical 1 KB opening — are read in full and hashed with XxHash64.

Before reading, Super Duper checks a **RocksDB hash cache**. The cache key is:

```
"{canonical_path}|{modified_secs}.{modified_subsec_nanos}"
```

Including sub-second precision in the key means that a file touched between two scans is never served a stale cached hash.

A cache hit returns the stored digest instantly. A cache miss reads the file, computes the hash, stores it, and continues. Because RocksDB persists across runs, re-scanning a large unchanged library takes a fraction of the original time.

Files that survive all three stages and share a full-content hash are **confirmed duplicates**. The wasted-space figure for each group is:

```
wasted_bytes = file_size × (copies − 1)
```

### Stage 4 — Database Write

All confirmed duplicates are written to SQLite in a single transaction:

- A `scan_session` row records the run, its root paths, and final counts
- Each file gets an upserted `scanned_file` row (keyed on canonical path so repeated scans update rather than duplicate)
- `duplicate_group` rows, scoped to the session, record the hash, size, and per-group wasted bytes
- `duplicate_group_member` join rows link each group to its constituent files

If the same set of root paths is scanned again, the existing session is reused and its groups are replaced rather than accumulated.

### Stage 5 — Directory Fingerprinting

After file-level analysis, Super Duper builds a hierarchical tree of every directory encountered during the scan. Working **bottom-up** (deepest directories first):

1. Collect the XxHash64 content hashes of every file directly in the directory
2. Union that set with the full hash sets already computed for all child directories
3. Sort and deduplicate the combined hash list
4. Hash the sorted list again with XxHash64 to produce a single **content fingerprint**

Two directories with identical fingerprints contain exactly the same files regardless of filenames or internal layout.

### Stage 6 — Directory Similarity (Jaccard Index)

To detect *near-duplicate* directories Super Duper uses the Jaccard similarity coefficient:

```
Jaccard(A, B) = |A ∩ B| / |A ∪ B|
```

where A and B are each directory's full set of content hashes (files anywhere beneath it).

Rather than comparing every pair of directories — O(n²) — Super Duper builds an **inverted index** mapping each hash to the directories that contain it. Only directories sharing at least one hash become candidates, and hashes that appear in more than 50 directories are treated as noise and skipped. This keeps the comparison space tractable even across large file trees.

Each candidate pair is classified:

| Classification | Condition |
|---|---|
| `exact` | Jaccard = 1.0 (or matching fingerprint) |
| `subset` | One directory's hash set is fully contained in the other |
| `threshold` | Jaccard ≥ configured minimum (default 0.5) |

Results are stored in the `directory_similarity` table and browsable from the Directory Comparison page.

---

## Getting Started

### Prerequisites

| Tool | Notes |
|---|---|
| Rust toolchain | `rustup` recommended, stable channel |
| `libclang-dev` | Required by RocksDB's bindgen step (Linux) |
| .NET 10 SDK | For the Windows UI only |
| Windows App SDK 1.8 | Runtime must be installed on the target machine |

### Building the Rust workspace

```bash
# Debug build (all crates)
cargo build --workspace

# Release build
cargo build --release --workspace
```

### Running the CLI

Edit `Config.toml` to set the paths you want to scan:

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

Then run the pipeline:

```bash
# Full duplicate detection pipeline
cargo run -p super-duper-cli -- process

# Re-run directory analysis only (fingerprints + similarity)
cargo run -p super-duper-cli -- analyze-directories

# Inspect the hash cache
cargo run -p super-duper-cli -- count-hash-cache

# Print the loaded configuration
cargo run -p super-duper-cli -- print-config

# Wipe all tables (with confirmation prompt)
cargo run -p super-duper-cli -- truncate-db
```

### Running the Windows UI

Open `ui/windows/SuperDuper.sln` in Visual Studio 2022 or later, select the `SuperDuper` project as the startup project, and press F5. The UI discovers `super_duper_ffi.dll` at startup; ensure the Rust FFI crate has been built first.

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

Super Duper uses an embedded SQLite database (`super_duper.db` in the working directory). No server or setup required. The schema is applied automatically on first run.

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
PRAGMA cache_size   = -64000;       -- 64 MB page cache
PRAGMA mmap_size    = 268435456;    -- 256 MB memory-mapped I/O
```

---

## FFI Design

The `super-duper-ffi` crate exposes a stable C ABI consumed by the C# UI via P/Invoke. Key design principles:

- **Handle-based** — the UI holds opaque `u64` handles; raw pointers never cross the boundary
- **Rust allocates, Rust frees** — every returned buffer has a matching `sd_free_*()` function; the C# wrapper calls it after marshalling
- **Thread-local errors** — `sd_last_error_message()` returns a human-readable description of the last failure on the calling thread
- **Paginated queries** — all list queries accept `offset` and `limit`; the response includes a `total` count for virtual scrolling
- **Synchronous scan** — `sd_scan_start()` blocks the calling thread; the C# wrapper runs it on a thread pool task and marshals progress callbacks to the UI dispatcher

---

## Project Status

The core pipeline, Windows UI, and CLI are all functional. This is an actively developed personal tool; the API surface and database schema may change between releases. See [ROADMAP.md](ROADMAP.md) for planned enhancements.

---

## Motivation

Accumulated over 20 years: countless machine builds, archives-of-archives, external drives with copies of copies. The only reliable way to know two files are identical is to verify their content — but hashing terabytes naively is prohibitively slow. Super Duper applies a cascade of progressively more expensive filters (size → partial hash → full hash → cache) to make that verification fast enough to actually run, and stores the results in a queryable database so a UI can present and act on them without re-scanning.

---

## License

MIT
