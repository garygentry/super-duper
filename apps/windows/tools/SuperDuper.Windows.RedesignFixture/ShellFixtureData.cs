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
        Model = new ShellViewModel(client, new TestFolderPicker(), new TestConfirmation(),
            new ImmediateDispatcher(), new TestClipboard(), new TestExplorer(), new TestCloudLocationService());
        Model.InitializeAsync().GetAwaiter().GetResult();
        OldRun = old;
        ActiveRun = active;
    }

    public void Dispose() => Model.Dispose();
}
