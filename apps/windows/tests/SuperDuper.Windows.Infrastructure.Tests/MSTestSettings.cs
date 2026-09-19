// Tests run in parallel, except those that start real worker processes: they are marked
// [DoNotParallelize] and run one at a time after the parallel batch. Several concurrent workers,
// each creating databases and scanning fixtures, starved the CI runner intermittently: hello
// handshakes timed out and SQLite reported locked or unopenable files. The app itself only ever
// runs one worker per data folder.
[assembly: Parallelize(Scope = ExecutionScope.MethodLevel)]
