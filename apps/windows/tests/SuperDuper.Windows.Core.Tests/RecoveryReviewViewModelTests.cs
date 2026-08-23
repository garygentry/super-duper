using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class RecoveryReviewViewModelTests
{
    [TestMethod]
    public void ExposesExactlyTheFiveApprovedOperatorObservations()
    {
        CollectionAssert.AreEqual(
            new[]
            {
                "observed_in_recycle_bin",
                "observed_at_source",
                "observed_in_both",
                "observed_in_neither",
                "deferred_unresolved",
            },
            RecoveryReviewViewModel.ObservationChoices.Select(choice => choice.Value).ToArray());
    }

    [DataTestMethod]
    [DataRow("not_started", 0, "Not started")]
    [DataRow("in_progress", 1, "In progress")]
    [DataRow("review_complete_with_unresolved_evidence", 2, "Review complete with unresolved evidence")]
    public async Task PresentsEveryDerivedReviewState(
        string state,
        int observedItemCount,
        string expectedText)
    {
        var worker = RecoveryWorker();
        worker.RecoveryReviewHandler = (operationId, _) => Task.FromResult(
            ReviewResult(operationId, state, 2, observedItemCount));
        using var viewModel = new RecoveryReviewViewModel(worker);

        await viewModel.ShowOperationAsync(RecoveryOperation(8));

        StringAssert.Contains(viewModel.ReviewStatus, expectedText);
        StringAssert.Contains(viewModel.ReviewBoundary, "does not inspect");
        StringAssert.Contains(viewModel.ReviewBoundary, "original unknown, ambiguous, or recovery-required evidence");
    }

    [TestMethod]
    public async Task RestartReconstructsReviewAndAppendOnlySupersessionHistory()
    {
        var worker = RecoveryWorker();
        worker.RecoveryReviewHandler = (operationId, _) => Task.FromResult(
            ReviewResult(operationId, "review_complete_with_unresolved_evidence", 1, 1));
        worker.RecoveryReviewPageHandler = (query, _) => Task.FromResult(
            new WorkerRecoveryReviewObservationPage(
                [
                    Observation(11, query.RecycleOperationId, 41, "observed_at_source", false, supersededBy: 12),
                    Observation(12, query.RecycleOperationId, 41, "observed_in_recycle_bin", true, supersedes: 11,
                        correctionReason: "Corrected the manual selection."),
                ],
                2,
                null,
                false));
        using var viewModel = new RecoveryReviewViewModel(worker);

        await viewModel.ShowOperationAsync(RecoveryOperation(8));

        Assert.AreEqual(2, viewModel.Observations.Count);
        Assert.AreEqual("Superseded by observation 12", viewModel.Observations[0].CurrentStatus);
        StringAssert.Contains(viewModel.Observations[1].Correction, "Corrects observation 11");
        StringAssert.Contains(viewModel.Observations[1].Correction, "Corrected the manual selection");
        StringAssert.Contains(viewModel.Announcement, "Recovery review reconstructed");
        StringAssert.Contains(viewModel.HistoryPageStatus, "records 1-2 of 2");
    }

    [TestMethod]
    public async Task PagesHistoryAndRetriesTheExactFailedReadWithoutChangingTheCommittedPage()
    {
        var worker = RecoveryWorker();
        var failNext = true;
        var requestedCursors = new List<string?>();
        worker.RecoveryReviewPageHandler = (query, _) =>
        {
            requestedCursors.Add(query.Cursor);
            if (query.Cursor == "next" && failNext)
            {
                failNext = false;
                throw new InvalidOperationException("The next observation page is unavailable.");
            }
            return Task.FromResult(query.Cursor is null
                ? ObservationPage(8, 0, 100, 101, "next")
                : ObservationPage(8, 100, 1, 101, null));
        };
        using var viewModel = new RecoveryReviewViewModel(worker);
        await viewModel.ShowOperationAsync(RecoveryOperation(8));

        await viewModel.NextHistoryPageCommand.ExecuteAsync(null);

        StringAssert.Contains(viewModel.HistoryPageStatus, "records 1-100 of 101");
        StringAssert.Contains(viewModel.ReadErrorMessage, "next observation page is unavailable");
        Assert.IsTrue(viewModel.CanRetryRead);

        await viewModel.RetryReadCommand.ExecuteAsync(null);

        CollectionAssert.AreEqual(new string?[] { null, "next", "next" }, requestedCursors);
        StringAssert.Contains(viewModel.HistoryPageStatus, "records 101-101 of 101");
        Assert.IsFalse(viewModel.HasReadError);
        Assert.IsTrue(viewModel.CanMoveHistoryPrevious);
        Assert.AreEqual("history", viewModel.FocusTarget);
    }

    [TestMethod]
    public async Task FailedSummaryReadRetriesTheExactOperationRequest()
    {
        var worker = RecoveryWorker();
        var operationIds = new List<long>();
        worker.RecoveryReviewHandler = (operationId, _) =>
        {
            operationIds.Add(operationId);
            if (operationIds.Count == 1)
            {
                throw new InvalidOperationException("The recovery review summary is unavailable.");
            }
            return Task.FromResult(ReviewResult(operationId, "not_started", 1, 0));
        };
        using var viewModel = new RecoveryReviewViewModel(worker);
        await viewModel.ShowOperationAsync(RecoveryOperation(8));

        Assert.IsTrue(viewModel.CanRetryRead);
        StringAssert.Contains(viewModel.ReadErrorMessage, "summary is unavailable");

        await viewModel.RetryReadCommand.ExecuteAsync(null);

        CollectionAssert.AreEqual(new long[] { 8, 8 }, operationIds);
        Assert.AreEqual(8, viewModel.Review?.RecycleOperationId);
        Assert.IsFalse(viewModel.HasReadError);
        StringAssert.Contains(viewModel.Announcement, "reconstructed");
    }

    [TestMethod]
    public async Task CancelledReadRemainsSilentAndLateReadCannotReplaceNewerContext()
    {
        var worker = RecoveryWorker();
        var lateSummary = new TaskCompletionSource<WorkerRecoveryReviewResult>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        worker.RecoveryReviewHandler = (operationId, _) => operationId == 8
            ? lateSummary.Task
            : Task.FromResult(ReviewResult(operationId, "in_progress", 2, 1));
        using var viewModel = new RecoveryReviewViewModel(worker);
        using var cancellation = new CancellationTokenSource();
        var cancelledRead = viewModel.ShowOperationAsync(RecoveryOperation(8), cancellation.Token);
        cancellation.Cancel();
        var newerRead = viewModel.ShowOperationAsync(RecoveryOperation(9));
        await newerRead;
        var newerAnnouncement = viewModel.Announcement;
        var newerVersion = viewModel.AnnouncementVersion;

        lateSummary.SetResult(ReviewResult(8, "not_started", 1, 0));
        await cancelledRead;

        Assert.AreEqual(9, viewModel.Review?.RecycleOperationId);
        Assert.AreEqual(newerAnnouncement, viewModel.Announcement);
        Assert.AreEqual(newerVersion, viewModel.AnnouncementVersion);
        Assert.IsFalse(viewModel.HasReadError);
    }

    [TestMethod]
    public async Task RecordsAllFiveObservationsWithoutChangingOperationAuthority()
    {
        var worker = RecoveryWorker();
        var records = new List<RecoveryReviewObservationRecord>();
        worker.RecoveryReviewRecordHandler = (record, _) =>
        {
            records.Add(record);
            return Task.FromResult(new WorkerRecoveryReviewMutationResult(
                new WorkerRecoveryReview(record.RecycleOperationId, "in_progress", 5, records.Count),
                Observation(records.Count, record.RecycleOperationId, record.ItemId, record.Observation, true),
                false,
                false));
        };
        using var viewModel = new RecoveryReviewViewModel(worker);
        await viewModel.ShowOperationAsync(RecoveryOperation(8));

        foreach (var choice in RecoveryReviewViewModel.ObservationChoices)
        {
            viewModel.SelectedUnknownItem = Item(records.Count + 1, 8);
            viewModel.SelectedObservationChoice = choice;
            viewModel.Note = $"Manual note {records.Count + 1}";
            await viewModel.RecordObservationCommand.ExecuteAsync(null);
        }

        CollectionAssert.AreEqual(
            RecoveryReviewViewModel.ObservationChoices.Select(choice => choice.Value).ToArray(),
            records.Select(record => record.Observation).ToArray());
        Assert.IsTrue(records.All(record => record.EvidenceVersion == 1));
        Assert.IsTrue(records.All(record => record.SupersedesObservationId is null));
        Assert.IsTrue(records.All(record => record.CorrectionReason is null));
        Assert.IsTrue(records.All(record => record.RequestId.Length == 32));
        Assert.IsFalse(viewModel.CanRetryMutation);
        Assert.AreEqual("status", viewModel.FocusTarget);
    }

    [TestMethod]
    public async Task ExplicitCorrectionAppendsThePriorObservationAndRequiredReason()
    {
        var worker = RecoveryWorker();
        RecoveryReviewObservationRecord? submitted = null;
        var prior = Observation(11, 8, 41, "observed_at_source", true);
        var corrected = false;
        worker.RecoveryReviewPageHandler = (_, _) => Task.FromResult(corrected
            ? new WorkerRecoveryReviewObservationPage(
                [
                    prior with { IsCurrent = false, SupersededByObservationId = 12 },
                    Observation(12, 8, 41, "observed_in_recycle_bin", true, supersedes: 11,
                        correctionReason: "The earlier radio choice was incorrect."),
                ],
                2,
                null,
                false)
            : new WorkerRecoveryReviewObservationPage([prior], 1, null, false));
        worker.RecoveryReviewRecordHandler = (record, _) =>
        {
            submitted = record;
            corrected = true;
            return Task.FromResult(new WorkerRecoveryReviewMutationResult(
                new WorkerRecoveryReview(8, "review_complete_with_unresolved_evidence", 1, 1),
                Observation(12, 8, 41, record.Observation, true, supersedes: 11,
                    correctionReason: record.CorrectionReason),
                false,
                false));
        };
        using var viewModel = new RecoveryReviewViewModel(worker);
        await viewModel.ShowOperationAsync(RecoveryOperation(8));
        viewModel.SelectedUnknownItem = Item(41, 8);
        viewModel.SelectedHistoryObservation = viewModel.Observations.Single();

        viewModel.BeginCorrectionCommand.Execute(null);

        Assert.IsTrue(viewModel.IsCorrection);
        StringAssert.Contains(viewModel.CorrectionSummary, "observation 11");
        Assert.AreEqual("observation-kind", viewModel.FocusTarget);
        viewModel.SelectedObservationChoice = RecoveryReviewViewModel.ObservationChoices[0];
        Assert.IsFalse(viewModel.CanRecordObservation, "A correction reason is mandatory.");
        viewModel.CorrectionReason = "The earlier radio choice was incorrect.";
        await viewModel.RecordObservationCommand.ExecuteAsync(null);

        Assert.IsNotNull(submitted);
        Assert.AreEqual(11, submitted.SupersedesObservationId);
        Assert.AreEqual("The earlier radio choice was incorrect.", submitted.CorrectionReason);
        StringAssert.Contains(viewModel.Announcement, "appended as a correction");
        Assert.IsFalse(viewModel.IsCorrection);
        Assert.AreEqual("Superseded by observation 12", viewModel.Observations[0].CurrentStatus);
        StringAssert.Contains(viewModel.Observations[1].Correction, "The earlier radio choice was incorrect");
    }

    [TestMethod]
    public async Task FailedMutationRetriesTheExactIdempotentRequest()
    {
        var worker = RecoveryWorker();
        var attempts = new List<RecoveryReviewObservationRecord>();
        worker.RecoveryReviewRecordHandler = (record, _) =>
        {
            attempts.Add(record);
            if (attempts.Count == 1)
            {
                throw new InvalidOperationException("The observation response was interrupted.");
            }
            return Task.FromResult(new WorkerRecoveryReviewMutationResult(
                new WorkerRecoveryReview(8, "in_progress", 1, 1),
                Observation(14, 8, 41, record.Observation, true),
                true,
                false));
        };
        using var viewModel = new RecoveryReviewViewModel(worker);
        await viewModel.ShowOperationAsync(RecoveryOperation(8));
        viewModel.SelectedUnknownItem = Item(41, 8);
        viewModel.SelectedObservationChoice = RecoveryReviewViewModel.ObservationChoices[4];
        viewModel.Note = "Inspection is not currently available.";

        await viewModel.RecordObservationCommand.ExecuteAsync(null);

        Assert.IsTrue(viewModel.CanRetryMutation);
        StringAssert.Contains(viewModel.MutationErrorMessage, "response was interrupted");
        var errorVersion = viewModel.ErrorAnnouncementVersion;

        await viewModel.RetryMutationCommand.ExecuteAsync(null);

        Assert.AreEqual(2, attempts.Count);
        Assert.AreEqual(attempts[0], attempts[1], "Retry must preserve the exact request ID and payload.");
        Assert.IsFalse(viewModel.CanRetryMutation);
        Assert.IsFalse(viewModel.HasMutationError);
        Assert.AreEqual(errorVersion, viewModel.ErrorAnnouncementVersion,
            "Clearing the error must not publish a second assertive notification.");
    }

    [TestMethod]
    public async Task LateMutationCannotReplaceANewerOperationContextOrAnnounce()
    {
        var worker = RecoveryWorker();
        var late = new TaskCompletionSource<WorkerRecoveryReviewMutationResult>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        worker.RecoveryReviewRecordHandler = (_, _) => late.Task;
        using var viewModel = new RecoveryReviewViewModel(worker);
        await viewModel.ShowOperationAsync(RecoveryOperation(8));
        viewModel.SelectedUnknownItem = Item(41, 8);
        viewModel.SelectedObservationChoice = RecoveryReviewViewModel.ObservationChoices[1];
        var staleMutation = viewModel.RecordObservationCommand.ExecuteAsync(null);

        await viewModel.ShowOperationAsync(RecoveryOperation(9));
        var replacementAnnouncement = viewModel.Announcement;
        var replacementVersion = viewModel.AnnouncementVersion;
        late.SetResult(new WorkerRecoveryReviewMutationResult(
            new WorkerRecoveryReview(8, "in_progress", 1, 1),
            Observation(15, 8, 41, "observed_at_source", true),
            false,
            false));
        await staleMutation;

        Assert.AreEqual(9, viewModel.Review?.RecycleOperationId);
        Assert.AreEqual(replacementAnnouncement, viewModel.Announcement);
        Assert.AreEqual(replacementVersion, viewModel.AnnouncementVersion);
        Assert.IsFalse(viewModel.HasMutationError);
    }

    [TestMethod]
    public async Task ApprovedCopyAndNavigationActionsRemainManualAndNonMutating()
    {
        var worker = RecoveryWorker();
        var clipboard = new TestClipboard();
        var recycleBin = new TestRecycleBin();
        var freshScanCount = 0;
        using var viewModel = new RecoveryReviewViewModel(
            worker,
            clipboard,
            recycleBin,
            () =>
            {
                freshScanCount++;
                return Task.CompletedTask;
            });
        await viewModel.ShowOperationAsync(RecoveryOperation(8));
        viewModel.SelectedUnknownItem = Item(41, 8);

        viewModel.CopyEvidenceCommand.Execute(null);
        StringAssert.Contains(clipboard.Text, "Operation item 41");
        viewModel.CopyPathCommand.Execute(null);
        Assert.AreEqual(@"C:\fixture\unknown-41.bin", clipboard.Text);
        await viewModel.OpenRecycleBinCommand.ExecuteAsync(null);
        await viewModel.NavigateToFreshScanCommand.ExecuteAsync(null);

        Assert.AreEqual(1, recycleBin.OpenCount);
        Assert.AreEqual(1, freshScanCount);
        StringAssert.Contains(viewModel.Announcement, "No prior operation work was retried");
    }

    private static TestWorkerClient RecoveryWorker()
    {
        var worker = new TestWorkerClient();
        worker.RecoveryReviewHandler = (operationId, _) => Task.FromResult(
            ReviewResult(operationId, "not_started", 1, 0));
        worker.RecoveryReviewPageHandler = (_, _) => Task.FromResult(
            new WorkerRecoveryReviewObservationPage([], 0, null, false));
        return worker;
    }

    private static WorkerRecoveryReviewResult ReviewResult(
        long operationId,
        string state,
        long unknownItemCount,
        long observedItemCount) =>
        new(new WorkerRecoveryReview(operationId, state, unknownItemCount, observedItemCount), false);

    private static WorkerRecycleOperation RecoveryOperation(long id) =>
        TestWorkerClient.CreateRecycleOperation(id, 12, 7, 4) with
        {
            Status = "recovery_required",
            UnknownCount = 1,
        };

    private static RecycleOperationItemViewModel Item(long id, long operationId) =>
        new(new WorkerRecycleOperationItem(
            id, operationId, 1, id - 1, id, null, "file", $@"C:\fixture\unknown-{id}.bin",
            null, null, null, id, null, "4096", "pending", null, "unknown", "worker_interrupted",
            null, null, null));

    private static WorkerRecoveryReviewObservation Observation(
        long id,
        long operationId,
        long itemId,
        string kind,
        bool isCurrent,
        long? supersedes = null,
        long? supersededBy = null,
        string? correctionReason = null) =>
        new(
            id,
            $"request-{id}",
            operationId,
            itemId,
            kind,
            "2026-08-23T18:00:00.0000000+00:00",
            "Manual observation",
            1,
            supersedes,
            correctionReason,
            "2026-08-23T18:00:01.000Z",
            supersededBy,
            isCurrent);

    private static WorkerRecoveryReviewObservationPage ObservationPage(
        long operationId,
        int offset,
        int count,
        int total,
        string? nextCursor) =>
        new(
            Enumerable.Range(offset + 1, count)
                .Select(id => Observation(id, operationId, id, "deferred_unresolved", true))
                .ToArray(),
            total,
            nextCursor,
            false);

    private sealed class TestRecycleBin : IRecycleBinService
    {
        public int OpenCount { get; private set; }

        public Task OpenAsync(CancellationToken cancellationToken = default)
        {
            OpenCount++;
            return Task.CompletedTask;
        }
    }
}
