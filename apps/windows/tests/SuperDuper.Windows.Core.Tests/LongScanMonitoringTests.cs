using System.Collections.Concurrent;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class LongScanMonitoringTests
{
    [TestMethod]
    public void ClockAdvancesDaysWithoutFabricatingProgressOrAnnouncements()
    {
        var clock = new ManualProgressClock();
        using var model = new ScanProgressViewModel(new TestWorkerClient(), new ImmediateDispatcher(), clock: clock);
        var run = TestWorkerClient.CreateRun(1, 1, "running", "discovering", clock.GetUtcNow());
        model.ShowRun(run);
        StringAssert.Contains(model.UpdateFreshness, "Waiting for the first");
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Discovery()));
        var snapshot = model.ProgressSnapshot;
        var announcement = model.ProgressAnnouncementVersion;
        clock.Advance(TimeSpan.FromDays(2) + TimeSpan.FromHours(7) + TimeSpan.FromMinutes(14));
        Assert.AreEqual("2d 7h 14m", model.Elapsed);
        StringAssert.Contains(model.UpdateFreshness, "2d 7h 14m ago");
        StringAssert.Contains(model.UpdateFreshness, "does not indicate failure");
        Assert.AreSame(snapshot, model.ProgressSnapshot);
        Assert.AreEqual(announcement, model.ProgressAnnouncementVersion);
        Assert.IsFalse(model.HasError);
        Assert.IsTrue(model.CanCancel);
        Assert.AreEqual("Unavailable — work is not yet known", model.EstimatedTimeRemaining);
        clock.AdjustUtc(TimeSpan.FromHours(-1));
        StringAssert.Contains(model.UpdateFreshness, "2d 7h 14m ago", "Receipt age uses monotonic local time.");
    }

    [TestMethod]
    public void OnlyAcceptedFramesRefreshReceiptEvenWhenCountersDoNotChange()
    {
        var clock = new ManualProgressClock();
        using var model = new ScanProgressViewModel(new TestWorkerClient(), new ImmediateDispatcher(), clock: clock);
        model.ShowRun(TestWorkerClient.CreateRun(1, 1, "running", "discovering", clock.GetUtcNow()));
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Discovery(discoveredFiles: 10)));
        clock.Advance(TimeSpan.FromSeconds(38));
        var stale = model.UpdateFreshness;
        Assert.IsFalse(model.ApplyProgress(ProgressTestData.Discovery())); // Duplicate sequence.
        Assert.IsFalse(model.ApplyProgress(ProgressTestData.Discovery(runId: 2, sequence: 2, revision: 2)));
        Assert.IsFalse(model.ApplyProgress(ProgressTestData.Discovery(sequence: 2, revision: 2, status: "unsupported")));
        Assert.IsFalse(model.ApplyProgress(ProgressTestData.Discovery(sequence: 2, revision: 2, discoveredFiles: 9)));
        model.ApplyLifecycle(TestWorkerClient.CreateRun(2, 1, "running", "hashing", clock.GetUtcNow()));
        Assert.AreEqual(stale, model.UpdateFreshness);
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Discovery(sequence: 2, revision: 2,
            discoveredFiles: 10, monotonicNanos: 39_000_000_000)));
        Assert.AreEqual("Last update 0s ago", model.UpdateFreshness);
        Assert.AreEqual("10", model.FilesDiscovered);
    }

    [DataTestMethod]
    [DataRow("completed")]
    [DataRow("cancelled")]
    [DataRow("failed")]
    [DataRow("interrupted")]
    public void TerminalStateFreezesDurationAndRetainsHistoricalMetricsWithoutRevival(string status)
    {
        var clock = new ManualProgressClock();
        using var model = new ScanProgressViewModel(new TestWorkerClient(), new ImmediateDispatcher(), clock: clock);
        var run = TestWorkerClient.CreateRun(1, 1, "running", "hashing", clock.GetUtcNow());
        model.ShowRun(run);
        model.ApplyProgress(ProgressTestData.Hashing());
        var snapshot = model.ProgressSnapshot;
        clock.Advance(TimeSpan.FromHours(49));
        model.ApplyLifecycle(model.Run! with { Status = status, CompletedAt = null,
            ErrorMessage = status == "failed" ? "Fixture failure" : null });
        var announcement = model.ProgressAnnouncementVersion;
        Assert.AreEqual(2L, announcement);
        Assert.AreEqual("Last reported activity", model.ActivityHeading);
        Assert.AreEqual("Last reported scan path", model.ActivityPathAutomationName);
        Assert.AreEqual(@"C:\Data\candidate.bin", model.CurrentPath);
        StringAssert.Contains(model.MetricsContext, "Historical metrics");
        Assert.AreSame(snapshot, model.ProgressSnapshot);
        Assert.IsFalse(model.IsIndeterminate);
        Assert.IsFalse(model.CanCancel);
        Assert.AreEqual(status == "failed", model.HasError);
        Assert.AreEqual("Elapsed at last observation", model.ElapsedLabel);
        clock.Advance(TimeSpan.FromDays(3));
        model.ApplyLifecycle(run); // Late lifecycle must not restart animation or the timer.
        Assert.IsFalse(model.ApplyProgress(ProgressTestData.Hashing(sequence: 2, revision: 2)));
        Assert.AreEqual("2d 1h 0m", model.Elapsed);
        Assert.AreEqual(status, model.Run!.Status);
        Assert.AreEqual(announcement, model.ProgressAnnouncementVersion);
        StringAssert.Contains(model.UpdateFreshness, "No live updates");
        // Reconciled durable terminal totals can still replace a provisional stop observation.
        model.ApplyLifecycle(model.Run! with { CompletedAt = run.StartedAt!.Value.AddHours(50), WarningCount = 7 });
        Assert.AreEqual("2d 2h 0m", model.Elapsed);
        Assert.AreEqual("7", model.WarningCount);
        Assert.IsFalse(model.IsIndeterminate);
    }

    [TestMethod]
    public void FreshWorkerUpdateAndNoCandidateProgressRemainSeparateObservations()
    {
        var clock = new ManualProgressClock();
        using var model = new ScanProgressViewModel(new TestWorkerClient(), new ImmediateDispatcher(), clock: clock);
        model.ShowRun(TestWorkerClient.CreateRun(1, 1, "running", "hashing", clock.GetUtcNow()));
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(etaUnavailableReason: "no_recent_progress")));
        Assert.AreEqual("Last update 0s ago", model.UpdateFreshness);
        Assert.AreEqual("Unavailable — no recent candidate progress", model.EstimatedTimeRemaining);
        clock.Advance(TimeSpan.FromSeconds(38));
        StringAssert.Contains(model.UpdateFreshness, "No recent worker update");
        Assert.IsFalse(model.HasError);
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(sequence: 2, revision: 2,
            etaUnavailableReason: "no_recent_progress")));
        Assert.AreEqual("Last update 0s ago", model.UpdateFreshness);
        Assert.AreEqual("Unavailable — no recent candidate progress", model.EstimatedTimeRemaining);
        Assert.AreEqual("4 files/s · 400 B/s · 10 s window", model.PartialRecentRate);
    }

    [DataTestMethod]
    [DataRow(59, "59s")]
    [DataRow(60, "1m 0s")]
    [DataRow(86399, "23h 59m 59s")]
    [DataRow(86400, "1d 0h 0m")]
    [DataRow(-1, "0s")]
    public void DurationKeepsDayBoundariesAndNegativeClockSkewReadable(int seconds, string expected)
    {
        Assert.AreEqual(expected, DisplayFormatting.Duration(TimeSpan.FromSeconds(seconds)));
    }

    [TestMethod]
    public void PhaseTransitionClearsRetainedPathAndLifecycleAnnouncesWithoutDetailedProgress()
    {
        var clock = new ManualProgressClock();
        using var model = new ScanProgressViewModel(new TestWorkerClient(), new ImmediateDispatcher(), clock: clock);
        var run = TestWorkerClient.CreateRun(1, 1, "running", "hashing", clock.GetUtcNow());
        model.ShowRun(run);
        model.ApplyProgress(ProgressTestData.Hashing());
        model.ApplyProgress(ProgressTestData.Hashing(sequence: 2, revision: 2, legacyPhase: "analyzing_folders",
            typedPhase: "analyzing_folders", folderAnalysis: new() { Substage = "hierarchy", Completed = 0, Total = 0 }));
        Assert.IsNull(model.CurrentPath);
        Assert.AreEqual(2L, model.ProgressAnnouncementVersion);
        model.ShowRun(run with { Id = 2 });
        Assert.IsNull(model.ProgressSnapshot);
        StringAssert.Contains(model.UpdateFreshness, "Waiting");
        model.ApplyLifecycle(model.Run! with { Status = "failed", ErrorMessage = "Fixture failure" });
        StringAssert.Contains(model.ProgressAnnouncement, "Fixture failure");
        Assert.AreEqual(3L, model.ProgressAnnouncementVersion);
    }

    [TestMethod]
    public async Task LateCancelResponseCannotReviveATerminalRun()
    {
        var client = new TestWorkerClient();
        var clock = new ManualProgressClock();
        var pending = new TaskCompletionSource<WorkerRun>();
        client.CancelHandler = (_, _) => pending.Task;
        using var model = new ScanProgressViewModel(client, new ImmediateDispatcher(), clock: clock);
        var run = TestWorkerClient.CreateRun(1, 1, "running", "hashing", clock.GetUtcNow());
        model.ShowRun(run);
        var cancel = model.CancelCommand.ExecuteAsync(null);
        StringAssert.Contains(model.ProgressAnnouncement, "cancellation requested");
        model.ApplyLifecycle(run with { Status = "completed", CompletedAt = clock.GetUtcNow() });
        pending.SetResult(run with { Status = "cancelling" });
        await cancel;
        Assert.AreEqual("completed", model.Run!.Status);
        Assert.IsFalse(model.IsIndeterminate);
    }

    [TestMethod]
    public void ClockTicksCoalesceAndDisposedOrTerminalCallbacksStaySilent()
    {
        var clock = new ManualProgressClock();
        var dispatcher = new DeferredDispatcher { Defer = true };
        var model = new ScanProgressViewModel(new TestWorkerClient(), dispatcher, clock: clock);
        var run = TestWorkerClient.CreateRun(1, 1, "running", "discovering", clock.GetUtcNow());
        model.ShowRun(run);
        var changes = 0;
        model.PropertyChanged += (_, _) => changes++;
        for (var i = 0; i < 100; i++) clock.Advance(TimeSpan.FromSeconds(1));
        Assert.AreEqual(1, dispatcher.Pending.Count);
        Assert.AreEqual(0, changes);
        dispatcher.Drain();
        Assert.AreEqual(2, changes);
        clock.Advance(TimeSpan.FromSeconds(1));
        model.Dispose();
        dispatcher.Drain();
        Assert.AreEqual(2, changes);
    }

    [TestMethod]
    public async Task ShellPreservesReceiptTimeAcrossDispatcherDelayAndCoalescing()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Long scan", Path.GetTempPath());
        var run = client.AddRun(session.Id, "running", "discovering");
        var clock = new ManualProgressClock();
        var dispatcher = new DeferredDispatcher();
        using var shell = new ShellViewModel(client, new TestFolderPicker(), new TestConfirmation(), dispatcher,
            new TestClipboard(), new TestExplorer(), new TestCloudLocationService(), clock: clock);
        await shell.InitializeAsync();
        dispatcher.Defer = true;
        client.RaiseProgress(ProgressTestData.Discovery(run.Id, discoveredFiles: 10));
        await dispatcher.Posted.Task.WaitAsync(TimeSpan.FromSeconds(5));
        clock.Advance(TimeSpan.FromSeconds(38));
        dispatcher.Drain();
        Assert.AreEqual("10", shell.Progress.FilesDiscovered);
        StringAssert.Contains(shell.Progress.UpdateFreshness, "38s ago");
        var stale = shell.Progress.UpdateFreshness;
        client.RaiseProgress(ProgressTestData.Discovery(run.Id, sequence: 2, revision: 2, discoveredFiles: 9));
        dispatcher.Drain();
        Assert.AreEqual(stale, shell.Progress.UpdateFreshness);
        client.RaiseUnexpectedExit(23);
        dispatcher.Drain();
        Assert.AreEqual("Interrupted", shell.Progress.Status);
        Assert.AreEqual(WorkerConnectionState.RecoveryRequired, shell.ConnectionState);
        Assert.IsFalse(shell.Progress.IsIndeterminate);
    }

    private sealed class DeferredDispatcher : IUiDispatcher
    {
        public bool Defer { get; set; }
        public ConcurrentQueue<Action> Pending { get; } = new();
        public TaskCompletionSource Posted { get; } = new(TaskCreationOptions.RunContinuationsAsynchronously);
        public void Post(Action action)
        {
            if (!Defer) { action(); return; }
            Pending.Enqueue(action);
            Posted.TrySetResult();
        }
        public void Drain() { while (Pending.TryDequeue(out var action)) action(); }
    }
}
