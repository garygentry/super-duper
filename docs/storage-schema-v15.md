# Storage schema v15

Schema v15 adds `scan_run.warning_revision`, a non-negative durable revision for one run's bounded
warning snapshot. Existing v14 runs migrate with revision zero. The migration checks for the column
before adding it, so narrowly constructed historical fixtures and an interrupted/repeated open do
not create a duplicate column. Versions newer than 15 remain rejected.

The revision advances when structured aggregates are replaced, the active unclassified fallback
changes, or the warning lifecycle moves through running, cancelling, terminal completion, or
startup interruption. Ordinary progress that leaves warning rows/count/state unchanged does not
invalidate a warning cursor. Warning rows, total aggregate count, exact accounted occurrence count,
persisted run warning count, revision, and run status are read from one SQLite snapshot.

Worker `warning.page` cursors bind the exact run, sort field/direction, warning revision, and run
status. An active mutation or terminal handoff therefore returns `invalid_cursor` rather than
combining adjacent pages from different snapshots. Terminal rows remain immutable, and restart
reconstructs the latest durable interrupted or terminal snapshot without filesystem access.

The additive worker response also reports separately configured bounded diagnostic-log location
metadata. That local worker-stderr log is supplemental developer/recovery detail; it is not stored
in SQLite, is not paged as a warning occurrence, and cannot replace exact durable warning
accounting.
