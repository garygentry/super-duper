# Super Duper Worker Protocol V1

## Status and Scope

This document defines version 1 of the local protocol between `SuperDuper.Windows` and the
`super-duper-worker` child process. The worker is a single-client, long-lived process launched by
the Windows application. Milestones 0–6 implement negotiation, session and scan lifecycle, and
separately paged duplicate-file and exact-duplicate-folder result browsing. The read-only
Milestone 8 additions extend duplicate-file pages with a filtered review summary, immutable
selected-root/drive member context, bounded per-group selected-root/drive counts, an optional
across-drives group filter, aggregate location coverage for the current query, and a keyset-paged
selected-root facet that can filter the group query. Warning commands remain reserved for a later
milestone.

The transport is UTF-8 newline-delimited JSON (JSONL) over redirected standard input and standard
output. It is a local process boundary, not a network API.

## Transport and Framing

- The client writes requests to the worker's standard input.
- The worker writes responses and events to standard output.
- Each frame is exactly one JSON object followed by LF (`0A`). Receivers may accept CRLF and remove
  the trailing CR before parsing.
- JSON strings use normal JSON escaping. A literal line break may not appear inside a frame.
- UTF-8 is used without a byte-order mark.
- A frame may contain at most 1,048,576 UTF-8 bytes, excluding the line terminator.
- Empty lines are invalid frames.
- Unknown object members are ignored so that fields can be added compatibly within version 1.
- A malformed, empty, non-object, invalid-UTF-8, or oversized input frame is a fatal transport
  error. The worker writes a diagnostic to stderr and exits non-zero because no request ID can be
  correlated reliably.
- A malformed, empty, non-object, invalid-UTF-8, or oversized stdout frame is a fatal worker error.
  The client fails all pending requests and stops using that worker process.

Standard output is protocol-only. The worker must never write log prefixes, panic diagnostics,
progress text, or other human-readable output to stdout. Diagnostics and panic output go to
standard error. The client continuously drains stderr to prevent a full pipe from blocking the
worker and retains a bounded diagnostic tail for connection errors.

Milestone 6 performance diagnostics use stderr records such as:

```text
performance kind=scan_phase run_id=19 phase=hashing duration_ms=842.117
performance kind=result_query method=duplicate_file_group.page run_id=19 group_id=- page_size=200 returned=42 total=42 duration_ms=3.604
```

A completed run produces duration records for all five phases (`discovering`, `hashing`,
`persisting`, `analyzing_folders`, and `finalizing`). Every successful duplicate-file/folder
group/member page query and selected-root facet page query produces a duration record. Query
records exclude path search/filter text. These diagnostics are not protocol frames and never
appear on stdout.

## Envelopes

All envelope names and fields are case-sensitive. Request IDs are opaque, non-empty strings chosen
by the client and unique among that client's pending requests.

### Request

```json
{"type":"request","id":"42","method":"hello","params":{}}
```

Required fields:

- `type`: `"request"`
- `id`: opaque string
- `method`: command name string
- `params`: JSON object; use `{}` when the command has no parameters

### Successful Response

```json
{"type":"response","id":"42","ok":true,"result":{}}
```

Required fields:

- `type`: `"response"`
- `id`: the corresponding request ID
- `ok`: `true`
- `result`: command-specific JSON value

### Error Response

```json
{"type":"response","id":"42","ok":false,"error":{"code":"invalid_request","message":"protocolVersions must not be empty","retryable":false,"details":{}}}
```

Required fields:

- `type`: `"response"`
- `id`: the corresponding request ID
- `ok`: `false`
- `error.code`: stable machine-readable snake-case string
- `error.message`: concise human-readable description suitable for local diagnostics
- `error.retryable`: whether retrying the same operation without user/configuration changes may work
- `error.details`: object containing structured, non-contract-breaking context; it may be empty

The V1 base error codes are:

