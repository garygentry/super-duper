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
        RecycleOperationItemQuery? recoveryQuery = null;
        worker.RecycleOperationItemPageHandler = (query, _) =>
        {
            recoveryQuery = query;
            return Task.FromResult(new WorkerRecycleOperationItemPage(
                [CreateItem(2, query.RecycleOperationId, @"C:\fixture\unknown.bin") with
                {
                    ResultStatus = "unknown",
                }],
                1,
                null));
        };
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
        Assert.AreEqual("unknown", recoveryQuery?.ResultStatus);
        StringAssert.Contains(viewModel.PageStatus, "showing items 1-1 of 1 unknown details");
        StringAssert.Contains(viewModel.Items[0].EvidenceDetails, "Operation item 2; preflight item 2; batch 1");
        StringAssert.Contains(viewModel.Items[0].EvidenceDetails, "result unknown");
        StringAssert.Contains(viewModel.Items[0].EvidenceDetails, "Shell HRESULT none recorded");
        StringAssert.Contains(viewModel.Items[0].EvidenceDetails, "recycled item present unknown");
        Assert.IsFalse(viewModel.RecoveryEvidenceSummary.Contains(@"C:\", StringComparison.Ordinal));
        Assert.IsFalse(viewModel.RecoveryEvidenceSummary.Contains("Error detail", StringComparison.Ordinal));
        Assert.IsTrue(viewModel.HasRecoveryGuidance);
        Assert.IsFalse(viewModel.CanSubmit);
    }

    [TestMethod]
    public void FormatsDurableItemCorrelationAndNumericShellEvidence()
    {
        var item = CreateItem(3, 8, @"C:\fixture\failed.bin") with
        {
            GroupId = 17,
            SnapshotFileId = 23,
            ResultStatus = "failed",
            ResultCode = "sharing_violation",
            ShellHresult = unchecked((int)0x80270027),
            RecycledItemPresent = false,
            ResultAt = "2026-08-22T21:15:00.000Z",
        };

        var viewModel = new RecycleOperationItemViewModel(item);

        StringAssert.Contains(viewModel.EvidenceDetails, "group 17");
        StringAssert.Contains(viewModel.EvidenceDetails, "snapshot file 23");
        StringAssert.Contains(viewModel.EvidenceDetails, "code sharing_violation");
        StringAssert.Contains(viewModel.EvidenceDetails, "Shell HRESULT 0x80270027");
        StringAssert.Contains(viewModel.EvidenceDetails, "recycled item present false");
        StringAssert.Contains(viewModel.EvidenceDetails, "result time 2026-08-22T21:15:00.000Z");
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

    [TestMethod]
    public async Task ReportsAndAnnouncesExactRecoveryPageRange()
    {
        var worker = new TestWorkerClient();
        worker.LatestRecycleOperationHandler = (_, _) => Task.FromResult<WorkerRecycleOperation?>(
            TestWorkerClient.CreateRecycleOperation(8, 12, 7, 4) with
            {
                Status = "recovery_required",
                UnknownCount = 101,
            });
        worker.RecycleOperationItemPageHandler = (query, _) => Task.FromResult(
            query.Cursor is null
                ? new WorkerRecycleOperationItemPage(
                    Enumerable.Range(1, 100)
                        .Select(id => CreateItem(id, query.RecycleOperationId, $@"C:\fixture\unknown-{id}.bin"))
                        .ToArray(),
                    101,
                    "next")
                : new WorkerRecycleOperationItemPage(
                    [CreateItem(101, query.RecycleOperationId, @"C:\fixture\unknown-101.bin")],
                    101,
                    null));
        using var viewModel = new RecycleOperationViewModel(worker, new DisabledCapability());

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            12, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        var initialAnnouncementVersion = viewModel.AnnouncementVersion;

        StringAssert.Contains(viewModel.PageStatus, "showing items 1-100 of 101 unknown details");

        await viewModel.NextPageCommand.ExecuteAsync(null);

        StringAssert.Contains(viewModel.PageStatus, "showing items 101-101 of 101 unknown details");
        StringAssert.Contains(viewModel.Announcement, viewModel.PageStatus);
        Assert.IsTrue(viewModel.AnnouncementVersion > initialAnnouncementVersion);
    }

    [TestMethod]
    public async Task PreviousRecoveryPageRepeatsItsAnnouncement()
    {
        var worker = CreatePagedRecoveryWorker();
        using var viewModel = new RecycleOperationViewModel(worker, new DisabledCapability());

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            12, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        await viewModel.NextPageCommand.ExecuteAsync(null);
        var nextPageAnnouncement = viewModel.Announcement;
        var nextPageAnnouncementVersion = viewModel.AnnouncementVersion;

        await viewModel.PreviousPageCommand.ExecuteAsync(null);

        StringAssert.Contains(viewModel.PageStatus, "showing items 1-100 of 101 unknown details");
        StringAssert.Contains(viewModel.Announcement, viewModel.PageStatus);
        Assert.AreNotEqual(nextPageAnnouncement, viewModel.Announcement);
        Assert.IsTrue(viewModel.AnnouncementVersion > nextPageAnnouncementVersion);
    }

    [TestMethod]
    public async Task StaleOrCancelledPagingDoesNotAnnounce()
    {
        var worker = CreatePagedRecoveryWorker();
        var pendingPage = new TaskCompletionSource<WorkerRecycleOperationItemPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        worker.RecycleOperationItemPageHandler = (query, _) => query.Cursor is null
            ? Task.FromResult(CreateRecoveryPage(query.RecycleOperationId, firstPage: true))
            : pendingPage.Task;
        using var viewModel = new RecycleOperationViewModel(worker, new DisabledCapability());

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            12, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        var announcementVersion = viewModel.AnnouncementVersion;
        var stalePage = viewModel.NextPageCommand.ExecuteAsync(null);

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            13, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        var replacementAnnouncement = viewModel.Announcement;
        pendingPage.SetResult(CreateRecoveryPage(8, firstPage: false));
        await stalePage;

        Assert.AreEqual(replacementAnnouncement, viewModel.Announcement);
        Assert.AreEqual(announcementVersion + 1, viewModel.AnnouncementVersion,
            "Only the replacement run load should announce; the stale page must remain silent.");

        var cancelledPage = new TaskCompletionSource<WorkerRecycleOperationItemPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        worker.RecycleOperationItemPageHandler = (query, _) => query.Cursor is null
            ? Task.FromResult(CreateRecoveryPage(query.RecycleOperationId, firstPage: true))
            : cancelledPage.Task;
        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            12, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        var announcement = viewModel.Announcement;
        announcementVersion = viewModel.AnnouncementVersion;
        var cancelled = viewModel.NextPageCommand.ExecuteAsync(null);

        viewModel.Dispose();
        cancelledPage.SetResult(CreateRecoveryPage(8, firstPage: false));
        await cancelled;

        Assert.AreEqual(announcement, viewModel.Announcement);
        Assert.AreEqual(announcementVersion, viewModel.AnnouncementVersion);
    }

    [TestMethod]
    public async Task ChangingContextClearsStaleAnnouncementWithoutPublishingAnotherNotification()
    {
        var worker = CreatePagedRecoveryWorker();
        using var viewModel = new RecycleOperationViewModel(worker, new DisabledCapability());

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            12, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        var announcementVersion = viewModel.AnnouncementVersion;

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            13, 1, "running", "hashing", DateTimeOffset.UtcNow));

        Assert.IsFalse(viewModel.HasOperation);
        Assert.AreEqual(0, viewModel.Items.Count);
        Assert.AreEqual(string.Empty, viewModel.Announcement);
        Assert.AreEqual(announcementVersion, viewModel.AnnouncementVersion,
            "Clearing stale automation text must not announce the empty replacement context.");
    }

    [TestMethod]
    public async Task FailedNextPagePreservesCommittedPageAndCanRetry()
    {
        var worker = CreatePagedRecoveryWorker();
        var failNext = true;
        worker.RecycleOperationItemPageHandler = (query, _) =>
        {
            if (query.Cursor is not null && failNext)
            {
                failNext = false;
                throw new InvalidOperationException("The next recovery page is unavailable.");
            }
            return Task.FromResult(CreateRecoveryPage(query.RecycleOperationId, query.Cursor is null));
        };
        using var viewModel = new RecycleOperationViewModel(worker, new DisabledCapability());

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            12, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        await viewModel.NextPageCommand.ExecuteAsync(null);

        StringAssert.Contains(viewModel.PageStatus, "showing items 1-100 of 101 unknown details");
        StringAssert.Contains(viewModel.ErrorMessage, "next recovery page is unavailable");
        StringAssert.Contains(viewModel.ErrorAnnouncement, "page error");
        Assert.IsTrue(viewModel.ErrorAnnouncementVersion > 0);
        Assert.IsTrue(viewModel.CanMoveNext);
        Assert.IsFalse(viewModel.CanMovePrevious);

        await viewModel.NextPageCommand.ExecuteAsync(null);

        StringAssert.Contains(viewModel.PageStatus, "showing items 101-101 of 101 unknown details");
        Assert.IsFalse(viewModel.HasError);
        Assert.IsTrue(viewModel.CanMovePrevious);
    }

    [TestMethod]
    public async Task FailedPreviousPageAfterCacheEvictionPreservesCommittedPageAndCanRetry()
    {
        var worker = CreatePagedRecoveryWorker();
        var failPrevious = false;
        worker.RecycleOperationItemPageHandler = (query, _) =>
        {
            var pageIndex = query.Cursor is null ? 0 : int.Parse(query.Cursor);
            if (pageIndex == 1 && failPrevious)
            {
                failPrevious = false;
                throw new InvalidOperationException("The previous recovery page is unavailable.");
            }
            return Task.FromResult(CreateRecoveryPage(query.RecycleOperationId, pageIndex, 7));
        };
        using var viewModel = new RecycleOperationViewModel(worker, new DisabledCapability());

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(
            12, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        for (var page = 1; page < 7; page++)
        {
            await viewModel.NextPageCommand.ExecuteAsync(null);
        }
        failPrevious = true;
        for (var page = 6; page > 2; page--)
        {
            await viewModel.PreviousPageCommand.ExecuteAsync(null);
        }

        await viewModel.PreviousPageCommand.ExecuteAsync(null);

        StringAssert.Contains(viewModel.PageStatus, "showing items 201-300 of 700 unknown details");
        StringAssert.Contains(viewModel.ErrorMessage, "previous recovery page is unavailable");
        StringAssert.Contains(viewModel.ErrorAnnouncement, "page error");
        Assert.IsTrue(viewModel.ErrorAnnouncementVersion > 0);
        Assert.IsTrue(viewModel.CanMoveNext);
        Assert.IsTrue(viewModel.CanMovePrevious);

        await viewModel.PreviousPageCommand.ExecuteAsync(null);

        StringAssert.Contains(viewModel.PageStatus, "showing items 101-200 of 700 unknown details");
        Assert.IsFalse(viewModel.HasError);
        Assert.IsTrue(viewModel.CanMoveNext);
        Assert.IsTrue(viewModel.CanMovePrevious);
    }

    private static TestWorkerClient CreatePagedRecoveryWorker()
    {
        var worker = new TestWorkerClient();
        worker.LatestRecycleOperationHandler = (_, _) => Task.FromResult<WorkerRecycleOperation?>(
            TestWorkerClient.CreateRecycleOperation(8, 12, 7, 4) with
            {
                Status = "recovery_required",
                UnknownCount = 101,
            });
        worker.RecycleOperationItemPageHandler = (query, _) => Task.FromResult(
            CreateRecoveryPage(query.RecycleOperationId, query.Cursor is null));
        return worker;
    }

    private static WorkerRecycleOperationItemPage CreateRecoveryPage(long operationId, bool firstPage) =>
        firstPage
            ? new WorkerRecycleOperationItemPage(
                Enumerable.Range(1, 100)
                    .Select(id => CreateItem(id, operationId, $@"C:\fixture\unknown-{id}.bin"))
                    .ToArray(),
                101,
                "next")
            : new WorkerRecycleOperationItemPage(
                [CreateItem(101, operationId, @"C:\fixture\unknown-101.bin")],
                101,
                null);

    private static WorkerRecycleOperationItemPage CreateRecoveryPage(
        long operationId,
        int pageIndex,
        int pageCount) =>
        new(
            Enumerable.Range((pageIndex * 100) + 1, 100)
                .Select(id => CreateItem(id, operationId, $@"C:\fixture\unknown-{id}.bin"))
                .ToArray(),
            pageCount * 100,
            pageIndex + 1 < pageCount ? (pageIndex + 1).ToString() : null);

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
