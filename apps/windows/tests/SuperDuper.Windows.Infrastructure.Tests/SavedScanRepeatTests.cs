using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class SavedScanRepeatTests
{
    [TestMethod]
    public async Task ReopenedCacheAndEditedSetupCreateNewRunsWithoutRewritingHistory()
    {
        var worker = FindWorker();
        var temp = Directory.CreateTempSubdirectory("super-duper-repeat-").FullName;
        var root = Directory.CreateDirectory(Path.Combine(temp, "files")).FullName;
        var firstPath = Path.Combine(root, "first.bin");
        var copyPath = Path.Combine(root, "copy.bin");
        var bytes = Enumerable.Range(0, 256 * 1024).Select(i => (byte)(i % 251)).ToArray();
        await File.WriteAllBytesAsync(firstPath, bytes);
        await File.WriteAllBytesAsync(copyPath, bytes);
        WorkerClient Open(string log) => new(worker, TimeSpan.FromSeconds(15), Path.Combine(temp, "history.db"),
            Path.Combine(temp, "logs", log), Path.Combine(temp, "hash-cache"));
        try
        {
            WorkerSessionDefinition session;
            WorkerRun original;
            WorkerDuplicateFileGroupPage originalGroups;
            WorkerDuplicateFileMemberPage originalMembers;
            await using (var client = Open("first.log"))
            {
                await client.ConnectAsync();
                session = await client.CreateSessionAsync("Saved repeat fixture", [root], []);
                original = await Scan(client, session.Id, RepeatCachePolicyNames.ReuseVerified);
                originalGroups = await Groups(client, original.Id);
                Assert.AreEqual(1, originalGroups.Total);
                originalMembers = await Members(client, original.Id, originalGroups.Groups.Single().Id);
                Assert.AreEqual(2, originalMembers.Total);
                await client.SetReviewDecisionAsync("retain-decision", original.Id, originalGroups.Groups.Single().Id,
                    originalMembers.Members[0].Id, "keep", 0);
            }
            // A new worker reopens exactly the same history and cache; no truncation or cache reset.
            await using (var client = Open("reopened.log"))
            {
                await client.ConnectAsync();
                var repeated = await Scan(client, session.Id, RepeatCachePolicyNames.ReuseVerified);
                var reuse = await client.GetPerformanceSnapshotAsync(productRunId: repeated.Id);
                Assert.IsFalse(reuse.ExecutorEnabled);
                Assert.IsTrue(reuse.Counters.Single(c => c.Metric == "partial_hash_cache_hits").Value > 0);
                Assert.IsTrue(reuse.Counters.Single(c => c.Metric == "full_hash_cache_hits").Value > 0);
                Assert.AreNotEqual(original.Id, repeated.Id);
                Assert.IsNotNull(repeated.StartedAt);

                // Change only disposable fixture membership and saved exclusions, retaining both old runs.
                File.Delete(copyPath);
                await File.WriteAllBytesAsync(Path.Combine(root, "added.bin"), bytes);
                await client.UpdateSessionAsync(session.Id, "Edited repeat fixture", [root], ["**/added.bin"]);
                var edited = await Scan(client, session.Id, RepeatCachePolicyNames.ReuseVerified);
                Assert.AreEqual(0, (await Groups(client, edited.Id)).Total);
                Assert.AreEqual(1, (await Groups(client, original.Id)).Total);
                Assert.AreEqual(1, (await Groups(client, repeated.Id)).Total);
                Assert.AreEqual(0, (await client.GetRunAsync(original.Id)).Parameters.IgnorePatterns.Count);
                CollectionAssert.AreEqual(new[] { "**/added.bin" }, edited.Parameters.IgnorePatterns.ToArray());
                var retained = await Members(client, original.Id, originalGroups.Groups.Single().Id);
                CollectionAssert.AreEquivalent(originalMembers.Members.Select(m => m.Id).ToArray(), retained.Members.Select(m => m.Id).ToArray());
                Assert.AreEqual("keep", retained.Members.Single(m => m.Id == originalMembers.Members[0].Id).Decision);

                await client.UpdateSessionAsync(session.Id, "Edited repeat fixture", [root], []);
                var reread = await Scan(client, session.Id, RepeatCachePolicyNames.RevalidateContent);
                Assert.AreEqual(1, (await Groups(client, reread.Id)).Total);
                Assert.AreEqual(RepeatCachePolicyNames.RevalidateContent, (await client.GetRunAsync(reread.Id)).Parameters.RepeatCachePolicy);
                var reads = await client.GetPerformanceSnapshotAsync(productRunId: reread.Id);
                Assert.AreEqual(0UL, reads.Counters.Single(c => c.Metric == "full_hash_cache_hits").Value);
                Assert.AreEqual(4, (await client.ListRunsAsync(session.Id)).Total);
            }
        }
        finally { await TestDirectoryCleanup.DeleteAsync(temp); }
    }

    private static async Task<WorkerRun> Scan(WorkerClient client, long sessionId, string policy)
    {
        var terminal = new TaskCompletionSource<WorkerRun>(TaskCreationOptions.RunContinuationsAsynchronously);
        void OnLifecycle(object? sender, WorkerRunLifecycleEventArgs args)
        {
            if (args.Run.Status is "completed" or "failed" or "cancelled") terminal.TrySetResult(args.Run);
        }
        client.RunLifecycleChanged += OnLifecycle;
        try
        {
            var started = await client.StartRunAsync(sessionId, policy);
            var completed = await terminal.Task.WaitAsync(TimeSpan.FromSeconds(30));
            Assert.AreEqual(started.Id, completed.Id);
            Assert.AreEqual("completed", completed.Status, completed.ErrorMessage);
            return await client.GetRunAsync(started.Id);
        }
        finally { client.RunLifecycleChanged -= OnLifecycle; }
    }

    private static Task<WorkerDuplicateFileGroupPage> Groups(WorkerClient client, long runId) =>
        client.GetDuplicateFileGroupsAsync(new DuplicateFileGroupQuery(runId, 25,
            DuplicateFileGroupSortField.RecoverableBytes, WorkerSortDirection.Descending, new DuplicateFileGroupFilter("", "0")));

    private static Task<WorkerDuplicateFileMemberPage> Members(WorkerClient client, long runId, long groupId) =>
        client.GetDuplicateFileGroupMembersAsync(new DuplicateFileMemberQuery(runId, groupId, 25,
            DuplicateFileMemberSortField.Path, WorkerSortDirection.Ascending, new DuplicateFileMemberFilter("")));

    private static string FindWorker()
    {
#if DEBUG
        const string profile = "debug";
#else
        const string profile = "release";
#endif
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory); directory is not null; directory = directory.Parent)
        {
            var path = Path.Combine(directory.FullName, "target", profile, "super-duper-worker.exe");
            if (File.Exists(path)) return path;
        }
        Assert.Inconclusive("Build the paired Rust worker before running this isolated fixture.");
        return "";
    }
}