- `invalid_request`: the envelope or command parameters are invalid
- `method_not_found`: the requested method is not supported
- `handshake_required`: a command was sent before a successful `hello`
- `unsupported_protocol`: client and worker have no mutually supported protocol version
- `invalid_state`: the command is not allowed in the current worker state
- `internal_error`: the worker could not complete the request because of an unexpected failure

Later commands may define additional stable codes, such as `scan_busy`.

The scan-lifecycle and result commands additionally use:

- `invalid_session`: a session name, root, ignore pattern, or saved definition is unusable
- `session_name_conflict`: a case-insensitive session name is already in use
- `session_not_found`: the requested session ID does not exist
- `run_not_found`: the requested run ID does not exist
- `scan_busy`: another run owns the single global scan slot; `details.activeRunId` identifies it
- `invalid_cursor`: a result cursor is malformed or belongs to a different run, sort, or filter
- `duplicate_group_not_found`: the requested duplicate-file group does not belong to the given run
- `duplicate_folder_group_not_found`: the requested visible duplicate-folder group does not belong
  to the given run

### Event

```json
{"type":"event","event":"worker.ready","data":{"protocolVersion":1}}
```

Required fields:

- `type`: `"event"`
- `event`: stable event name string
- `data`: event-specific JSON object

Events have no request ID and require no acknowledgement. Responses may be interleaved with
events. Clients correlate responses only by `id`. The Milestone 0 worker emits no events; the event
envelope is reserved now for later lifecycle and progress work.

## Version Negotiation and `hello`

`hello` must be the first successfully processed request on a connection. The client lists every
major protocol version it can speak, ordered from most to least preferred. The worker selects the
first client-preferred version it supports. This worker supports version `1`.

Request:

```json
{"type":"request","id":"1","method":"hello","params":{"protocolVersions":[1],"client":{"name":"SuperDuper.Windows","version":"0.1.0"}}}
```

`params` fields:

- `protocolVersions`: required, non-empty array of positive integers
- `client.name`: required, non-empty diagnostic product name
- `client.version`: required, non-empty diagnostic version string

Successful result:

```json
{"type":"response","id":"1","ok":true,"result":{"protocolVersion":1,"workerVersion":"0.1.0","engineVersion":"0.1.0"}}
```

`result` fields:

- `protocolVersion`: selected major protocol version
- `workerVersion`: semantic version of the worker executable
- `engineVersion`: semantic version of the linked `super-duper-core` engine

If there is no common version, the worker returns `unsupported_protocol` and remains unnegotiated.
The client may send a corrected `hello` request. After a successful negotiation, another `hello`
returns `invalid_state`. Any other request before negotiation returns `handshake_required`.

Both sides must reject a successful `hello` response that selects a version the client did not
offer. Minor compatible additions use new optional fields rather than a new version. Breaking
envelope or command changes require a new major protocol version.

## Session Commands

Session and run IDs are positive JSON integers. Session names are trimmed and unique under
case-insensitive comparison. A session contains 1–64 absolute roots and at most 512 valid glob
ignore patterns. Reachable non-excluded roots are canonicalized; roots already classified inside an
effective cloud/manual exclusion remain lexical so validation cannot hydrate them. Duplicates are removed case-insensitively, and
nested roots are collapsed so a child is not scanned twice. Definitions may retain a temporarily
unreachable absolute root, but `run.start` requires at least one currently accessible directory.

Session mutations (`session.create`, `session.update`, and `session.delete`) return `invalid_state`
while a scan is active. This keeps the saved definition and the new run snapshot unambiguous.

### `session.list`

Request params are optional `offset` (default 0) and `limit` (default 100, maximum 500):

```json
{"type":"request","id":"s1","method":"session.list","params":{"offset":0,"limit":100}}
```

Result:

