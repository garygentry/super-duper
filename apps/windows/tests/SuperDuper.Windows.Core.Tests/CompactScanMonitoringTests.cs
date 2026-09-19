using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class CompactScanMonitoringTests
{
    [TestMethod]
    public void HashBarUsesResolvedLogicalCandidateBytesAndNeverDeclaresCompletion()
    {
        using var model = Create();
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing()));
        Assert.AreEqual(50d, model.PhaseWork.Percent);
        Assert.IsFalse(model.IsIndeterminate);
        StringAssert.Contains(model.PhaseWork.Label, "Hash candidate work resolved");
        StringAssert.Contains(model.PhaseWork.Detail, "not disk bytes read");
        StringAssert.Contains(model.ExactHashWork, "4000 of 8000 logical bytes");
        Assert.AreEqual("400 B actually read", model.PartialReadBytes);
        Assert.AreEqual("1000 B actually read", model.FullReadBytes);
        var logical = model.ProgressSnapshot!.Logical with { HashPipelineResolvedFiles = 8, HashPipelineResolvedBytes = "8000" };
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(sequence: 2, revision: 2, measuredLogical: logical)));
        Assert.AreEqual(100d, model.PhaseWork.Percent);
        Assert.IsTrue(model.IsActive);
        Assert.IsTrue(model.CanCancel);
        Assert.AreEqual("Scanning", model.Status);
    }

    [TestMethod]
    public void UnknownZeroAndLifecyclePhaseWithoutMatchingSnapshotStayDistinct()
    {
        using var model = Create();
        Assert.IsTrue(model.IsIndeterminate);
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Discovery(discoveredFiles: 10)));
        Assert.IsNull(model.PhaseWork.Percent);
        StringAssert.Contains(model.PhaseWork.Detail, "total unknown");
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(sequence: 2, revision: 2)));
        model.ApplyLifecycle(model.Run! with { Phase = "analyzing_folders" });
        Assert.IsTrue(model.IsIndeterminate);
        Assert.IsNull(model.PhaseWork.Percent, "Hash denominator must not appear as folder work.");
        model.ShowRun(model.Run! with { Phase = "hashing" });
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(measuredCounters: new(), measuredLogical: new())));
        Assert.IsFalse(model.IsIndeterminate);
        Assert.IsNull(model.PhaseWork.Percent);
        Assert.AreEqual(0d, model.PhaseWork.BarValue);
        StringAssert.Contains(model.PhaseWork.Detail, "No logical candidate bytes");
        Assert.IsTrue(model.IsActive);
        model.ShowRun(model.Run!);
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(measuredCounters: new(), measuredLogical: new(), candidateTotalsKnown: false)));
        Assert.IsTrue(model.IsIndeterminate, "Zero counters before candidate totals are known are not an empty phase.");
        StringAssert.Contains(model.PhaseWork.Detail, "Candidate total unknown");
        StringAssert.Contains(model.HashPipelineCandidateContext, "not yet known");
        StringAssert.Contains(model.ExactHashWork, "not yet known");
    }

    [TestMethod]
    public void EveryFolderSubstageUsesItsOwnDenominatorIncludingZeroAndUnknown()
    {
        using var model = Create();
        model.ApplyLifecycle(model.Run! with { Phase = "analyzing_folders" });
        Assert.IsTrue(model.IsIndeterminate);
        ulong sequence = 0;
        foreach (var (stage, label) in new[] { ("hierarchy", "Building hierarchy"),
            ("structural_candidates", "Finding structural candidates"),
            ("verification", "Verifying exact content"), ("persistence", "Saving folder results") })
        {
            foreach (var (done, total) in new[] { (0UL, 0UL), (1UL, 4UL), (4UL, 4UL) })
            {
                sequence++;
                Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(sequence: sequence, revision: sequence,
                    legacyPhase: "analyzing_folders", typedPhase: "analyzing_folders",
                    etaUnavailableReason: "not_applicable",
                    folderAnalysis: new() { Substage = stage, Completed = done, Total = total })));
                Assert.AreEqual(label, model.PhaseWork.Label);
                Assert.AreEqual(total == 0 ? null : (double?)done * 100 / total, model.PhaseWork.Percent);
                Assert.IsFalse(model.IsIndeterminate);
                Assert.IsTrue(model.IsActive);
                StringAssert.Contains(model.EstimatedTimeRemaining, "does not apply");
            }
        }
        model.ApplyLifecycle(model.Run! with { Status = "completed" });
        Assert.IsFalse(model.IsIndeterminate);
        StringAssert.Contains(model.MetricsContext, "Historical metrics");
    }

    [TestMethod]
    public void LargeDenominatorDoesNotOverflowAndRetainsExactDiagnosticBytes()
    {
        using var model = Create();
        var original = ProgressTestData.Hashing().Progress;
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(
            measuredCounters: original.Counters with { CandidateBytes = ulong.MaxValue.ToString() },
            measuredLogical: original.Logical with { HashPipelineResolvedBytes = (ulong.MaxValue / 2).ToString() })));
        Assert.AreEqual(50d, model.PhaseWork.Percent!.Value, 0.00001);
        StringAssert.Contains(model.ExactHashWork, "18446744073709551615");
        var stage = new ScanProgressStage("Discovered", 1, "18446744073709551615", "stage");
        Assert.AreEqual("18446744073709551615 B", stage.BytesText);
    }

    [TestMethod]
    public void CacheDiagnosticsPreserveIndependentPartialAndFullOutcomesAndUnavailable()
    {
        using var model = Create();
        StringAssert.Contains(model.PartialCacheOutcomes, "Unavailable");
        var original = ProgressTestData.Hashing().Progress;
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(measuredCounters: original.Counters with
        {
            PartialHashCacheHits = 1, PartialHashCacheMisses = 2, PartialHashCacheErrors = 1, PartialHashCacheStores = 2,
        })));
        Assert.AreEqual("Hits 1 · misses 2 · errors 1 · stores 2", model.PartialCacheOutcomes);
        Assert.AreEqual("Hits 1 · misses 1 · errors 0 · stores 1", model.FullCacheOutcomes);
        StringAssert.Contains(model.ReadOutcomes, "Full requests 2");
        StringAssert.Contains(model.TelemetryDiagnostics, "phase 10000000000 ns");
    }

    [TestMethod]
    public void StoppedRunWithoutMeasuredPhaseWorkDoesNotPromiseMoreUpdates()
    {
        using var model = Create();
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing()));
        model.ApplyLifecycle(model.Run! with { Phase = "finalizing", Status = "completed" });
        Assert.IsFalse(model.IsIndeterminate);
        Assert.IsNull(model.PhaseWork.Percent);
        StringAssert.Contains(model.PhaseWork.Detail, "Historical phase work");
        StringAssert.Contains(model.PhaseWork.Detail, "no measured total was reported");
        Assert.IsFalse(model.CanCancel);
    }

    [TestMethod]
    public void SampledPathIsDisplayOnlyAndClearsOnPhaseChange()
    {
        using var model = Create();
        const string path = @"\\?\UNC\server\share\long parent\candidate.bin";
        Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(currentPath: path)));
        Assert.AreEqual("candidate.bin", model.ActivityFileName);
        Assert.AreEqual(@"\\server\share\long parent\", model.ActivityParent);
        Assert.AreEqual(path, model.CurrentPath);
        Assert.AreEqual(@"\\server\share\long parent\candidate.bin", model.DisplayCurrentPath);
        model.ApplyLifecycle(model.Run! with { Phase = "persisting" });
        Assert.AreEqual("No path reported for this phase", model.ActivityFileName);
        Assert.AreEqual(string.Empty, model.ActivityParent);
        Assert.IsNull(model.CurrentPath);
        Assert.IsNull(model.PhaseWork.Percent);
    }

    private static ScanProgressViewModel Create()
    {
        var clock = new ManualProgressClock();
        var model = new ScanProgressViewModel(new TestWorkerClient(), new ImmediateDispatcher(), clock: clock);
        model.ShowRun(TestWorkerClient.CreateRun(1, 1, "running", "discovering", clock.GetUtcNow()));
        return model;
    }
}
