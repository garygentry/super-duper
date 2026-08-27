using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class PerformanceViewModelTests
{
    [TestMethod]
    public async Task BoundedSnapshotPreservesUnavailableValuesAndComparesPersistedContextAfterRestart()
    {
        var worker = new TestWorkerClient();
        var current = Snapshot(7, 107, "input-a", "engine-a", "device-a", "volume-a", unavailable: true);
        var prior = Snapshot(6, 106, "input-b", "engine-b", "device-b", "volume-b", unavailable: false);
        worker.PerformanceRunsHandler = (beforeId, pageSize, _) =>
        {
            Assert.IsNull(beforeId);
            Assert.AreEqual(PerformanceViewModel.HistoryLimit, pageSize);
            return Task.FromResult(new WorkerPerformanceRunPage(
                Enumerable.Range(0, PerformanceViewModel.HistoryLimit)
                    .Select(index => Run(7 - index, 107 - index, "input-a", "engine-a"))
                    .ToArray(),
                1,
                false));
        };
        worker.PerformanceSnapshotHandler = (statusRunId, productRunId, _) =>
            Task.FromResult(statusRunId == 6 || productRunId == 106 ? prior : current);
        var productRun = TestWorkerClient.CreateRun(107, 3, "completed", "finalizing", DateTimeOffset.UtcNow.AddMinutes(-2));

        using var viewModel = new PerformanceViewModel(worker);
        await viewModel.ShowRunAsync(productRun);

        Assert.AreEqual(PerformanceViewModel.HistoryLimit, viewModel.History.Count);
        Assert.AreEqual(1, viewModel.Phases.Count);
        Assert.AreEqual(1, viewModel.Devices.Count);
        Assert.AreEqual("Unavailable", viewModel.CpuSummary);
        Assert.AreEqual("Unavailable", viewModel.Devices[0].CurrentIops);
        StringAssert.Contains(viewModel.UnavailableSummary, "unavailable in latest host sample");
        Assert.IsFalse(viewModel.HasError);

        viewModel.SelectedComparisonRun = viewModel.History.Single(item => item.StatusRunId == 6);
        await viewModel.CompareCommand.ExecuteAsync(null);
        StringAssert.Contains(viewModel.ComparisonMessage, "volume/device");
        StringAssert.Contains(viewModel.ComparisonMessage, "scan inputs");
        StringAssert.Contains(viewModel.ComparisonMessage, "software build");
        Assert.AreNotEqual("—", viewModel.ComparisonPeakRead);

        using var restarted = new PerformanceViewModel(worker);
        await restarted.ShowRunAsync(productRun);
        restarted.SelectedComparisonRun = restarted.History.Single(item => item.StatusRunId == 6);
        await restarted.CompareCommand.ExecuteAsync(null);
        Assert.AreEqual(viewModel.RunDuration, restarted.RunDuration);
        Assert.AreEqual(viewModel.ComparisonMessage, restarted.ComparisonMessage);
    }

    [TestMethod]
    public async Task RejectsCollectionsBeyondTheUiBoundaryAndNeverEnablesExecution()
    {
        var worker = new TestWorkerClient
        {
            PerformanceRunsHandler = (_, _, _) => Task.FromResult(new WorkerPerformanceRunPage(
                Enumerable.Range(1, PerformanceViewModel.HistoryLimit + 1)
                    .Select(index => Run(index, index, "input", "engine"))
                    .ToArray(),
                null,
                false)),
            PerformanceSnapshotHandler = (_, _, _) => Task.FromResult(Snapshot(
                1, 1, "input", "engine", "device", "volume", unavailable: false)),
        };
        using var viewModel = new PerformanceViewModel(worker);

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(1, 1, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.IsTrue(viewModel.HasError);
        Assert.AreEqual(0, viewModel.Phases.Count);
        Assert.AreEqual(0, viewModel.Devices.Count);
        StringAssert.Contains(viewModel.ErrorMessage, "unbounded performance collection");

        var noCounters = Snapshot(1, 1, "input", "engine", "device", "volume", unavailable: true);
        noCounters = noCounters with
        {
            Counters = [],
            Phases = [],
            Devices = [noCounters.Devices[0] with { PeakReadBytesPerSecond = 0 }],
        };
        worker.PerformanceRunsHandler = (_, _, _) => Task.FromResult(
            new WorkerPerformanceRunPage([Run(1, 1, "input", "engine")], null, false));
        worker.PerformanceSnapshotHandler = (_, _, _) => Task.FromResult(noCounters);

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(1, 1, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.IsFalse(viewModel.HasError);
        StringAssert.Contains(viewModel.CandidateFunnel, "no counter summary");
        Assert.AreEqual("Unavailable", viewModel.WarningSummary);
        Assert.AreEqual("0 B/s", viewModel.CurrentPeakRead);
    }

    private static WorkerPerformanceSnapshot Snapshot(
        long statusRunId,
        long productRunId,
        string input,
        string engine,
        string deviceKey,
        string volumeKey,
        bool unavailable)
    {
        var run = Run(statusRunId, productRunId, input, engine);
        var latestHost = new WorkerHostPerformanceSample(
            2, 1_700_000_005_000, 5_000_000_000, "full_hashing", 50_000_000,
            unavailable ? null : 128_000_000, unavailable ? null : 96_000_000, 96_000_000,
            20, 1_000_000, 3, 10_000, unavailable ? null : 2500,
            unavailable ? null : 2_000_000_000, 4_000_000_000, unavailable ? 4u : 0u);
        var host = new WorkerHostPerformanceSummary(
            latestHost,
            unavailable ? null : 150_000_000,
            unavailable ? null : 100_000_000,
            unavailable ? null : 3000,
            unavailable ? null : 1_900_000_000);
        var latestDevice = new WorkerDevicePerformanceSample(
            2, deviceKey, unavailable ? null : 20_000_000,
            unavailable ? null : 125_000, unavailable ? null : 12_000,
            unavailable ? null : 750, unavailable ? null : 2_500, unavailable ? 5u : 0u);
        var device = new WorkerDevicePerformanceSummary(
            new WorkerDeviceDescriptor(deviceKey, volumeKey, "NTFS", 1_000_000_000, 500_000_000, "NVMe", "SSD", "Fixture drive"),
            latestDevice,
            unavailable ? null : 25_000_000,
            unavailable ? null : 150_000,
            unavailable ? null : 15_000,
            unavailable ? null : 800,
            unavailable ? null : 3_000);
        return new WorkerPerformanceSnapshot(
            run,
            [
                new("discovered_files", 100, 2), new("metadata_resolved_files", 20, 2),
                new("candidate_files", 80, 2), new("partial_hashes_succeeded", 70, 2),
                new("full_hash_requests", 50, 2), new("confirmed_physical_items", 10, 2),
                new("full_hash_cache_hits", 30, 2), new("full_hash_cache_misses", 15, 2),
                new("full_hash_cache_errors", 5, 2), new("full_hash_bytes_read", 50_000_000, 2),
                new("warnings", 3, 2), new("unavailable_counters", unavailable ? 9u : 0u, 2),
            ],
            [new("full_hashing", "completed", 0, 5_000_000_000, 5_000_000_000)],
            host,
            [device],
            false);
    }

    private static WorkerPerformanceRun Run(long statusRunId, long productRunId, string input, string engine) => new(
        statusRunId, $"operation-{statusRunId}", productRunId, 2, engine, "worker", "app", 14,
        input, "completed", 1_700_000_000_000, 1_700_000_005_000, 5_000_000_000, 2, null, null);
}
