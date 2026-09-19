using System.Diagnostics;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Infrastructure.Tests;

// Starts real worker processes; see MSTestSettings.cs.
[TestClass]
[DoNotParallelize]
public sealed class WorkerDatabaseUnavailableTests
{
    [TestMethod]
    public async Task ConnectAsync_ReportsNewerDatabaseWithoutChangingIt()
    {
        var worker = FindWorker();
        var temp = Directory.CreateTempSubdirectory("super-duper-newer-db-").FullName;
        try
        {
            var database = Path.Combine(temp, "super_duper.db");
            await using (var creator = Open(worker, temp, database))
            {
                _ = await creator.ConnectAsync();
            }
            SetUserVersion(database, 99);
            var before = await File.ReadAllBytesAsync(database);

            await using var client = Open(worker, temp, database);
            var exception = await Assert.ThrowsExactlyAsync<WorkerDatabaseUnavailableException>(
                () => client.ConnectAsync());

            Assert.AreEqual(WorkerDatabaseUnavailableException.NewerVersion, exception.Reason);
            Assert.AreEqual(database, exception.DatabasePath);
            StringAssert.Contains(exception.WorkerMessage, "newer than supported");
            Assert.IsNull(client.OwnedProcessId);
            CollectionAssert.AreEqual(before, await File.ReadAllBytesAsync(database));
        }
        finally
        {
            await TestDirectoryCleanup.DeleteAsync(temp);
        }
    }

    [TestMethod]
    public async Task ConnectAsync_SecondClientOnSameDatabaseIsInUseAndOwnerKeepsWorking()
    {
        var worker = FindWorker();
        var temp = Directory.CreateTempSubdirectory("super-duper-in-use-").FullName;
        try
        {
            var database = Path.Combine(temp, "super_duper.db");
            await using var owner = Open(worker, temp, database);
            _ = await owner.ConnectAsync();
            var session = await owner.CreateSessionAsync("Owned", [temp], []);

            await using var second = Open(worker, temp, database);
            var exception = await Assert.ThrowsExactlyAsync<WorkerDatabaseUnavailableException>(
                () => second.ConnectAsync());

            Assert.AreEqual(WorkerDatabaseUnavailableException.InUse, exception.Reason);
            Assert.AreEqual("Super Duper is already open", exception.Describe().Title);
            var sessions = await owner.ListSessionsAsync();
            Assert.AreEqual(session.Id, sessions.Sessions.Single().Id);
            Assert.IsNotNull(owner.OwnedProcessId);
            Assert.IsFalse(Process.GetProcessById(owner.OwnedProcessId.Value).HasExited);
        }
        finally
        {
            await TestDirectoryCleanup.DeleteAsync(temp);
        }
    }

    private static WorkerClient Open(string worker, string temp, string database) => new(
        worker,
        TimeSpan.FromSeconds(15),
        database,
        Path.Combine(temp, "logs", "worker.log"),
        Path.Combine(temp, "hash-cache"));

    private static void SetUserVersion(string database, int version)
    {
        using var stream = File.Open(database, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
        stream.Position = 60;
        stream.Write([(byte)(version >> 24), (byte)(version >> 16), (byte)(version >> 8), (byte)version]);
    }

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