```json
{"sessions":[{"id":7,"name":"Photos","roots":["D:\\Photos"],"ignorePatterns":["**/node_modules/**"],"cloudPolicy":"exclude_registered_roots","manualLocationExclusions":[],"registeredCloudLocations":[{"path":"D:\\OneDrive","providerId":"OneDrive!account","displayName":"OneDrive"}],"cloudDetectionStatus":"complete","createdAt":"2026-08-15T12:00:00Z","updatedAt":"2026-08-15T12:00:00Z"}],"total":1}
```

### `session.get`

Params are `{ "sessionId": 7 }`. The result is `{ "session": <session> }` using the session shape
above.

### `session.create`

Params contain `name`, `roots`, `ignorePatterns`, and the cloud-safety snapshot supplied by Windows
Infrastructure:

```json
{"type":"request","id":"s2","method":"session.create","params":{"name":"Photos","roots":["D:\\Photos"],"ignorePatterns":["**/node_modules/**"],"cloudPolicy":"exclude_registered_roots","manualLocationExclusions":[],"registeredCloudLocations":[{"path":"D:\\OneDrive","providerId":"OneDrive!account","displayName":"OneDrive"}],"cloudDetectionStatus":"complete"}}
```

The result is `{ "session": <created-session> }`.

### `session.update`

Params contain `sessionId`, `name`, `roots`, `ignorePatterns`, `cloudPolicy`,
`manualLocationExclusions`, `registeredCloudLocations`, and `cloudDetectionStatus`. All editable
fields replace the current definition. Existing run snapshots are immutable. The result is
`{ "session": <updated-session> }`.

The registered location shape is `{ "path", "providerId", "displayName" }`. Paths must be
absolute and provider metadata is bounded. The first Milestone 7 slice executes only
`exclude_registered_roots`; `include_sync_roots_skip_placeholders` and `allow_cloud_access` are
reserved until their Windows placeholder/confirmation contracts are implemented. `run.start`
fails with `invalid_session` unless default-policy detection is `complete`.

### `session.delete`

Params are `{ "sessionId": 7 }`; the result is `{ "sessionId": 7 }`. SQLite cascade semantics
remove that session's run history and results, so deletion is rejected while any scan is active.

## Run Commands

A run response uses this shape (large byte counters are decimal strings):

```json
{
  "id":19,
  "sessionId":7,
  "parameters":{"roots":["D:\\Photos"],"ignorePatterns":[],"directorySimilarityThresholdMillis":500,"cloudPolicy":"exclude_registered_roots","manualLocationExclusions":[],"registeredCloudLocations":[],"cloudDetectionStatus":"complete"},
  "status":"running",
  "phase":"discovering",
  "createdAt":"2026-08-15T12:00:00Z",
  "startedAt":"2026-08-15T12:00:00Z",
  "completedAt":null,
  "filesDiscovered":1200,
  "bytesDiscovered":"9876543210",
  "filesHashed":400,
  "duplicateFileGroups":12,
  "duplicateFolderGroups":0,
  "wastedBytes":"1234567",
  "warningCount":2,
  "excludedSubtreeCount":1,
  "errorMessage":null,
  "engineVersion":"0.1.0"
}
```

`parameters` is the immutable snapshot used by that execution. Durable statuses and phases are the
values defined in `storage-schema-v4.md`. `excludedSubtreeCount` is distinct from recoverable
warnings and counts aggregated subtrees pruned before content access.

### `run.list`

Params contain optional `sessionId`, `offset` (default 0), and `limit` (default 100, maximum 500).
The result is `{ "runs": [<run>], "total": 1 }`, newest first. Omitting `sessionId` returns global
history.

### `run.get`

Params are `{ "runId": 19 }`; the result is `{ "run": <run> }` read from durable storage.

### `run.start`

Params are `{ "sessionId": 7 }`. The worker creates and starts the durable run before returning
`{ "run": <run> }`. At most one run may be active globally. A second start returns `scan_busy`
without creating a run or changing the active run.

### `run.cancel`

