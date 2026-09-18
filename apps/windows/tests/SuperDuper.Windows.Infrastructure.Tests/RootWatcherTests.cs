namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class RootWatcherTests
{
    [TestMethod]
    public void MissingRootIsNotReportedAsLostCoverage()
    {
        var missing = Path.Combine(Path.GetTempPath(), $"super-duper-missing-root-{Guid.NewGuid():N}");

        Assert.AreEqual(WorkerClient.RootWatchOutcome.Missing, WorkerClient.TryCreateRootWatcher(missing, out var watcher));
        Assert.IsNull(watcher);
        Assert.AreEqual(
            WorkerClient.RootWatchOutcome.Missing,
            WorkerClient.TryCreateRootWatcher(@"\\?\" + missing, out _),
            "Roots are stored in verbatim form.");
    }

    [TestMethod]
    public void ExistingRootIsWatched()
    {
        var root = Directory.CreateTempSubdirectory("super-duper-watched-root-").FullName;
        try
        {
            foreach (var spelling in new[] { root, @"\\?\" + root })
            {
                Assert.AreEqual(WorkerClient.RootWatchOutcome.Watching, WorkerClient.TryCreateRootWatcher(spelling, out var watcher));
                using (watcher)
                {
                    Assert.IsNotNull(watcher);
                    Assert.IsTrue(watcher.IncludeSubdirectories);
                }
            }
        }
        finally
        {
            Directory.Delete(root);
        }
    }
}
