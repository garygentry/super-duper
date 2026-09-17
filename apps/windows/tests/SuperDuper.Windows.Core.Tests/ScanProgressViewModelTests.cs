using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class ScanProgressViewModelTests
{
    [TestMethod]
    public async Task CurrentWarningEntryUsesExactRunContextAndAccessibleCount()
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "discovering") with { WarningCount = 0 };
        WorkerRun? opened = null;
        using var viewModel = new ScanProgressViewModel(
            client,
            new ImmediateDispatcher(),
            openWarnings: (target, _) =>
            {
                opened = target;
                return Task.CompletedTask;
            });

        viewModel.ShowRun(run);
        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            warningCount: 12)));

        Assert.IsTrue(viewModel.CanOpenWarnings);
        Assert.AreEqual("12", viewModel.WarningCount);
        StringAssert.Contains(viewModel.WarningAutomationName, "12 current warnings");
        StringAssert.Contains(viewModel.WarningAutomationName, "Alt+W");
        await viewModel.OpenWarningsCommand.ExecuteAsync(null);
        Assert.AreEqual(run.Id, opened?.Id);
        Assert.AreEqual(12, opened?.WarningCount);

        viewModel.ShowRun(run);
        Assert.IsFalse(viewModel.CanOpenWarnings);
        Assert.IsFalse(viewModel.OpenWarningsCommand.CanExecute(null));
    }

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
    public void Elapsed_UsesReadableDaysBeyondOneDay()
    {
        var startedAt = new DateTimeOffset(2026, 9, 1, 8, 0, 0, TimeSpan.Zero);
        var run = TestWorkerClient.CreateRun(
            1,
            1,
            "completed",
            "finalizing",
            startedAt) with
        {
            CompletedAt = startedAt.AddHours(49).AddMinutes(2).AddSeconds(3),
        };
        using var viewModel = new ScanProgressViewModel(new TestWorkerClient(), new ImmediateDispatcher());

        viewModel.ShowRun(run);

        Assert.AreEqual("2d 1h 2m", viewModel.Elapsed);
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
        Assert.AreEqual("Cancelling…", viewModel.CancelButtonText);
        Assert.AreEqual("Scan cancellation requested", viewModel.CancelAutomationName);
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

        Assert.AreEqual(6, viewModel.Stages.Count);
        Assert.AreEqual("Discovered", viewModel.Stages[0].Name);
        Assert.AreEqual("ScanStageDiscovered", viewModel.Stages[0].AutomationId);
        Assert.AreEqual(
            "Discovered: 10 files; 10000 B logical bytes",
            viewModel.Stages[0].AutomationName);
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
            "8 files · 7.81 KB candidate denominator",
            viewModel.HashPipelineCandidateContext);
        Assert.AreEqual(
            "Hash pipeline: about 4 s remaining · 3.91 KB at 1000 B/s logical · 10 s window",
            viewModel.EstimatedTimeRemaining);
        Assert.AreEqual("About 4 s for file reads", viewModel.EstimatedTimeRemainingSummary);
    }

    [TestMethod]
    public void FolderAnalysisProgress_AllowsSameSnapshotRevisionOnlyForMonotonicSubstages()
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "analyzing_folders");
        using var viewModel = new ScanProgressViewModel(client, new ImmediateDispatcher());
        viewModel.ShowRun(run);
        var hierarchy = new WorkerFolderAnalysisProgress
        {
            Substage = "hierarchy",
            Completed = 1,
            Total = 10,
        };
        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Hashing(
            run.Id,
            sequence: 1,
            revision: 7,
            legacyPhase: "analyzing_folders",
            typedPhase: "analyzing_folders",
            folderAnalysis: hierarchy)));
        Assert.AreEqual("Building hierarchy: 1 of 10", viewModel.FolderAnalysisProgress);
        Assert.IsTrue(viewModel.IsFolderAnalysis);

        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Hashing(
            run.Id,
            sequence: 2,
            revision: 7,
            legacyPhase: "analyzing_folders",
            typedPhase: "analyzing_folders",
            folderAnalysis: new WorkerFolderAnalysisProgress
            {
                Substage = "verification",
                Completed = 3,
                Total = 6,
            })));
        Assert.AreEqual("Verifying exact content: 3 of 6", viewModel.FolderAnalysisProgress);
        Assert.IsFalse(viewModel.ApplyProgress(ProgressTestData.Hashing(
            run.Id,
            sequence: 3,
            revision: 7,
            legacyPhase: "analyzing_folders",
            typedPhase: "analyzing_folders",
            folderAnalysis: hierarchy)));
    }

    [TestMethod]
    public void AcceptedProgress_CoalescesAnnouncementsAndRejectedProgressStaysSilent()
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "discovering");
        using var viewModel = new ScanProgressViewModel(client, new ImmediateDispatcher());
        viewModel.ShowRun(run);

        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 1,
            revision: 1,
            discoveredFiles: 10,
            monotonicNanos: 1_000_000_000)));
        Assert.AreEqual(1L, viewModel.ProgressAnnouncementVersion);
        StringAssert.Contains(viewModel.ProgressAnnouncement, "10 discovered");

        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 2,
            revision: 2,
            discoveredFiles: 20,
            monotonicNanos: 1_100_000_000)));
        Assert.AreEqual(1L, viewModel.ProgressAnnouncementVersion);
        StringAssert.Contains(viewModel.ProgressAnnouncement, "20 discovered");

        var latestAnnouncement = viewModel.ProgressAnnouncement;
        Assert.IsFalse(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 3,
            revision: 3,
            discoveredFiles: 19,
            monotonicNanos: 6_000_000_000)));
        Assert.AreEqual(1L, viewModel.ProgressAnnouncementVersion);
        Assert.AreEqual(latestAnnouncement, viewModel.ProgressAnnouncement);

        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 3,
            revision: 3,
            discoveredFiles: 30,
            monotonicNanos: 6_000_000_000)));
        Assert.AreEqual(2L, viewModel.ProgressAnnouncementVersion);

        viewModel.ApplyLifecycle(run with { Status = "cancelling" });
        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Discovery(
            run.Id,
            sequence: 4,
            revision: 4,
            discoveredFiles: 40,
            monotonicNanos: 6_100_000_000,
            status: "cancelling")));
        Assert.AreEqual(3L, viewModel.ProgressAnnouncementVersion);
        StringAssert.Contains(viewModel.ProgressAnnouncement, "Cancelling");
    }

    [TestMethod]
    [DataRow("cancelling", "Unavailable — cancellation is in progress", "Unavailable — cancellation is in progress", "Unavailable — cancellation is in progress")]
    [DataRow("completed", "Unavailable — no active scan I/O", "Complete", "Complete")]
    [DataRow("cancelled", "Unavailable — scan was cancelled", "Unavailable — scan was cancelled", "Unavailable — scan was cancelled")]
    [DataRow("failed", "Unavailable — scan ended before completion", "Unavailable — scan ended before completion", "Unavailable — scan ended before completion")]
    [DataRow("interrupted", "Unavailable — scan ended before completion", "Unavailable — scan ended before completion", "Unavailable — scan ended before completion")]
    public void LifecycleProjection_DoesNotPresentStaleActiveClaims(
        string status,
        string expectedDevices,
        string expectedRemaining,
        string expectedEta)
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "hashing");
        using var viewModel = new ScanProgressViewModel(client, new ImmediateDispatcher());
        viewModel.ShowRun(run);
        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Hashing(run.Id)));

        viewModel.ApplyLifecycle(run with
        {
            Status = status,
            CompletedAt = status is "completed" or "cancelled" or "failed" or "interrupted"
                ? DateTimeOffset.UtcNow
                : null,
        });

        Assert.AreEqual(expectedDevices, viewModel.ActiveDevices);
        Assert.AreEqual(expectedRemaining, viewModel.RemainingWork);
        Assert.AreEqual(expectedEta, viewModel.EstimatedTimeRemaining);
        Assert.AreEqual(6, viewModel.Stages.Count, "The last accepted funnel should remain visible.");
    }

    [TestMethod]
    public void ShowRun_ExplainsMissingDetailAndKeepsAnnouncementVersionMonotonic()
    {
        var client = new TestWorkerClient();
        var first = client.AddRun(1, "running", "discovering");
        var second = client.AddRun(2, "completed", "finalizing");
        using var viewModel = new ScanProgressViewModel(client, new ImmediateDispatcher());
        viewModel.ShowRun(first);
        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Discovery(first.Id)));
        Assert.AreEqual(1L, viewModel.ProgressAnnouncementVersion);

        viewModel.ShowRun(second);

        Assert.IsFalse(viewModel.HasDetailedProgress);
        Assert.AreEqual(0, viewModel.Stages.Count);
        StringAssert.Contains(viewModel.DetailedProgressUnavailableMessage, "completed scan");
        StringAssert.Contains(viewModel.ProgressAnnouncement, "Completed");
        Assert.AreEqual(1L, viewModel.ProgressAnnouncementVersion);

        var third = client.AddRun(3, "running", "discovering");
        viewModel.ShowRun(third);
        Assert.IsTrue(viewModel.ApplyProgress(ProgressTestData.Discovery(third.Id)));
        Assert.AreEqual(2L, viewModel.ProgressAnnouncementVersion);
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
    public void Projection_FormatsBoundedDeviceStatesWithoutExposingACollection()
    {
        Assert.AreEqual("physical:0", ScanProgressProjection.Devices(new WorkerActiveDeviceProgress
        {
            State = "one",
            DeviceKey = "physical:0",
        }));
        Assert.AreEqual("2 active devices", ScanProgressProjection.Devices(new WorkerActiveDeviceProgress
        {
            State = "multiple",
            DeviceKeys = ["physical:0", "physical:1"],
        }));
        Assert.AreEqual("Unavailable — no active I/O", ScanProgressProjection.Devices(new WorkerActiveDeviceProgress
        {
            State = "unavailable",
            Reason = "no_active_io",
        }));
        Assert.AreEqual(
            "Unavailable — active device mapping is ambiguous",
            ScanProgressProjection.Devices(new WorkerActiveDeviceProgress
            {
                State = "unavailable",
                Reason = "ambiguous",
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
