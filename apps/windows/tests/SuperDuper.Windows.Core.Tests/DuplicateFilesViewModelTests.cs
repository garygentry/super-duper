using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class DuplicateFilesViewModelTests
{
    [TestMethod]
    public async Task CompletedRunLoadsMasterDetailAndExecutesPathActions()
    {
        DuplicateFileGroupQuery? lastGroupQuery = null;
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                lastGroupQuery = query;
                return Task.FromResult(new WorkerDuplicateFileGroupPage(
                    [Group(1, query.RunId, "photo.jpg")],
                    1,
                    null,
                    null)
                {
                    Summary = new WorkerDuplicateFileReviewSummary(1, 2, "4096", "2048"),
                });
            },
            MemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileMemberPage(
                    [Member(1, query.GroupId, @"C:\Photos\photo.jpg")],
                    1,
                    null,
                    null)),
        };
        var clipboard = new TestClipboard();
        var explorer = new TestExplorer();
        using var viewModel = new DuplicateFilesViewModel(client, clipboard, explorer);

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(7, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.AreEqual(1, viewModel.Groups.Count);
        Assert.AreEqual(1, viewModel.Members.Count);
        Assert.AreEqual("photo.jpg", viewModel.Groups[0].RepresentativeName);
        Assert.AreEqual("1", viewModel.MatchingSetCountText);
        Assert.AreEqual("2", viewModel.MatchingCopyCountText);
        Assert.AreEqual("4 KB", viewModel.PotentialRecoverableText);
        Assert.AreEqual("2 KB", viewModel.LargestOpportunityText);
        Assert.AreEqual("2 selected roots · across 2 drives", viewModel.Groups[0].LocationSpan);
        Assert.AreEqual(@"C:\Photos", viewModel.Members[0].SelectedRoot);
        Assert.AreEqual("photo.jpg", viewModel.Members[0].RelativePath);
        Assert.AreEqual("C:", viewModel.Members[0].Drive);
        viewModel.CopyPathCommand.Execute(viewModel.Members[0]);
        await viewModel.RevealInExplorerCommand.ExecuteAsync(viewModel.Members[0]);
        Assert.AreEqual(@"C:\Photos\photo.jpg", clipboard.Text);
        Assert.AreEqual(@"C:\Photos\photo.jpg", explorer.RevealedPath);

        viewModel.AcrossDrives = true;
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.IsTrue(lastGroupQuery!.Filter.AcrossDrives);

        await viewModel.ClearFiltersCommand.ExecuteAsync(null);
        Assert.IsFalse(viewModel.AcrossDrives);
        Assert.IsFalse(lastGroupQuery.Filter.AcrossDrives);
    }

    [TestMethod]
    public async Task NewFilterGenerationRejectsLateOldResponse()
    {
        var oldResponse = new TaskCompletionSource<WorkerDuplicateFileGroupPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var firstRequestObserved = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                if (query.Filter.Search.Length == 0)
                {
                    firstRequestObserved.TrySetResult();
                    return oldResponse.Task;
                }
                return Task.FromResult(new WorkerDuplicateFileGroupPage(
                    [Group(2, query.RunId, "new-result.bin")],
                    1,
                    null,
                    null)
                {
                    Summary = new WorkerDuplicateFileReviewSummary(1, 2, "222", "111"),
                });
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        var initialLoad = viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(8, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        await firstRequestObserved.Task;

        viewModel.SearchText = "new";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        oldResponse.SetResult(new WorkerDuplicateFileGroupPage(
            [Group(1, 8, "stale-result.bin")],
            9,
            null,
            null)
        {
            Summary = new WorkerDuplicateFileReviewSummary(9, 18, "999", "999"),
        });
        await initialLoad;

        Assert.AreEqual(1, viewModel.Groups.Count);
        Assert.AreEqual("new-result.bin", viewModel.Groups[0].RepresentativeName);
        Assert.AreEqual("222 B", viewModel.PotentialRecoverableText);
        Assert.AreEqual("111 B", viewModel.LargestOpportunityText);
    }

    [TestMethod]
    public async Task ResortKeepsDisplayedResultsUntilReplacementPageArrives()
    {
        var replacement = new TaskCompletionSource<WorkerDuplicateFileGroupPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var resortObserved = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                if (query.SortDirection == WorkerSortDirection.Ascending)
                {
                    resortObserved.TrySetResult();
                    return replacement.Task;
                }
                return Task.FromResult(new WorkerDuplicateFileGroupPage(
                    [Group(1, query.RunId, "before-sort.bin")], 1, null, null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(11, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        var resort = viewModel.ApplySortAsync(
            DuplicateFileGroupSortField.RecoverableBytes,
            WorkerSortDirection.Ascending);
        await resortObserved.Task;

        Assert.IsTrue(viewModel.IsLoading);
        Assert.IsFalse(viewModel.IsEmpty);
        Assert.IsFalse(viewModel.IsLoadingOverlayVisible);
        Assert.AreEqual("before-sort.bin", viewModel.Groups.Single().RepresentativeName);

        replacement.SetResult(new WorkerDuplicateFileGroupPage(
            [Group(2, 11, "after-sort.bin")], 1, null, null));
        await resort;

        Assert.IsFalse(viewModel.IsLoading);
        Assert.AreEqual("after-sort.bin", viewModel.Groups.Single().RepresentativeName);
    }

    [TestMethod]
    public async Task PageCacheNeverExceedsTwoPagesOnEitherSide()
    {
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                var page = query.Cursor is null ? 0 : int.Parse(query.Cursor);
                return Task.FromResult(new WorkerDuplicateFileGroupPage(
                    [Group(page + 1, query.RunId, $"page-{page}.bin")],
                    10,
                    page < 9 ? (page + 1).ToString() : null,
                    page > 0 ? (page - 1).ToString() : null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(9, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        for (var page = 1; page < 9; page++)
        {
            await viewModel.NextPageCommand.ExecuteAsync(null);
            Assert.IsTrue(viewModel.CachedGroupPageCount <= DuplicateFilesViewModel.CacheCapacity);
            Assert.AreEqual($"page-{page}.bin", viewModel.Groups[0].RepresentativeName);
        }
    }

    [TestMethod]
    public async Task InvalidMinimumSizeAndExplorerFailureBecomeActionableStates()
    {
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([Group(1, query.RunId, "item.bin")], 1, null, null)),
            MemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileMemberPage([Member(1, query.GroupId, @"C:\Data\item.bin")], 1, null, null)),
        };
        var explorer = new TestExplorer { Error = new IOException("Explorer could not open the item.") };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), explorer);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(10, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        viewModel.MinimumSizeText = "-1";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.IsTrue(viewModel.HasError);
        StringAssert.Contains(viewModel.ErrorMessage, "non-negative");

        await viewModel.RevealInExplorerCommand.ExecuteAsync(viewModel.Members[0]);
        Assert.IsTrue(viewModel.HasDetailError);
        StringAssert.Contains(viewModel.DetailErrorMessage, "Explorer could not open");
    }

    private static WorkerDuplicateFileGroup Group(long id, long runId, string name) =>
        new(id, runId, "1024", 2, "1024", name, ".bin")
        {
            DistinctSelectedRootCount = 2,
            DistinctDriveCount = 2,
        };

    private static WorkerDuplicateFileMember Member(long id, long groupId, string path) =>
        new(id, groupId, path, Path.GetFileName(path), Path.GetDirectoryName(path)!, "1024", "1700000000000000000")
        {
            RootPath = Path.GetDirectoryName(path)!,
            RelativePath = Path.GetFileName(path),
            DriveLetter = Path.GetPathRoot(path)!.TrimEnd(Path.DirectorySeparatorChar),
        };
}
