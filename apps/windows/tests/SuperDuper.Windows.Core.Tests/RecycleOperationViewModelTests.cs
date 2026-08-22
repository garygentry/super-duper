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
        StringAssert.Contains(viewModel.CancellationDisclosure, "No Shell work has started");
        Assert.IsFalse(viewModel.HasRecoveryGuidance);
        Assert.IsTrue(viewModel.AnnouncementVersion > 0);
    }

    [TestMethod]
    public async Task ExplainsActiveCancellationBoundaryAndAmbiguousRecoveryWithoutOfferingRetry()
    {
        var worker = new TestWorkerClient();
        var operation = TestWorkerClient.CreateRecycleOperation(8, 12, 7, 4) with
        {
            Status = "executing",
        };
        worker.LatestRecycleOperationHandler = (_, _) => Task.FromResult<WorkerRecycleOperation?>(operation);
        using var viewModel = new RecycleOperationViewModel(worker, new DisabledCapability());

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            12, 1, "completed", "finalizing", DateTimeOffset.UtcNow));

        StringAssert.Contains(viewModel.CancellationDisclosure, "current Windows Shell item may finish");
        StringAssert.Contains(viewModel.CancellationDisclosure, "Already recycled items are not restored");
        Assert.IsFalse(viewModel.CanSubmit);

        worker.LatestRecycleOperationHandler = (_, _) => Task.FromResult<WorkerRecycleOperation?>(
            operation with
            {
                Status = "recovery_required",
                UnknownCount = 1,
                SubmittedAt = "2026-08-22T20:01:02.000Z",
                CancellationRequested = true,
                ErrorCode = "worker_interrupted",
                ErrorDetail = "Shell result reporting was interrupted.",
            });
        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            12, 1, "completed", "finalizing", DateTimeOffset.UtcNow));

        StringAssert.Contains(viewModel.CancellationDisclosure, "Do not retry");
        StringAssert.Contains(viewModel.RecoveryGuidance, "every unknown item");
        StringAssert.Contains(viewModel.RecoveryGuidance, "cannot resolve or replay");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Operation key: recycle-operation-8");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Evidence record: 8");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Run: 12");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Preflight: 7");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Policy version: 1");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Preflight snapshot signature:");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Intent signature:");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Prepared at:");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Submitted at: 2026-08-22T20:01:02.000Z");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Completed at: none recorded");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Cancellation requested: true");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "unknown: 1");
        StringAssert.Contains(viewModel.RecoveryEvidenceSummary, "Error code: worker_interrupted");
        Assert.IsFalse(viewModel.RecoveryEvidenceSummary.Contains(@"C:\", StringComparison.Ordinal));
        Assert.IsFalse(viewModel.RecoveryEvidenceSummary.Contains("Error detail", StringComparison.Ordinal));
        Assert.IsTrue(viewModel.HasRecoveryGuidance);
        Assert.IsFalse(viewModel.CanSubmit);
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
