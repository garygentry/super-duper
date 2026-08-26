using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class RunWarningDrilldownViewModelTests
{
    [TestMethod]
    public async Task ActiveRefreshRebindsRevisionAndKeepsPageAndCacheBounded()
    {
        var worker = new TestWorkerClient();
        var revision = 4L;
        var queries = new List<RunWarningQuery>();
        worker.RunWarningsHandler = (query, _) =>
        {
            queries.Add(query);
            var pageIndex = query.Cursor is null ? 0 : int.Parse(query.Cursor[7..]);
            var firstId = (revision * 1_000) + (pageIndex * RunWarningDrilldownViewModel.PageSize) + 1;
            return Task.FromResult(Page(
                query.RunId,
                revision,
                "active",
                "running",
                firstId,
                RunWarningDrilldownViewModel.PageSize,
                100_000,
                100_000,
                $"cursor-{pageIndex + 1}"));
        };
        using var viewModel = new RunWarningDrilldownViewModel(worker);

        await viewModel.OpenAsync(31);
        for (var index = 0; index < 6; index++)
        {
            await viewModel.LoadNextPageAsync();
        }

        Assert.AreEqual(RunWarningDrilldownViewModel.PageSize, viewModel.Warnings.Count);
        Assert.AreEqual(RunWarningDrilldownViewModel.CachePageLimit, viewModel.CachedPageCount);
        Assert.IsTrue(queries.All(query => query.PageSize == RunWarningDrilldownViewModel.PageSize));
        Assert.AreEqual(4L, viewModel.SnapshotRevision);
        Assert.IsTrue(viewModel.IsActiveSnapshot);

        revision = 5;
        await viewModel.RefreshAsync();

        Assert.AreEqual(5L, viewModel.SnapshotRevision);
        Assert.AreEqual(5_001, viewModel.Warnings[0].Id);
        Assert.AreEqual(1, viewModel.CachedPageCount);
        Assert.AreEqual(100_000, viewModel.WarningCount);
        Assert.AreEqual(viewModel.WarningCount, viewModel.AccountedWarningCount);
        StringAssert.Contains(viewModel.StatusMessage, "100,000 of 100,000 warnings durably accounted");
        Assert.IsNull(queries[^1].Cursor, "An active refresh reused a cached first page.");
    }

    [TestMethod]
    public async Task RunChangeCancelsLateGenerationAndPagingRejectsRevisionMix()
    {
        var worker = new TestWorkerClient();
        var late = new TaskCompletionSource<WorkerRunWarningPage>(TaskCreationOptions.RunContinuationsAsynchronously);
        CancellationToken firstToken = default;
        worker.RunWarningsHandler = (query, token) =>
        {
            if (query.RunId == 41)
            {
                firstToken = token;
                return late.Task;
            }
            return Task.FromResult(Page(42, 8, "active", "running", 1, 1, 1, 1, "cursor-1"));
        };
        using var viewModel = new RunWarningDrilldownViewModel(worker);

        var oldLoad = viewModel.OpenAsync(41);
        await viewModel.OpenAsync(42);
        late.SetResult(Page(41, 7, "active", "running", 91, 1, 1, 1, null));
        await oldLoad;

        Assert.IsTrue(firstToken.IsCancellationRequested);
        Assert.AreEqual(42L, viewModel.RunId);
        Assert.AreEqual(42, viewModel.Warnings.Single().RunId);
        Assert.AreEqual(1, viewModel.Warnings.Single().Id);

        worker.RunWarningsHandler = (_, _) => Task.FromResult(
            Page(42, 9, "active", "running", 2, 1, 1, 1, null));
        await viewModel.LoadNextPageAsync();

        Assert.IsTrue(viewModel.HasError);
        StringAssert.Contains(viewModel.ErrorMessage, "changed snapshot revision or state");
        Assert.AreEqual(8L, viewModel.SnapshotRevision);
        Assert.AreEqual(1, viewModel.Warnings.Single().Id, "A mixed-revision page replaced the accepted page.");
    }

    [TestMethod]
    public async Task NewInstanceReconstructsInterruptedTerminalSnapshot()
    {
        var worker = new TestWorkerClient
        {
            RunWarningsHandler = (_, _) => Task.FromResult(
                Page(51, 17, "terminal", "interrupted", 700, 2, 2, 9, null)),
        };

        using var reconstructed = new RunWarningDrilldownViewModel(worker);
        await reconstructed.OpenAsync(51);

        Assert.IsTrue(reconstructed.IsTerminalSnapshot);
        Assert.AreEqual("interrupted", reconstructed.RunStatus);
        Assert.AreEqual(17L, reconstructed.SnapshotRevision);
        Assert.AreEqual(9, reconstructed.WarningCount);
        Assert.AreEqual(9, reconstructed.AccountedWarningCount);
        Assert.AreEqual(2, reconstructed.Warnings.Count);
        StringAssert.Contains(reconstructed.StatusMessage, "Terminal interrupted snapshot revision 17");
    }

    [TestMethod]
    public async Task TerminalHandoffIsOneWayAndPreservesCompletedHistory()
    {
        var worker = new TestWorkerClient();
        var responses = new Queue<WorkerRunWarningPage>(
        [
            Page(61, 21, "active", "running", 1, 1, 1, 1, null),
            Page(61, 22, "terminal", "completed", 2, 1, 1, 1, null),
            Page(61, 23, "active", "running", 3, 1, 1, 1, null),
        ]);
        worker.RunWarningsHandler = (_, _) => Task.FromResult(responses.Dequeue());
        using var viewModel = new RunWarningDrilldownViewModel(worker);

        await viewModel.OpenAsync(61);
        Assert.IsTrue(viewModel.IsActiveSnapshot);
        await viewModel.RefreshAsync();

        Assert.IsTrue(viewModel.IsTerminalSnapshot);
        Assert.AreEqual(22L, viewModel.SnapshotRevision);
        Assert.AreEqual(2, viewModel.Warnings.Single().Id);

        await viewModel.ApplySortAsync(RunWarningSortField.Phase, WorkerSortDirection.Ascending);

        Assert.IsTrue(viewModel.HasError);
        StringAssert.Contains(viewModel.ErrorMessage, "cannot return to an active state");
        Assert.IsTrue(viewModel.IsTerminalSnapshot);
        Assert.AreEqual(22L, viewModel.SnapshotRevision);
        Assert.AreEqual(2, viewModel.Warnings.Single().Id, "A stale active page revived completed history.");
    }

    private static WorkerRunWarningPage Page(
        long runId,
        long revision,
        string state,
        string status,
        long firstId,
        int rowCount,
        long total,
        long warningCount,
        string? nextCursor) => new(
            Enumerable.Range(0, rowCount)
                .Select(offset => new WorkerRunWarningAggregate(
                    firstId + offset,
                    runId,
                    "hashing",
                    "scan",
                    $"warning-{firstId + offset}",
                    "warning",
                    $"Warning {firstId + offset}",
                    1,
                    [$"Example {firstId + offset}"]))
                .ToArray(),
            total,
            warningCount,
            warningCount,
            revision,
            state,
            status,
            TestWorkerClient.DiagnosticLog,
            nextCursor,
            false);
}