Params are `{ "runId": 19 }`. A successful request atomically signals the scan thread and moves
the durable state from `running` to `cancelling`; the immediate result is `{ "run": <run> }` with
status `cancelling`. Cancellation completes asynchronously. `run.cancelled` confirms the durable
`cancelled` terminal state, and `run.get` is authoritative after reconnecting. Cancelling a
terminal or non-active run returns `invalid_state`.

### `run_exclusion.page`

This is the bounded initial Activity-data hook for cloud/manual subtree exclusions. Params are
`runId`, optional `offset` (default 0), and optional `limit` (default 100, maximum 500):

```json
{"type":"request","id":"x1","method":"run_exclusion.page","params":{"runId":19,"offset":0,"limit":100}}
```

Result:

```json
{"exclusions":[{"id":4,"runId":19,"path":"D:\\OneDrive","reasonCode":"registered_cloud_root_excluded","providerId":"OneDrive!account","providerName":"OneDrive","occurrenceCount":1}],"total":1}
```

Rows are run-owned, ordered by path and ID, and never trigger filesystem access. Stable reason codes
introduced here are `registered_cloud_root_excluded` and `manual_location_exclusion`.

## Run Events and Ordering

The implemented lifecycle events are `run.started`, `run.progress`, `run.completed`,
`run.cancelled`, and `run.failed`. Lifecycle event data is `{ "run": <run> }`.

Progress data is:

```json
{"runId":19,"sequence":8,"status":"running","phase":"hashing","filesDiscovered":8000,"bytesDiscovered":"45000000000","filesHashed":1200,"warningCount":3,"currentPath":"D:\\Photos\\2025\\image.jpg"}
```

- `sequence` is strictly increasing within one run and establishes event order.
- `status` is `running` or `cancelling`; terminal lifecycle events carry the persisted terminal
  status.
- `phase` is `discovering`, `hashing`, `persisting`, `analyzing_folders`, or `finalizing`.
- `currentPath` and `message` are optional and intentionally throttled.
- High-frequency updates are coalesced to approximately ten events per second. Phase boundaries
  and terminal lifecycle events are emitted immediately and may form a small burst.
- Periodic SQLite counter writes occur no more than approximately twice per second. Durable phase
  boundaries and terminal transitions are written immediately.
- Events may appear before or after unrelated correlated responses. Clients must use response IDs,
  event names, run IDs, and progress sequence numbers rather than assuming request/response
  adjacency.

## Duplicate File Result Commands

Duplicate-file results are immutable and queryable only when the addressed run has status
`completed`. All commands require the run ID, so a group from one historical run cannot leak into
another run's result view. Querying a non-completed run returns `invalid_state` with the durable
status in `details.status`.

Result pages use opaque keyset cursors rather than offsets. A cursor is bound to the command, run,
group when applicable, sort, direction, and normalized filter. Clients must not inspect or modify
it. Reusing it after any query input changes returns `invalid_cursor`. `nextCursor` and
`previousCursor` are `null` at their respective boundaries. Sort ties always use group or member ID
ascending, which makes paging stable. `pageSize` defaults to 200 and must be 1–500.

### `duplicate_file_group.page`

Request:

```json
{"type":"request","id":"g1","method":"duplicate_file_group.page","params":{"runId":19,"pageSize":200,"sort":{"field":"recoverableBytes","direction":"descending"},"filter":{"search":"photos","minimumSize":"1048576","acrossDrives":true,"selectedRoot":"D:\\Photos"},"cursor":null}}
```

Allowed group sort fields are `recoverableBytes`, `groupSize`, `copyCount`, and
`representativeName`; directions are `ascending` and `descending`. The default is recoverable bytes
descending. `filter.search` is an optional case-insensitive literal substring search across member
paths, limited to 512 characters. `filter.minimumSize` is a non-negative decimal byte string and
defaults to `"0"`. `filter.acrossDrives` is an optional boolean that defaults to `false`; when
`true`, only sets with more than one distinct, non-empty, case-insensitive immutable drive label
are returned. `filter.selectedRoot` is an optional exact, case-insensitive immutable selected-root
value; when present, only sets with at least one member under that root are returned. Blank values
are treated as absent. The filter performs no filesystem access.

