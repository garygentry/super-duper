using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class RecycleOperationViewModelTests
{
    [TestMethod]
    public async Task ReconstructsAccessibleReadOnlySummaryWithoutEnablingSubmission()
    {
        var worker = new TestWorkerClient();
        var operation = TestWorkerClient.CreateRecycleOperation(8, 12, 7, 4);
        worker.LatestRecycleOperationHandler = (_, _) => Task.FromResult<WorkerRecycleOperation?>(operation);
        worker.RecycleOperationItemPageHandler = (query, _) => Task.FromResult(
            new WorkerRecycleOperationItemPage(
                [CreateItem(1, query.RecycleOperationId, @"C:\fixture\copy.bin")],
                1,
                null));
        using var viewModel = new RecycleOperationViewModel(worker, new DisabledCapability());

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            12, 1, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.AreEqual(8, viewModel.Operation?.Id);
        Assert.AreEqual(1, viewModel.Items.Count);
        Assert.IsFalse(viewModel.IsExecutorEnabled);
        Assert.IsFalse(viewModel.CanSubmit);
        StringAssert.Contains(viewModel.ConfirmationSummary, "Windows Recycle Bin");
        StringAssert.Contains(viewModel.ConfirmationSummary, "excluded locations remain untouched");
        StringAssert.Contains(viewModel.ConfirmationSummary, "partial or ambiguous results");
        StringAssert.Contains(viewModel.BoundaryNotice, "execution is disabled");
        Assert.IsTrue(viewModel.AnnouncementVersion > 0);
    }

    [TestMethod]
    public async Task RejectsStaleGenerationAndKeepsOnlyBoundedPages()
    {
        var worker = new TestWorkerClient();
        var stale = new TaskCompletionSource<WorkerRecycleOperation?>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        worker.LatestRecycleOperationHandler = (runId, cancellationToken) => runId == 1
            ? stale.Task.WaitAsync(cancellationToken)
            : Task.FromResult<WorkerRecycleOperation?>(
                TestWorkerClient.CreateRecycleOperation(22, runId, 9, 3));
        var pageCalls = 0;
        worker.RecycleOperationItemPageHandler = (query, _) =>
        {
            pageCalls++;
            var page = query.Cursor is null ? 0 : int.Parse(query.Cursor);
            var next = page < 6 ? (page + 1).ToString() : null;
            return Task.FromResult(new WorkerRecycleOperationItemPage(
                [CreateItem(page + 1, query.RecycleOperationId, $@"C:\fixture\copy-{page}.bin")],
                7,
                next));
        };
        using var viewModel = new RecycleOperationViewModel(worker, new DisabledCapability());

        var first = viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            1, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            2, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        stale.TrySetResult(TestWorkerClient.CreateRecycleOperation(11, 1, 8, 2));
        await first;
        for (var page = 0; page < 6; page++)
        {
            await viewModel.NextPageCommand.ExecuteAsync(null);
        }
        for (var page = 0; page < 6; page++)
        {
            await viewModel.PreviousPageCommand.ExecuteAsync(null);
        }

        Assert.AreEqual(22, viewModel.Operation?.Id);
        Assert.AreEqual(1, viewModel.Items.Count);
        Assert.IsTrue(pageCalls >= 8, "Evicted pages should be fetched again after the five-page cache bound.");
    }

    private static WorkerRecycleOperationItem CreateItem(long id, long operationId, string path) =>
        new(
            id, operationId, 1, id - 1, id, null, "file", path, 1, null, null, id,
            null, "4096", "pending", null, "pending", null, null, null, null);

    private sealed class DisabledCapability : IRecycleOperationCapabilityExecutor
    {
        public bool IsEnabled => false;

        public Task<IReadOnlyList<RecycleEligibilityObservation>> InspectAsync(
            IReadOnlyList<WorkerRecycleOperationItem> items,
            CancellationToken cancellationToken = default) =>
            Task.FromResult<IReadOnlyList<RecycleEligibilityObservation>>([]);

        public Task<RecycleBatchExecutionResult> ExecuteBatchAsync(
            WorkerRecycleOperationBatch batch,
            Func<CancellationToken, Task> acknowledgeShellStart,
            CancellationToken cancellationToken = default) =>
            throw new InvalidOperationException("disabled test executor");
    }
}
