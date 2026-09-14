using SuperDuper.Windows.Core.Tests;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Fixtures;

// Fictional in-memory data shared by loaded-STA checks and the manual desktop host.
// Every external service is fake: no worker, database, scan, Explorer or deletion is reachable.
internal sealed class ShellFixtureData : IDisposable
{
    internal TestWorkerClient Client { get; } = new();
    internal ShellViewModel Model { get; }
    internal WorkerRun OldRun { get; }
    internal WorkerRun ActiveRun { get; }
    internal int FileQueries { get; private set; }

    internal ShellFixtureData()
    {
        var client = Client;
        var name = "Fictional family archive — photographs and recordings from several generations — preserved originals and travel collections";
        var session = client.AddSession(name, @"C:\fixture\originals", @"C:\fixture\copies");
        var old = client.AddRun(session.Id, "completed") with { WarningCount = 1 };
        old = old with { Parameters = old.Parameters with { Roots = session.Roots } };
        client.Runs[0] = old;
        var active = client.AddRun(session.Id, "running", "hashing") with { WarningCount = 1 };
        client.Runs[1] = active;

        client.GroupPageHandler = (query, _) =>
        {
            FileQueries++;
            return Task.FromResult(new WorkerDuplicateFileGroupPage(
                Enumerable.Range(1, 25).Select(id => new WorkerDuplicateFileGroup(id, query.RunId,
                    "4096", 2, "4096", $"{id:00} — {name}.jpg", "jpg")
                { DistinctSelectedRootCount = 2, DistinctDriveCount = 1 }).ToArray(), 25, null, null)
            { Summary = new WorkerDuplicateFileReviewSummary(25, 50, "102400", "4096") });
        };
        client.MemberPageHandler = (query, _) => Task.FromResult(new WorkerDuplicateFileMemberPage(
            Enumerable.Range(1, 2).Select(id => new WorkerDuplicateFileMember(id, query.GroupId,
                $@"{session.Roots[id - 1]}\travel\{name}.jpg", $"{name}.jpg", $@"{session.Roots[id - 1]}\travel",
                "4096", "1704067200000000000")
            {
                RootPath = session.Roots[id - 1], RelativePath = $@"travel\{name}.jpg", DriveLetter = "C:",
                Decision = id == 1 ? "remove" : "keep",
            }).ToArray(), 2, null, null)
        {
            ReviewPlanId = 7,
            ReviewRevision = 2,
            ReviewSummary = new WorkerReviewGroupSummary(query.GroupId, 1, 1, 0, 1),
        });
        client.ReviewPlanHandler = (runId, _) => Task.FromResult(runId == old.Id
            ? new WorkerReviewPlanView(
                new WorkerReviewPlan(7, runId, "active", 2, "2026-09-08T09:45:00Z", "2026-09-08T09:46:00Z"),
                new WorkerReviewPlanSummary(1, 1, 1, 48, "4096", 49)
                {
                    FolderUndecidedCount = 50,
                    EffectiveRemovalFileCount = 1,
                    PlannedRemovalPhysicalItemCount = 1,
                    IntactFolderCopyCount = 50,
                })
            : new WorkerReviewPlanView(
                new WorkerReviewPlan(null, runId, "notCreated", 0, null, null),
                new WorkerReviewPlanSummary(0, 0, 0, 0, "0", 0)));
        client.ReviewGroupPageHandler = (runId, pageSize, cursor, _) => Task.FromResult(
            new WorkerReviewGroupPage(
                Enumerable.Range(1, Math.Min(pageSize, 25)).Select(id =>
                    id == 1
                        ? new WorkerReviewGroupSummary(id, 1, 1, 0, 1)
                        : new WorkerReviewGroupSummary(id, 0, 0, 2, 2)).ToArray(),
                25,
                runId == old.Id ? 7 : null,
                runId == old.Id ? 2 : 0,
                null));
        client.ReviewFolderGroupPageHandler = (runId, pageSize, cursor, _) => Task.FromResult(
            new WorkerReviewFolderGroupPage(
                Enumerable.Range(1, Math.Min(pageSize, 25))
                    .Select(id => new WorkerReviewFolderGroupSummary(id, 0, 0, 2, 2)).ToArray(),
                25,
                runId == old.Id ? 7 : null,
                runId == old.Id ? 2 : 0,
                null));
        client.LatestPreflightHandler = (runId, _) => Task.FromResult<WorkerPreflight?>(
            runId == old.Id ? TestWorkerClient.CreatePreflight(17, runId, "completed", 2) : null);
        client.PreflightItemPageHandler = (query, _) => Task.FromResult(new WorkerPreflightItemPage(
            [new WorkerPreflightItem(
                1, query.PreflightId, 0, "file", "remove", 1, null, null, 1, null,
                $@"{session.Roots[0]}\travel\{name}.jpg", "ready", "matched_snapshot", "4096", 1,
                null, "2026-09-08T09:47:00Z", 1)],
            1,
            null));
        client.RunWarningsHandler = (query, _) => Task.FromResult(new WorkerRunWarningPage(
            [new WorkerRunWarningAggregate(1, query.RunId, "hashing", "scan", RunHistoryViewModel.HashWarningCode,
                "warning", "Fictional unavailable copy; inspect immutable duplicate results.", 1,
                [@"C:\fixture\unavailable-copy.jpg"])], 1, 1, 1, 1,
            query.RunId == active.Id ? "active" : "terminal",
            query.RunId == active.Id ? "running" : "completed", TestWorkerClient.DiagnosticLog, null, false));
        var performanceSnapshots = Enumerable.Range(0, PerformanceViewModel.HistoryLimit)
            .Select(index => index switch
            {
                0 => CreatePerformanceSnapshot(702, active.Id, "running", $"input-{active.Id}"),
                1 => CreatePerformanceSnapshot(701, old.Id, "completed", $"input-{old.Id}"),
                _ => CreatePerformanceSnapshot(700 - index, 10_000 - index, "completed", $"input-{10_000 - index}"),
            })
            .ToArray();
        client.PerformanceRunsHandler = (_, pageSize, _) => Task.FromResult(new WorkerPerformanceRunPage(
            performanceSnapshots.Take(pageSize).Select(snapshot => snapshot.Run).ToArray(), null, false));
        client.PerformanceSnapshotHandler = (statusRunId, productRunId, _) => Task.FromResult(
            statusRunId is long telemetryId
                ? performanceSnapshots.Single(snapshot => snapshot.Run.Id == telemetryId)
                : performanceSnapshots.Single(snapshot => snapshot.Run.ProductRunId == productRunId));
        Model = new ShellViewModel(client, new TestFolderPicker(), new TestConfirmation(),
            new ImmediateDispatcher(), new TestClipboard(), new TestExplorer(), new TestCloudLocationService());
        Model.InitializeAsync().GetAwaiter().GetResult();
        OldRun = old;
        ActiveRun = active;
    }

