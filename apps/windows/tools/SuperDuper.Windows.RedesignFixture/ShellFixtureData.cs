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
                    "4096", 2, "4096", $"{id:00} — {name}.jpg", "jpg")).ToArray(), 25, null, null)
            { Summary = new WorkerDuplicateFileReviewSummary(25, 50, "102400", "4096") });
        };
        client.MemberPageHandler = (query, _) => Task.FromResult(new WorkerDuplicateFileMemberPage(
            Enumerable.Range(1, 2).Select(id => new WorkerDuplicateFileMember(id, query.GroupId,
                $@"C:\fixture\copy-{id}\{name}.jpg", $"{name}.jpg", $@"C:\fixture\copy-{id}",
                "4096", "0")).ToArray(), 2, null, null));
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
