using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class PreflightViewModelTests
{
    [TestMethod]
    public async Task StartConfirmsNonDeletingReadsPollsTerminalAndLoadsBoundedDetail()
    {
        var worker = new TestWorkerClient();
        var run = TestWorkerClient.CreateRun(7, 1, "completed", "finalizing", DateTimeOffset.UtcNow);
        worker.ReviewPlanHandler = (_, _) => Task.FromResult(Review(7, 4, 2));
        worker.LatestPreflightHandler = (_, _) => Task.FromResult<WorkerPreflight?>(null);
        var started = TestWorkerClient.CreatePreflight(12, 7, "running", 4, 0, 2, 0);
        var completed = TestWorkerClient.CreatePreflight(12, 7, "completed", 4, 2, 2, 2);
        worker.PreflightStartHandler = (_, requestedRun, revision, _) =>
        {
            Assert.AreEqual(7, requestedRun);
            Assert.AreEqual(4, revision);
            return Task.FromResult(new WorkerPreflightStartResult(started, false));
        };
        worker.PreflightHandler = (_, _) => Task.FromResult(completed);
        worker.PreflightItemPageHandler = (query, _) => Task.FromResult(new WorkerPreflightItemPage(
            [new WorkerPreflightItem(
                1, query.PreflightId, 0, "file", "remove", 3, null, null, 9, null,
                @"C:\fixture\remove.bin", "ready", "matched_snapshot", "100", 10, null,
                DateTimeOffset.UtcNow.ToString("O"), 1)],
            1,
            null));
        var confirmation = new RecordingConfirmation(true);
        using var viewModel = new PreflightViewModel(worker, confirmation);

        await viewModel.ShowRunAsync(run);
        Assert.IsTrue(viewModel.CanStart);
        await viewModel.StartCommand.ExecuteAsync(null);

        Assert.AreEqual("Run preflight validation?", confirmation.Title);
        StringAssert.Contains(confirmation.Message, "complete file content");
        StringAssert.Contains(confirmation.Message, "No files will be deleted");
        Assert.AreEqual("completed", viewModel.Preflight?.Status);
        Assert.AreEqual(1, viewModel.Items.Count);
        Assert.AreEqual("Ready", viewModel.Items[0].Outcome);
        Assert.AreEqual("summary", viewModel.FocusTarget);
        StringAssert.Contains(viewModel.Announcement, "Ready 2");
        Assert.IsFalse(viewModel.CanCancel);
    }

    [TestMethod]
    public async Task DeclinedConfirmationDoesNotStartPreflight()
    {
        var worker = new TestWorkerClient();
        var run = TestWorkerClient.CreateRun(8, 1, "completed", "finalizing", DateTimeOffset.UtcNow);
        worker.ReviewPlanHandler = (_, _) => Task.FromResult(Review(8, 2, 1));
        worker.LatestPreflightHandler = (_, _) => Task.FromResult<WorkerPreflight?>(null);
        var starts = 0;
        worker.PreflightStartHandler = (_, _, _, _) =>
        {
            starts++;
            return Task.FromResult(new WorkerPreflightStartResult(
                TestWorkerClient.CreatePreflight(1, 8, "running", 2), false));
        };
        using var viewModel = new PreflightViewModel(worker, new RecordingConfirmation(false));
        await viewModel.ShowRunAsync(run);

        await viewModel.StartCommand.ExecuteAsync(null);

        Assert.AreEqual(0, starts);
        Assert.IsNull(viewModel.Preflight);
    }

    [TestMethod]
    public async Task ReviewRevisionRefreshInvalidatesHistoricalPreflightWithoutMutatingIt()
    {
        var worker = new TestWorkerClient();
        var run = TestWorkerClient.CreateRun(9, 1, "completed", "finalizing", DateTimeOffset.UtcNow);
        var revision = 3L;
        worker.ReviewPlanHandler = (_, _) => Task.FromResult(Review(9, revision, 1));
        var historical = TestWorkerClient.CreatePreflight(20, 9, "completed", 3);
        worker.LatestPreflightHandler = (_, _) => Task.FromResult<WorkerPreflight?>(historical);
        worker.PreflightItemPageHandler = (_, _) =>
            Task.FromResult(new WorkerPreflightItemPage([], 0, null));
        worker.PreflightHandler = (_, _) => Task.FromResult(historical with
        {
            CurrentReviewRevision = revision,
            IsCurrent = revision == historical.ReviewRevision,
        });
        using var viewModel = new PreflightViewModel(worker, new RecordingConfirmation(true));
        await viewModel.ShowRunAsync(run);
        Assert.IsTrue(viewModel.IsCurrent);

        revision = 4;
        await viewModel.RefreshReviewRevisionAsync(9, 4);

        Assert.IsFalse(viewModel.IsCurrent);
        Assert.AreEqual(3, viewModel.Preflight?.ReviewRevision);
        StringAssert.Contains(viewModel.RevisionStatus, "current review revision is 4");
        StringAssert.Contains(viewModel.Announcement, "Run preflight again");
    }

    private static WorkerReviewPlanView Review(long runId, long revision, long removals) =>
        new(
            new WorkerReviewPlan(1, runId, "active", revision, "created", "updated"),
            new WorkerReviewPlanSummary(1, 1, removals, 0, "100", 1)
            {
                EffectiveRemovalFileCount = removals,
                PlannedRemovalPhysicalItemCount = removals,
            });

    private sealed class RecordingConfirmation(bool answer) : IUserConfirmationService
    {
        public string? Title { get; private set; }

        public string? Message { get; private set; }

        public Task<bool> ConfirmAsync(
            string title,
            string message,
            CancellationToken cancellationToken = default)
        {
            Title = title;
            Message = message;
            return Task.FromResult(answer);
        }
    }
}
