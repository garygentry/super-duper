using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WorkerStateLocationsTests
{
    private static readonly string LocalAppData = Path.Combine(Path.GetTempPath(), "sd-local-app-data");

    [TestMethod]
    public void Resolve_WithoutOverridesPlacesAllStateUnderLocalAppData()
    {
        var state = WorkerStateLocations.Resolve(_ => null, LocalAppData);

        var directory = Path.Combine(LocalAppData, "SuperDuper");
        Assert.AreEqual(directory, state.StateDirectory);
        Assert.AreEqual(Path.Combine(directory, "super_duper.db"), state.DatabasePath);
        Assert.AreEqual(Path.Combine(directory, "scan_status.db"), state.StatusDatabasePath);
        Assert.AreEqual(Path.Combine(directory, "content_hash_cache.db"), state.HashCachePath);
        Assert.IsTrue(state.CreateStateDirectory);
    }

    [TestMethod]
    public void Resolve_WithDatabaseOverrideKeepsOtherStateBesideThatDatabase()
    {
        var database = Path.Combine(Path.GetTempPath(), "disposable", "run.db");

        var state = WorkerStateLocations.Resolve(
            Environment(("SUPER_DUPER_DB_PATH", database)),
            LocalAppData);

        var directory = Path.Combine(Path.GetTempPath(), "disposable");
        Assert.AreEqual(directory, state.StateDirectory);
        Assert.AreEqual(database, state.DatabasePath);
        Assert.AreEqual(Path.Combine(directory, "scan_status.db"), state.StatusDatabasePath);
        Assert.AreEqual(Path.Combine(directory, "content_hash_cache.db"), state.HashCachePath);
        Assert.IsFalse(state.CreateStateDirectory);
    }

    [TestMethod]
    public void Resolve_HonorsEachOverrideAsFullPath()
    {
        var root = Path.Combine(Path.GetTempPath(), "sd-overrides");

        var state = WorkerStateLocations.Resolve(
            Environment(
                ("SUPER_DUPER_DB_PATH", Path.Combine(root, "db", "..", "db", "main.db")),
                ("SUPER_DUPER_STATUS_DB_PATH", Path.Combine(root, "status", "status.db")),
                ("HASH_CACHE_PATH", Path.Combine(root, "cache"))),
            LocalAppData);

        Assert.AreEqual(Path.Combine(root, "db", "main.db"), state.DatabasePath);
        Assert.AreEqual(Path.Combine(root, "status", "status.db"), state.StatusDatabasePath);
        Assert.AreEqual(Path.Combine(root, "cache"), state.HashCachePath);
    }

    [TestMethod]
    public void Resolve_WithOnlyCacheOverrideAndBlankDatabaseKeepsDatabaseUnderLocalAppData()
    {
        var cache = Path.Combine(Path.GetTempPath(), "sd-cache-only");

        var state = WorkerStateLocations.Resolve(
            Environment(("HASH_CACHE_PATH", cache), ("SUPER_DUPER_DB_PATH", "   ")),
            LocalAppData);

        Assert.AreEqual(Path.Combine(LocalAppData, "SuperDuper", "super_duper.db"), state.DatabasePath);
        Assert.AreEqual(cache, state.HashCachePath);
        Assert.IsTrue(state.CreateStateDirectory);
    }

    [TestMethod]
    public async Task Scan_CreatesMissingDefaultStateDirectoryAndKeepsAllStateThere()
    {
        var worker = FindWorker();
        var temp = Path.Combine(Path.GetTempPath(), $"super-duper-state-dir-{Guid.NewGuid():N}");
        try
        {
            var root = Directory.CreateDirectory(Path.Combine(temp, "files")).FullName;
            var bytes = Enumerable.Range(0, 8192).Select(i => (byte)(i % 251)).ToArray();
            await File.WriteAllBytesAsync(Path.Combine(root, "a.bin"), bytes);
            await File.WriteAllBytesAsync(Path.Combine(root, "b.bin"), bytes);
            var state = WorkerStateLocations.Resolve(_ => null, Path.Combine(temp, "LocalAppData"));
            Assert.IsFalse(Directory.Exists(state.StateDirectory));

            await using (var client = new WorkerClient(worker, state))
            {
                _ = await client.ConnectAsync();
                var session = await client.CreateSessionAsync("State directory fixture", [root], []);
                var terminal = new TaskCompletionSource<WorkerRun>(TaskCreationOptions.RunContinuationsAsynchronously);
                client.RunLifecycleChanged += (_, args) =>
                {
                    if (args.Run.Status is "completed" or "failed" or "cancelled") terminal.TrySetResult(args.Run);
                };
                _ = await client.StartRunAsync(session.Id, RepeatCachePolicyNames.ReuseVerified);
                var completed = await terminal.Task.WaitAsync(TimeSpan.FromSeconds(30));
                Assert.AreEqual("completed", completed.Status, completed.ErrorMessage);
            }

            Assert.IsTrue(File.Exists(state.DatabasePath), "Main database was not created in the state directory.");
            Assert.IsTrue(File.Exists(state.StatusDatabasePath), "Status database was not created in the state directory.");
            Assert.IsTrue(Directory.Exists(state.HashCachePath), "Hash cache was not created in the state directory.");
        }
        finally
        {
            if (Directory.Exists(temp))
            {
                await TestDirectoryCleanup.DeleteAsync(temp);
            }
        }
    }

    private static Func<string, string?> Environment(params (string Name, string Value)[] values) =>
        name => values.FirstOrDefault(value => value.Name == name).Value;

    private static string FindWorker()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            var candidate = Path.Combine(directory.FullName, "target", BuildProfile, "super-duper-worker.exe");
            if (File.Exists(candidate))
            {
                return candidate;
            }
        }

        Assert.Inconclusive("Build the Rust workspace before running Windows integration tests.");
        return string.Empty;
    }

#if DEBUG
    private const string BuildProfile = "debug";
#else
    private const string BuildProfile = "release";
#endif
}