Result:

```json
{"groups":[{"id":31,"runId":19,"groupSize":"5242880","copyCount":3,"recoverableBytes":"10485760","representativeName":"photo.jpg","representativeType":".jpg","distinctSelectedRootCount":2,"distinctDriveCount":2}],"total":42,"summary":{"matchingGroupCount":42,"matchingCopyCount":98,"potentialRecoverableBytes":"734003200","largestRecoverableBytes":"104857600","distinctSelectedRootCount":3,"distinctDriveCount":2,"acrossDriveGroupCount":14},"nextCursor":"opaque","previousCursor":null}
```

The representative is the member with the first path under case-insensitive path ordering, then
member ID. `groupSize` and `recoverableBytes` are decimal strings.

`distinctSelectedRootCount` and `distinctDriveCount` are fixed-width counts computed from the
immutable members of that group. Blank root or drive labels do not contribute, and labels are
de-duplicated case-insensitively. A drive count greater than one identifies a cross-drive set;
zero remains valid for migrated snapshots or path types without a drive label. These fields do not
change the allowed group sorts or cursor signature.

`summary` uses the same normalized run, search, minimum-size, across-drives, and selected-root
predicate as `total`. It is computed by SQLite in the worker, not by walking client pages. The
across-drives and selected-root values are included in the opaque cursor's query signature.
`matchingGroupCount` equals `total`;
`matchingCopyCount` sums copies in the matching sets; `potentialRecoverableBytes` sums recoverable
bytes; and `largestRecoverableBytes` is the largest matching set's recoverable bytes. Both byte
fields are decimal strings. `distinctSelectedRootCount` and `distinctDriveCount` count distinct,
non-empty, case-insensitive immutable labels represented anywhere in the matching sets.
`acrossDriveGroupCount` counts matching sets that contain more than one such drive label. The
summary is repeated on each bounded page so its rows and summary share one cursor-query generation
and cannot be mixed by a late client response.

### `duplicate_file_selected_root_facet.page`

This read-only command returns selected-root values and matching-set counts without materializing
group or member pages. It applies the current group search, minimum-size, and across-drives
predicate. It intentionally does not accept or apply `selectedRoot`, so a current root selection
does not collapse the alternatives and the user can switch roots.

Request:

```json
{"type":"request","id":"rf1","method":"duplicate_file_selected_root_facet.page","params":{"runId":19,"pageSize":25,"sort":{"field":"matchingGroupCount","direction":"descending"},"filter":{"search":"photos","minimumSize":"1048576","acrossDrives":false},"cursor":null}}
```

Allowed sort fields are `matchingGroupCount` and `value`; directions are `ascending` and
`descending`. The default is matching group count descending. `pageSize` defaults to 200 and must
be 1–500. The response is keyset-paged:

```json
{"facets":[{"value":"D:\\Photos","matchingGroupCount":31}],"total":3,"nextCursor":"opaque","previousCursor":null}
```

Values are distinct, non-empty immutable `scanned_file.root_path` labels de-duplicated
case-insensitively. `matchingGroupCount` counts matching duplicate sets represented under that
root, not member copies. Values, counts, filtering, sorting, and paging are computed by SQLite in
the worker. The cursor kind is `duplicate-file-selected-root-facets`; its explicit query signature
binds the run, facet sort and direction, normalized search, minimum size, and across-drives value.
A cursor from another facet sort/filter/run or from a group/member channel returns
`invalid_cursor`.

### `duplicate_file_group.members`

Request:

