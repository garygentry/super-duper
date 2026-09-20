# Export format v1

The `super-duper export` CLI subcommand writes duplicate file groups or session data as JSON or
CSV, to stdout. This is v1 of that format. It is versioned independently of the storage schema and
the worker protocol: a future Windows app export (issue #30, `WPM14-export`) must reuse these field
names and rules, or the version bumps. Additive fields are allowed within v1 without bumping it;
the CLI is the only current producer, and `crates/super-duper-core/src/export.rs` is the source of
truth for exact field names.

## Kinds

Two independent export kinds, each its own document:

- `duplicate-groups`: every duplicate file group and its member files for one run.
- `sessions`: session definitions and their run history.

Neither kind currently covers duplicate *folder* groups, review decisions, or deletion outcomes
(the app-side `WPM14-export` scope); those are out of scope for v1.

## Selecting what to export

- `duplicate-groups` takes an optional run ID (`--run`); it defaults to the latest completed run.
  An explicit run ID that does not exist is a clear error (`run <id> not found`), as is omitting
  `--run` when no run has completed (`no completed run is available for export`).
- `sessions` takes an optional session ID (`--session`); it defaults to every session. An explicit
  session ID that does not exist is a clear error (`session <id> not found`).

## JSON

One pretty-printed JSON object per document, camelCase field names (matching
`docs/worker-protocol-v1.md`), UTF-8, with a trailing newline. Every document carries a
`formatVersion` (currently `1`) and a `generatedAt` (RFC 3339, UTC) at the top level.

### `duplicate-groups`

```json
{
  "formatVersion": 1,
  "generatedAt": "2026-09-20T12:00:00+00:00",
  "runId": 19,
  "groups": [
    {
      "id": 31,
      "contentHash": "3a7c1f9e2b4d5061",
      "fileSize": 5242880,
      "fileCount": 2,
      "wastedBytes": 5242880,
      "members": [
        {
          "path": "D:\\Archive\\photo.jpg",
          "fileName": "photo.jpg",
          "parentDir": "D:\\Archive",
          "rootPath": "D:\\Archive",
          "relativePath": "photo.jpg",
          "driveLetter": "D:",
          "fileSize": 5242880,
          "lastModifiedUnixNanos": "1786795200000000000"
        }
      ]
    }
  ]
}
```

| Field | Type | Notes |
| --- | --- | --- |
| `runId` | integer | The exported run. |
| `groups[].id` | integer | `duplicate_group.id`. |
| `groups[].contentHash` | string | Lowercase 16-digit hex XxHash64, the grouping key — not a path, and not guaranteed stable across engine versions. |
| `groups[].fileSize` | integer | Size shared by every member, in bytes. |
| `groups[].fileCount` | integer | Member count. |
| `groups[].wastedBytes` | integer | `fileSize * (fileCount - 1)`. |
| `groups[].members[].path` | string | Plain spelling; see **Path spelling** below. Not an identity value. |
| `groups[].members[].fileName`, `.parentDir`, `.rootPath`, `.relativePath`, `.driveLetter` | string | As recorded by the scan. `parentDir` is plain-spelled like `path`; `rootPath` and `relativePath` are already plain. |
| `groups[].members[].fileSize` | integer | Bytes. |
| `groups[].members[].lastModifiedUnixNanos` | string | Unix nanoseconds. See **Wide integers** below. |

### `sessions`

```json
{
  "formatVersion": 1,
  "generatedAt": "2026-09-20T12:00:00+00:00",
  "sessions": [
    {
      "id": 7,
      "name": "Photos",
      "roots": ["D:\\Photos"],
      "ignorePatterns": ["**/node_modules/**"],
      "cloudPolicy": "exclude_registered_roots",
      "createdAt": "2026-08-15T12:00:00+00:00",
      "updatedAt": "2026-08-15T12:00:00+00:00",
      "runs": [
        {
          "id": 19,
          "status": "completed",
          "createdAt": "2026-09-19T08:00:00+00:00",
          "startedAt": "2026-09-19T08:00:01+00:00",
          "completedAt": "2026-09-19T08:04:30+00:00",
          "filesDiscovered": 42000,
          "bytesDiscovered": 128849018880,
          "filesHashed": 41000,
          "duplicateFileGroups": 31,
          "duplicateFolderGroups": 2,
          "wastedBytes": 5242880,
          "warningCount": 3,
          "excludedSubtreeCount": 1,
          "engineVersion": "0.2.0"
        }
      ]
    }
  ]
}
```

`roots` and `ignorePatterns` are the session's own definitions, already plain-spelled (they are
never canonicalized). `runs` is every run of the session, newest first, each with the same fields
as `scan_run` (`docs/storage-schema-v15.md`); `startedAt`/`completedAt` are `null` for a run that
never started or finished.

## CSV

RFC 4180: comma-separated, CRLF-terminated rows, a header row of the same field names as the JSON
form, a field quoted (with doubled internal quotes) only when it contains a comma, quote, or
newline. There is no envelope — no `formatVersion` or `generatedAt` row — so a script that needs
those reads the JSON form instead.

`duplicate-groups` is one row per group member, the group's own fields repeated on every row:

```csv
groupId,contentHash,groupFileSize,groupFileCount,wastedBytes,path,fileName,parentDir,rootPath,relativePath,driveLetter,fileSize,lastModifiedUnixNanos
```

`sessions` is one row per run, the session's own fields repeated on every row; a session with no
runs gets one row with the run columns blank. `roots` and `ignorePatterns` are each one cell, their
entries joined with `;` (a Windows path never contains `;`):

```csv
sessionId,sessionName,roots,ignorePatterns,cloudPolicy,sessionCreatedAt,sessionUpdatedAt,runId,runStatus,runCreatedAt,runStartedAt,runCompletedAt,filesDiscovered,bytesDiscovered,filesHashed,duplicateFileGroups,duplicateFolderGroups,wastedBytes,warningCount,excludedSubtreeCount,engineVersion
```

## Path spelling

Scans store canonical Windows paths in verbatim form (`\\?\C:\...`, `\\?\UNC\server\share\...`;
`crates/super-duper-core/src/path_spelling.rs`). Export shows the plain form instead
(`C:\...`, `\\server\share\...`) — the same rule the Windows app's `DisplayPaths.Plain` display
converter uses. This is a display/export convenience, never an identity value: a script that needs
to feed an exported path back to the CLI or the worker should not assume it round-trips byte for
byte with what the database stores.

## Wide integers

`contentHash` and `lastModifiedUnixNanos` are JSON strings, not numbers, because they exceed the
53-bit safe-integer range most JSON parsers preserve exactly for numbers — the same reason
`docs/worker-protocol-v1.md` stringifies `observedFileSize` and `modifiedTimeUnixNanos`. Every other
integer field (byte counts, counts, IDs) fits comfortably and stays a JSON number.
