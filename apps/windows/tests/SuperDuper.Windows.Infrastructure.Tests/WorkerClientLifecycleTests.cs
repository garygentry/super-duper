using SuperDuper.Windows.Core.Workers;

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
        await File.WriteAllTextAsync(Path.Combine(root, "one-copy.txt"), "non-empty");
        var folderA = Directory.CreateDirectory(Path.Combine(root, "folder-a"));
        var folderB = Directory.CreateDirectory(Path.Combine(root, "folder-b"));
        await File.WriteAllTextAsync(Path.Combine(folderA.FullName, "same.txt"), "folder content");
        await File.WriteAllTextAsync(Path.Combine(folderB.FullName, "same.txt"), "folder content");
        var diagnostics = Path.Combine(temp, "logs", "worker.log");

        try
        {
            await using var client = new WorkerClient(
                worker,
                TimeSpan.FromSeconds(10),
                Path.Combine(temp, "worker.db"),
                diagnostics);
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
            var groups = await client.GetDuplicateFileGroupsAsync(
                new DuplicateFileGroupQuery(
                    started.Id,
                    200,
                    DuplicateFileGroupSortField.RecoverableBytes,
                    WorkerSortDirection.Descending,
                    new DuplicateFileGroupFilter(string.Empty, "0")));
            var members = await client.GetDuplicateFileGroupMembersAsync(
                new DuplicateFileMemberQuery(
                    started.Id,
                    groups.Groups.Single(group => group.RepresentativeName.StartsWith("one", StringComparison.Ordinal)).Id,
                    200,
                    DuplicateFileMemberSortField.Path,
                    WorkerSortDirection.Ascending,
                    new DuplicateFileMemberFilter(string.Empty)));
            var folderGroups = await client.GetDuplicateFolderGroupsAsync(
                new DuplicateFolderGroupQuery(
                    started.Id,
                    200,
                    DuplicateFolderGroupSortField.TotalBytes,
                    WorkerSortDirection.Descending,
                    new DuplicateFolderGroupFilter(string.Empty, "0")));
            var folderMembers = await client.GetDuplicateFolderGroupMembersAsync(
                new DuplicateFolderMemberQuery(
                    started.Id,
                    folderGroups.Groups.Single().Id,
                    200,
                    DuplicateFolderMemberSortField.Path,
                    WorkerSortDirection.Ascending,
                    new DuplicateFolderMemberFilter(string.Empty)));

            Assert.AreEqual(1, sessions.Total);
            Assert.AreEqual("run.completed", terminalEvent);
            Assert.AreEqual("completed", durable.Status);
            Assert.AreEqual(session.Id, durable.SessionId);
            Assert.AreEqual(2, groups.Total);
            Assert.IsTrue(members.Total >= 2);
            CollectionAssert.IsSubsetOf(
                new[] { "one.txt", "one-copy.txt" },
                members.Members.Select(member => member.FileName).ToArray());
            Assert.AreEqual(1, folderGroups.Total);
            Assert.AreEqual(2, folderMembers.Total);
            CollectionAssert.AreEquivalent(
                new[] { folderA.Name, folderB.Name },
                folderMembers.Members
                    .Select(member => Path.GetFileName(member.Path.TrimEnd(Path.DirectorySeparatorChar)))
                    .ToArray());

            var diagnosticText = await WaitForDiagnosticsAsync(diagnostics);
            foreach (var phase in new[] { "discovering", "hashing", "persisting", "analyzing_folders", "finalizing" })
            {
                StringAssert.Contains(diagnosticText, $"kind=scan_phase run_id={started.Id} phase={phase}");
            }
            foreach (var method in new[]
                     {
                         "duplicate_file_group.page",
                         "duplicate_file_group.members",
                         "duplicate_folder_group.page",
                         "duplicate_folder_group.members",
                     })
            {
                StringAssert.Contains(diagnosticText, $"kind=result_query method={method}");
            }
        }
        finally
        {
            if (Directory.Exists(temp))
            {
                Directory.Delete(temp, recursive: true);
            }
        }
    }

    private static async Task<string> WaitForDiagnosticsAsync(string path)
    {
        var deadline = DateTime.UtcNow + TimeSpan.FromSeconds(5);
        while (DateTime.UtcNow < deadline)
        {
            if (File.Exists(path))
            {
                await using var stream = new FileStream(
                    path,
                    FileMode.Open,
                    FileAccess.Read,
                    FileShare.ReadWrite | FileShare.Delete,
                    bufferSize: 4096,
                    useAsync: true);
                using var reader = new StreamReader(stream);
                var text = await reader.ReadToEndAsync();
                if (text.Contains("duplicate_folder_group.members", StringComparison.Ordinal))
                {
                    return text;
                }
            }
            await Task.Delay(50);
        }
        Assert.Fail($"Timed out waiting for worker diagnostics at {path}.");
        return string.Empty;
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