```json
{"type":"request","id":"m1","method":"duplicate_file_group.members","params":{"runId":19,"groupId":31,"pageSize":200,"sort":{"field":"path","direction":"ascending"},"filter":{"search":"archive"},"cursor":null}}
```

Allowed member sort fields are `path`, `modifiedTime`, and `size`; the default is path ascending.
The optional member `filter.search` applies a case-insensitive literal substring match to the full
path and is limited to 512 characters.

Result:

```json
{"members":[{"id":88,"groupId":31,"path":"D:\\Archive\\photo.jpg","fileName":"photo.jpg","parentPath":"D:\\Archive","rootPath":"D:\\Archive","relativePath":"photo.jpg","driveLetter":"D:","size":"5242880","modifiedTimeUnixNanos":"1786795200000000000"}],"total":3,"nextCursor":null,"previousCursor":null}
```

Byte sizes and the nanosecond Unix modification timestamp are decimal strings. Member pages verify
that `groupId` belongs to `runId`; a mismatched pair returns `duplicate_group_not_found`.
`rootPath`, `relativePath`, and `driveLetter` come from the immutable scanned-file snapshot and do
not access the current filesystem. A drive label may be empty for a path type without one.

## Exact Duplicate Folder Result Commands

Exact-folder results use the same completed-run requirement, immutable run ownership, page-size
limits, opaque query-bound cursors, and stable ID tie-breaker rules as duplicate-file results.
Suppressed nested groups are retained in storage but are not returned by these V1 commands.

### `duplicate_folder_group.page`

```json
{"type":"request","id":"fg1","method":"duplicate_folder_group.page","params":{"runId":19,"pageSize":200,"sort":{"field":"totalBytes","direction":"descending"},"filter":{"search":"archive","minimumSize":"1048576"},"cursor":null}}
```

Allowed sort fields are `totalBytes`, `copyCount`, `fileCount`, and `representativePath`; the
default is total bytes descending. `filter.search` is an optional case-insensitive literal
substring across member folder paths. `filter.minimumSize` is a non-negative decimal byte string.

```json
{"groups":[{"id":41,"runId":19,"totalBytes":"5242880","descendantFileCount":18,"copyCount":2,"representativePath":"D:\\Archive\\Set A"}],"total":3,"nextCursor":null,"previousCursor":null}
```

`totalBytes` is the bytes in one folder copy, not the sum across all copies. Root names and
locations do not participate in exactness.

### `duplicate_folder_group.members`

```json
{"type":"request","id":"fm1","method":"duplicate_folder_group.members","params":{"runId":19,"groupId":41,"pageSize":200,"sort":{"field":"path","direction":"ascending"},"filter":{"search":"archive"},"cursor":null}}
```

Path is the only V1 member sort field. The optional search filter has the same 512-character limit.

```json
{"members":[{"id":101,"groupId":41,"path":"D:\\Archive\\Set A"},{"id":102,"groupId":41,"path":"E:\\Backup\\Renamed Set"}],"total":2,"nextCursor":null,"previousCursor":null}
```

Member pages verify both group and run ownership; a mismatch returns
`duplicate_folder_group_not_found`.

## Startup

1. The host starts one worker with stdin, stdout, and stderr redirected; shell execution is disabled
   and no console window is created.
2. The host immediately and asynchronously drains stdout and stderr.
3. The host sends `hello` and applies a bounded startup timeout (10 seconds by default).
4. The connection becomes ready only after a valid correlated successful `hello` response selects
   protocol version 1.
5. A launch error, timeout, premature exit, invalid stdout frame, or failed `hello` puts the client
   in a failed state. The failure includes the attempted executable path and captured stderr tail.

The worker writes nothing to stdout merely because it started. This avoids a race between an
unsolicited ready event and negotiation. A `worker.ready` event may be introduced after negotiation
in a later milestone without changing the handshake.

## Shutdown and Process Exit

