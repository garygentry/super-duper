namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WorkerClientLifecycleTests
{
    [TestMethod]
    public async Task TypedClient_CreatesSessionRunsScanAndObservesDurableCompletion()
    {
        var worker = FindWorker();
        var temp = Path.Combine(Path.GetTempPath(), $"super-duper-worker-test-{Guid.NewGuid():N}");
        var root = Path.Combine(temp, "root");
        Directory.CreateDirectory(root);
        await File.WriteAllTextAsync(Path.Combine(root, "one.txt"), "non-empty");

        try
        {
            await using var client = new WorkerClient(
                worker,
                TimeSpan.FromSeconds(10),
                Path.Combine(temp, "worker.db"));
            var terminal = new TaskCompletionSource<string>(
                TaskCreationOptions.RunContinuationsAsynchronously);
            client.RunLifecycleChanged += (_, eventArgs) =>
            {
                if (eventArgs.EventName is "run.completed" or "run.cancelled" or "run.failed")
                {
                    terminal.TrySetResult(eventArgs.EventName);
                }
            };

            _ = await client.ConnectAsync();
            var session = await client.CreateSessionAsync("Lifecycle", [root], []);
            var sessions = await client.ListSessionsAsync();
            var started = await client.StartRunAsync(session.Id);
            var terminalEvent = await terminal.Task.WaitAsync(TimeSpan.FromSeconds(10));
            var durable = await client.GetRunAsync(started.Id);

            Assert.AreEqual(1, sessions.Total);
            Assert.AreEqual("run.completed", terminalEvent);
            Assert.AreEqual("completed", durable.Status);
            Assert.AreEqual(session.Id, durable.SessionId);
        }
        finally
        {
            if (Directory.Exists(temp))
            {
                Directory.Delete(temp, recursive: true);
            }
        }
    }

    private static string FindWorker()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            var candidate = Path.Combine(directory.FullName, "target", "debug", "super-duper-worker.exe");
            if (File.Exists(candidate))
            {
                return candidate;
            }
        }

        Assert.Inconclusive("Build the Rust workspace before running Windows integration tests.");
        return string.Empty;
    }
}
