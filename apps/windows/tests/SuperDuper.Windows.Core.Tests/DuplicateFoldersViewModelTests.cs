using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class DuplicateFoldersViewModelTests
{
    [TestMethod]
    public async Task CompletedRunLoadsDistinctMasterDetailAndPathActions()
    {
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage([Group(1, query.RunId, @"C:\One")], 1, null, null)),
            FolderMemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderMemberPage([new(1, query.GroupId, @"C:\One")], 1, null, null)),
        };
        var clipboard = new TestClipboard();
        var explorer = new TestExplorer();
        using var viewModel = new DuplicateFoldersViewModel(client, clipboard, explorer);

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(7, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.AreEqual(1, viewModel.Groups.Count);
        Assert.AreEqual(1, viewModel.Members.Count);
        viewModel.CopyPathCommand.Execute(viewModel.Members[0]);
        await viewModel.RevealInExplorerCommand.ExecuteAsync(viewModel.Members[0]);
        Assert.AreEqual(@"C:\One", clipboard.Text);
        Assert.AreEqual(@"C:\One", explorer.RevealedPath);
    }

    [TestMethod]
    public async Task FilterGenerationRejectsLateResponseAndCacheRemainsBounded()
    {
        var oldResponse = new TaskCompletionSource<WorkerDuplicateFolderGroupPage>(TaskCreationOptions.RunContinuationsAsynchronously);
        var observed = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) =>
            {
                if (query.Filter.Search.Length == 0 && query.Cursor is null)
                {
                    observed.TrySetResult();
                    return oldResponse.Task;
                }
                var page = query.Cursor is null ? 0 : int.Parse(query.Cursor);
                return Task.FromResult(new WorkerDuplicateFolderGroupPage(
                    [Group(page + 10, query.RunId, $@"C:\new-{page}")],
                    10,
                    page < 9 ? (page + 1).ToString() : null,
                    page > 0 ? (page - 1).ToString() : null));
            },
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), new TestExplorer());
        var initial = viewModel.ShowRunAsync(TestWorkerClient.CreateRun(8, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        await observed.Task;
        viewModel.SearchText = "new";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        oldResponse.SetResult(new WorkerDuplicateFolderGroupPage([Group(1, 8, @"C:\stale")], 1, null, null));
        await initial;
        Assert.AreEqual(@"C:\new-0", viewModel.Groups[0].RepresentativePath);

        for (var page = 1; page < 9; page++)
        {
            await viewModel.NextPageCommand.ExecuteAsync(null);
            Assert.IsTrue(viewModel.CachedGroupPageCount <= DuplicateFoldersViewModel.CacheCapacity);
        }
    }

    [TestMethod]
    public async Task ResortKeepsDisplayedResultsUntilReplacementPageArrives()
    {
        var replacement = new TaskCompletionSource<WorkerDuplicateFolderGroupPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var resortObserved = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) =>
            {
                if (query.SortDirection == WorkerSortDirection.Ascending)
                {
                    resortObserved.TrySetResult();
                    return replacement.Task;
                }
                return Task.FromResult(new WorkerDuplicateFolderGroupPage(
                    [Group(1, query.RunId, @"C:\before-sort")], 1, null, null));
            },
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(12, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        var resort = viewModel.ApplySortAsync(
            DuplicateFolderGroupSortField.TotalBytes,
            WorkerSortDirection.Ascending);
        await resortObserved.Task;

        Assert.IsTrue(viewModel.IsLoading);
        Assert.IsFalse(viewModel.IsEmpty);
        Assert.IsFalse(viewModel.IsLoadingOverlayVisible);
        Assert.AreEqual(@"C:\before-sort", viewModel.Groups.Single().RepresentativePath);

        replacement.SetResult(new WorkerDuplicateFolderGroupPage(
            [Group(2, 12, @"C:\after-sort")], 1, null, null));
        await resort;

        Assert.IsFalse(viewModel.IsLoading);
        Assert.AreEqual(@"C:\after-sort", viewModel.Groups.Single().RepresentativePath);
    }

    [TestMethod]
    public async Task NonCompletedAndEmptyRunsExposeExplicitStates()
    {
        using var viewModel = new DuplicateFoldersViewModel(new TestWorkerClient(), new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(1, 1, "running", "hashing", DateTimeOffset.UtcNow));
        Assert.IsTrue(viewModel.IsUnavailable);
        StringAssert.Contains(viewModel.StateMessage, "after this scan completes");

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(2, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        Assert.IsTrue(viewModel.IsEmpty);
    }

    private static WorkerDuplicateFolderGroup Group(long id, long runId, string path) =>
        new(id, runId, "2048", 2, 2, path);
}