For V1, graceful shutdown is signalled by the client closing the worker's stdin. On EOF, the worker
finishes writing the response for every completely received request. If a scan is active, it
signals cancellation, persists `cancelling`, waits for the scan thread to persist its terminal
state, flushes remaining protocol frames, and exits with code 0. A partial final frame is a fatal
framing error and exits non-zero.

The host waits asynchronously for a bounded grace period. If the worker does not exit, the host may
terminate only that child process tree. Host cancellation of an individual pending request stops
waiting in the host but does not imply that a future state-changing worker command was cancelled;
commands that support cancellation define their own protocol behavior.

Unexpected worker exit fails every pending request. Exit code 0 means graceful EOF shutdown only;
it does not turn incomplete commands or runs into successful operations.

## Path and Data Rules

Paths in future commands are JSON strings containing normal Windows Unicode paths. They are not
URLs and are not required to use slash normalization. The worker owns canonicalization and all
filesystem/database access. Numbers that can exceed JavaScript's exact integer range must be
encoded as decimal strings when those fields are introduced.

Fixed local roots are primary. Explicit removable, mapped-drive, and UNC roots are best-effort.
Reparse points and links are skipped. Under `exclude_registered_roots`, effective registered/manual
locations are compared lexically before `is_dir`, directory entry type, metadata, canonicalization,
stable identity, hashing, or persistence validation. A selected root inside an excluded location is
handled at the same pre-I/O boundary. Access failures, disconnects, vanished files, and files whose
size or modified time changes after discovery are recoverable warnings when a consistent run can
still be persisted; affected entries cannot create false duplicate results.

Secrets must not be placed in protocol errors. Local stderr diagnostics may contain paths needed
for troubleshooting; any future telemetry must redact sensitive path data.

## Representative Transcripts

Lines prefixed `C>` are written by the client to worker stdin. Lines prefixed `W>` are written by
the worker to stdout. The prefixes are explanatory and are not transmitted.

### Successful negotiation

```text
C> {"type":"request","id":"1","method":"hello","params":{"protocolVersions":[1],"client":{"name":"SuperDuper.Windows","version":"0.1.0"}}}
W> {"type":"response","id":"1","ok":true,"result":{"protocolVersion":1,"workerVersion":"0.1.0","engineVersion":"0.1.0"}}
```

### Unsupported protocol, then successful retry

```text
C> {"type":"request","id":"a","method":"hello","params":{"protocolVersions":[7],"client":{"name":"protocol-test","version":"1.0.0"}}}
W> {"type":"response","id":"a","ok":false,"error":{"code":"unsupported_protocol","message":"No mutually supported protocol version","retryable":false,"details":{"workerProtocolVersions":[1]}}}
C> {"type":"request","id":"b","method":"hello","params":{"protocolVersions":[1],"client":{"name":"protocol-test","version":"1.0.0"}}}
W> {"type":"response","id":"b","ok":true,"result":{"protocolVersion":1,"workerVersion":"0.1.0","engineVersion":"0.1.0"}}
```

### Command before negotiation

```text
C> {"type":"request","id":"9","method":"app.status","params":{}}
W> {"type":"response","id":"9","ok":false,"error":{"code":"handshake_required","message":"hello must succeed before other requests","retryable":false,"details":{}}}
```

### Unknown command after negotiation

```text
C> {"type":"request","id":"1","method":"hello","params":{"protocolVersions":[1],"client":{"name":"protocol-test","version":"1.0.0"}}}
W> {"type":"response","id":"1","ok":true,"result":{"protocolVersion":1,"workerVersion":"0.1.0","engineVersion":"0.1.0"}}
C> {"type":"request","id":"2","method":"not.a.command","params":{}}
W> {"type":"response","id":"2","ok":false,"error":{"code":"method_not_found","message":"Unknown method: not.a.command","retryable":false,"details":{}}}
```

Diagnostics associated with any transcript are separate, for example:

```text
stderr: worker input ended; shutting down
```