    public void Dispose() => Model.Dispose();

    private static WorkerPerformanceSnapshot CreatePerformanceSnapshot(
        long statusRunId,
        long productRunId,
        string state,
        string inputSignature)
    {
        var run = new WorkerPerformanceRun(
            statusRunId,
            $"fixture-operation-{statusRunId}",
            productRunId,
            2,
            "fixture-engine",
            "fixture-worker",
            "fixture-app",
            15,
            inputSignature,
            state,
            1_788_750_000_000 + statusRunId,
            state == "completed" ? 1_788_750_100_000 + statusRunId : null,
            100_000_000_000,
            20,
            null,
            null);
        var phases = new[] { "discovery", "partial_hashing", "full_hashing", "folder_tree", "folder_structural", "finalizing" }
            .Select((phase, index) => new WorkerPerformancePhase(
                phase,
                index == 5 && state != "completed" ? "running" : "completed",
                (ulong)(index * 1_000_000_000),
                (ulong)((index + 1) * 1_000_000_000),
                (ulong)((index + 1) * 1_000_000_000)))
            .ToArray();
        var devices = Enumerable.Range(1, PerformanceViewModel.DeviceLimit)
            .Select(index => new WorkerDevicePerformanceSummary(
                new WorkerDeviceDescriptor(
                    $"fixture-device-{index:00}",
                    $"volume:{index:00}",
                    "NTFS",
                    2_000_000_000_000,
                    1_000_000_000_000,
                    "NVMe",
                    "SSD",
                    $"Fictional archive drive {index:00}"),
                new WorkerDevicePerformanceSample(
                    20,
                    $"fixture-device-{index:00}",
                    index == 1 ? null : (ulong)(20_000_000 + index),
                    index == 1 ? null : (ulong)(125_000 + index),
                    index == 1 ? null : (ulong)(12_000 + index),
                    index == 1 ? null : (uint)(750 + index),
                    index == 1 ? null : (ulong)(2_500 + index),
                    index == 1 ? 5u : 0u),
                25_000_000 + (ulong)index,
                150_000 + (ulong)index,
                15_000 + (ulong)index,
                800 + (uint)index,
                3_000 + (ulong)index))
            .ToArray();
        return new WorkerPerformanceSnapshot(
            run,
            [
                new("discovered_files", 100, 20), new("metadata_resolved_files", 20, 20),
                new("candidate_files", 80, 20), new("candidate_bytes", 100_000_000, 20),
                new("partial_hashes_succeeded", 70, 20), new("partial_hash_bytes_read", 1_000_000, 20),
                new("full_hash_requests", 50, 20), new("full_hash_bytes_read", 50_000_000, 20),
                new("confirmed_physical_items", 10, 20),
                new("partial_hash_cache_hits", 20, 20), new("partial_hash_cache_misses", 10, 20),
                new("partial_hash_cache_errors", 0, 20), new("partial_hash_cache_stores", 10, 20),
                new("full_hash_cache_hits", 30, 20), new("full_hash_cache_misses", 15, 20),
                new("full_hash_cache_errors", 5, 20), new("full_hash_cache_stores", 15, 20),
                new("warnings", 1, 20), new("unavailable_counters", 5, 20),
            ],
            phases,
            new WorkerHostPerformanceSummary(
                new WorkerHostPerformanceSample(
                    20, 1_788_750_100_000, 100_000_000_000, "finalizing", 50_000_000,
                    128_000_000, 96_000_000, 96_000_000, 20, 1_000_000, 3, 10_000, null,
                    2_000_000_000, 4_000_000_000, 1),
                150_000_000,
                100_000_000,
                3_000,
                1_900_000_000),
            devices,
            false);
    }
}
