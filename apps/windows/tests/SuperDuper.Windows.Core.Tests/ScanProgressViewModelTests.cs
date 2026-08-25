using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class ScanProgressViewModelTests
{
    [TestMethod]
    public void TerminalRun_LabelsPhaseAsLastPhase()
    {
        using var viewModel = new ScanProgressViewModel(new TestWorkerClient(), new ImmediateDispatcher());
        viewModel.ShowRun(TestWorkerClient.CreateRun(
            1,
            1,
            "cancelled",
            "finalizing",
            DateTimeOffset.UtcNow));

        Assert.AreEqual("Cancelled", viewModel.Status);
        Assert.AreEqual("Last phase: Finalizing", viewModel.Phase);
    }

    [TestMethod]
    public void ApplyProgress_IgnoresOutOfOrderSequence()
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "discovering");
        using var viewModel = new ScanProgressViewModel(client, new ImmediateDispatcher());
        viewModel.ShowRun(run);

        viewModel.ApplyProgress(Progress(run.Id, sequence: 2, files: 20));
        viewModel.ApplyProgress(Progress(run.Id, sequence: 1, files: 10));

        Assert.AreEqual("20", viewModel.FilesDiscovered.Replace(",", ""));
    }

    [TestMethod]
    public async Task CancelCommand_ShowsCancellingBeforeWorkerConfirms()
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "hashing");
        var completion = new TaskCompletionSource<WorkerRun>(TaskCreationOptions.RunContinuationsAsynchronously);
        client.CancelHandler = (_, _) => completion.Task;
        using var viewModel = new ScanProgressViewModel(client, new ImmediateDispatcher());
        viewModel.ShowRun(run);

        var cancel = viewModel.CancelCommand.ExecuteAsync(null);

        Assert.IsTrue(viewModel.IsCancelling);
        completion.SetResult(run with { Status = "cancelling" });
        await cancel;
        Assert.AreEqual("Cancelling", viewModel.Status);
    }

    [TestMethod]
    public void ApplyProgress_ProjectsTypedFunnelRatesCacheDevicesRemainingAndEta()
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "hashing");
        using var viewModel = new ScanProgressViewModel(client, new ImmediateDispatcher());
        viewModel.ShowRun(run);

        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Hashing(run.Id)));

        Assert.AreEqual(7, viewModel.Stages.Count);
        Assert.AreEqual("Discovered", viewModel.Stages[0].Name);
        Assert.AreEqual(10UL, viewModel.Stages[0].Files);
        Assert.AreEqual("Finalized duplicates", viewModel.Stages[^1].Name);
        Assert.AreEqual(2UL, viewModel.Stages[^1].Files);
        Assert.AreEqual("10 s", viewModel.ProgressPhaseElapsed);
        Assert.AreEqual("4 files/s · 400 B/s · 10 s window", viewModel.PartialRecentRate);
        Assert.AreEqual("2 files/s · 100 B/s · 10 s window", viewModel.FullCumulativeRate);
        Assert.AreEqual("50.00% hits", viewModel.CacheEffectiveness);
        Assert.AreEqual(
            "Unavailable — scan work is not mapped to a device",
            viewModel.ActiveDevices);
        StringAssert.Contains(viewModel.RemainingWork, "4 files");
        StringAssert.Contains(viewModel.RemainingWork, "3.91 KB");
        Assert.AreEqual(
            "About 4 s remaining · 3.91 KB at 1000 B/s logical · 10 s window",
            viewModel.EstimatedTimeRemaining);
    }

    [TestMethod]
    public void ApplyProgress_RejectsWrongRunDuplicateRevisionRegressionCancellingAndTerminalRevival()
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "discovering");
        using var viewModel = new ScanProgressViewModel(client, new ImmediateDispatcher());
        viewModel.ShowRun(run);

        Assert.IsFalse(viewModel.ApplyProgress(ProgressTestData.Discovery(run.Id + 1)));
        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 1,
            revision: 1,
            discoveredFiles: 20)));
        Assert.IsFalse(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 2,
            revision: 1,
            discoveredFiles: 30,
            monotonicNanos: 2_000_000_000)));
        Assert.IsFalse(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 2,
            revision: 2,
            discoveredFiles: 10,
            monotonicNanos: 2_000_000_000)));

        viewModel.ApplyLifecycle(run with { Status = "cancelling" });
        Assert.IsFalse(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 2,
            revision: 2,
            discoveredFiles: 30,
            monotonicNanos: 2_000_000_000,
            status: "running")));
        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 2,
            revision: 2,
            discoveredFiles: 30,
            monotonicNanos: 2_000_000_000,
            status: "cancelling")));

        viewModel.ApplyLifecycle(run with { Status = "completed", CompletedAt = DateTimeOffset.UtcNow });
        Assert.IsFalse(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 3,
            revision: 3,
            discoveredFiles: 40,
            monotonicNanos: 3_000_000_000,
            status: "cancelling")));
        Assert.AreEqual("Completed", viewModel.Status);
    }

    [TestMethod]
    public void Projection_ExplainsEveryUnavailableEtaReasonAndExplicitRateUnits()
    {
        var expectations = new Dictionary<string, string>
        {
            ["work_not_yet_known"] = "Unavailable — work is not yet known",
            ["window_warming"] = "Unavailable — collecting a stable 10-second window",
            ["no_recent_progress"] = "Unavailable — no recent candidate progress",
            ["unstable_rate"] = "Unavailable — recent progress rate is unstable",
            ["not_applicable"] = "Unavailable — ETA does not apply to this phase",
        };

        foreach (var (reason, expected) in expectations)
        {
            Assert.AreEqual(
                expected,
                ScanProgressProjection.Eta(new WorkerProgressEta
                {
                    State = "unavailable",
                    Reason = reason,
                }));
        }
        Assert.AreEqual("Complete", ScanProgressProjection.Eta(new WorkerProgressEta
        {
            State = "complete",
        }));
        Assert.AreEqual(
            "Unavailable — no elapsed time",
            ScanProgressProjection.Rate(new WorkerProgressRateValue
            {
                State = "unavailable",
                Reason = "no_elapsed_time",
            }));
    }

    [TestMethod]
    public async Task LocalCancellationInvalidatesPendingGateBeforeWorkerConfirmation()
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "hashing");
        var completion = new TaskCompletionSource<WorkerRun>(TaskCreationOptions.RunContinuationsAsynchronously);
        client.CancelHandler = (_, _) => completion.Task;
        long? cancellingRunId = null;
        using var viewModel = new ScanProgressViewModel(
            client,
            new ImmediateDispatcher(),
            runId => cancellingRunId = runId);
        viewModel.ShowRun(run);

        var cancellation = viewModel.CancelCommand.ExecuteAsync(null);

        Assert.AreEqual(run.Id, cancellingRunId);
        completion.SetResult(run with { Status = "cancelling" });
        await cancellation;
    }

    private static WorkerRunProgressEventArgs Progress(long runId, ulong sequence, long files) =>
        ProgressTestData.Discovery(
            runId,
            sequence,
            revision: sequence,
            discoveredFiles: checked((ulong)files),
            monotonicNanos: sequence * 1_000_000_000);
}
