using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class ShellSessionWorkflowTests
{
    [TestMethod]
    public async Task HistoricalResultsRemainSelectedWhileActiveScanProgressesAndCompletes()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Archive", Path.GetTempPath());
        var older = client.AddRun(session.Id, "completed");
        var active = client.AddRun(session.Id, "running", "discovering");
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.History.SelectedRun = shell.History.Runs.Single(run => run.Id == older.Id);
        shell.SelectedDestination = WorkspaceDestination.FileResults;

        var progressApplied = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        shell.Progress.PropertyChanged += (_, _) =>
        {
            if (shell.Progress.Run?.WarningCount == 4) progressApplied.TrySetResult();
        };
        client.RaiseProgress(ProgressTestData.Discovery(active.Id, warningCount: 4));
        await progressApplied.Task.WaitAsync(TimeSpan.FromSeconds(5));
        Assert.AreEqual(active.Id, shell.Progress.Run?.Id);
        Assert.AreEqual(4, shell.Progress.Run?.WarningCount);
        Assert.AreEqual(older.Id, shell.SelectedRun?.Id);
        Assert.AreEqual(older.Id, shell.DuplicateFiles.Run?.Id);
        StringAssert.Contains(shell.SelectedScanContext, $"Scan {older.Id}");
        Assert.AreEqual("Archive", shell.ActiveScanName);
        Assert.IsFalse(shell.CanStartRun);
        Assert.IsFalse(shell.Preflight.Operation.CanSubmit);

        client.RaiseLifecycle("run.completed", active with { Status = "completed", CompletedAt = DateTimeOffset.UtcNow });
        Assert.IsFalse(shell.HasActiveRun);
        Assert.AreEqual(older.Id, shell.SelectedRun?.Id);
        Assert.AreEqual(older.Id, shell.DuplicateFiles.Run?.Id);
        Assert.AreEqual(WorkspaceDestination.FileResults, shell.SelectedDestination);
        StringAssert.Contains(shell.ProgressScanContext, $"Scan {active.Id}");
        StringAssert.Contains(shell.SelectedScanContext, $"Scan {older.Id}");
    }

    [TestMethod]
    public async Task BrowsingAnotherSavedScanKeepsCancellationOnTheActiveRun()
    {
        var client = new TestWorkerClient();
        var first = client.AddSession("Active locations", Path.GetTempPath());
        var second = client.AddSession("Historical locations", Path.GetTempPath());
        var active = client.AddRun(first.Id, "running", "discovering");
        var old = client.AddRun(second.Id, "completed");
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.Sessions.SelectedSession = shell.Sessions.Find(second.Id);
        Assert.AreEqual(old.Id, shell.SelectedRun?.Id);
        Assert.AreEqual(active.Id, shell.Progress.Run?.Id);

        shell.ViewProgressCommand.Execute(null);
        Assert.AreEqual(WorkspaceDestination.ScanProgress, shell.SelectedDestination);
        await shell.Progress.CancelCommand.ExecuteAsync(null);
        Assert.AreEqual("cancelling", client.Runs.Single(run => run.Id == active.Id).Status);
        Assert.AreEqual("completed", client.Runs.Single(run => run.Id == old.Id).Status);
        Assert.AreEqual(old.Id, shell.SelectedRun?.Id);
    }

    [TestMethod]
    public async Task SlowOptionalResultsDoNotBlockSetupAndPerformanceLoadsOnlyOnNavigation()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Ready setup", Path.GetTempPath());
        var run = client.AddRun(session.Id, "completed");
        var groups = new TaskCompletionSource<WorkerDuplicateFileGroupPage>();
        var groupRequests = 0;
        var performanceRequests = 0;
        client.GroupPageHandler = (_, _) => { groupRequests++; return groups.Task; };
        client.PerformanceRunsHandler = (_, _, _) =>
        {
            performanceRequests++;
            return Task.FromResult(new WorkerPerformanceRunPage([], null, false));
        };
        using var shell = CreateShell(client);

        await shell.InitializeAsync().WaitAsync(TimeSpan.FromSeconds(5));
        Assert.IsFalse(shell.IsLoadingSession);
        Assert.AreEqual("Ready setup", shell.Setup.Name);
        Assert.IsTrue(shell.DuplicateFiles.IsLoading);
        Assert.AreEqual(1, groupRequests, "The shell must not duplicate the selection event's query.");
        Assert.AreEqual(0, performanceRequests);
        shell.SelectedDestination = WorkspaceDestination.Performance;
        Assert.AreEqual(1, performanceRequests);
        groups.SetException(new InvalidOperationException("This result query failed."));
        Assert.IsNull(shell.ContentErrorMessage);
        Assert.IsTrue(shell.IsConnected);
        Assert.AreEqual(run.Id, shell.SelectedRun?.Id);
    }

    [DataTestMethod]
    [DataRow(false)]
    [DataRow(true)]
    public async Task LateHistoryResponseCannotReplaceAnotherSavedScan(bool fail)
    {
        var client = new TestWorkerClient();
        var first = client.AddSession("First", Path.GetTempPath());
        var second = client.AddSession("Second", Path.GetTempPath());
        var old = client.AddRun(first.Id, "completed");
        var current = client.AddRun(second.Id, "completed");
        var delayed = new TaskCompletionSource<WorkerRunPage>();
        client.RunsHandler = (id, _) => id == first.Id
            ? delayed.Task : Task.FromResult(new WorkerRunPage([current], 1));
        using var shell = CreateShell(client);
        var initialization = shell.InitializeAsync();
        shell.Sessions.SelectedSession = shell.Sessions.Find(second.Id);
        if (fail) delayed.SetException(new InvalidOperationException("Stale history error"));
        else delayed.SetResult(new WorkerRunPage([old], 1));
        await initialization;

        Assert.AreEqual("Second", shell.DisplaySessionName);
        Assert.AreEqual(current.Id, shell.SelectedRun?.Id);
        Assert.AreEqual(current.Id, shell.DuplicateFiles.Run?.Id);
        Assert.AreEqual(second.Id, shell.History.SessionId);
        Assert.IsNull(shell.History.ErrorMessage);
        Assert.IsNull(shell.ContentErrorMessage);
        Assert.IsFalse(shell.History.IsLoading);
    }

    [TestMethod]
    public async Task LateSetupFailureCannotCoverTheNewWorkspace()
    {
        var client = new TestWorkerClient();
        var first = client.AddSession("First", Path.GetTempPath());
        var second = client.AddSession("Second", Path.GetTempPath());
        var delayed = new TaskCompletionSource<WorkerSessionDefinition>();
        client.SessionHandler = (id, _) => id == first.Id ? delayed.Task : Task.FromResult(second);
        using var shell = CreateShell(client);
        var initialization = shell.InitializeAsync();
        Assert.IsTrue(shell.IsLoadingSession);
        Assert.IsFalse(shell.CanStartRun);
        shell.Sessions.SelectedSession = shell.Sessions.Find(second.Id);
        delayed.SetException(new InvalidOperationException("Stale setup error"));
        await initialization;
        Assert.AreEqual("Second", shell.Setup.Name);
        Assert.IsNull(shell.ContentErrorMessage);
        Assert.IsFalse(shell.IsLoadingSession);
    }

    [TestMethod]
    public async Task FailedSetupLoadCannotExposeOrStartThePreviousDefinitionAfterErrorDismissal()
    {
        var client = new TestWorkerClient();
        client.AddSession("First", Path.GetTempPath());
        var second = client.AddSession("Second", Path.GetTempPath());
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        client.SessionHandler = (_, _) => Task.FromException<WorkerSessionDefinition>(new InvalidOperationException("Unavailable"));
        shell.Sessions.SelectedSession = shell.Sessions.Find(second.Id);
        shell.ClearContentErrorCommand.Execute(null);
        Assert.AreEqual("Second", shell.DisplaySessionName);
        Assert.IsFalse(shell.IsSetupAvailable);
        Assert.IsFalse(shell.CanStartRun);
    }

    [TestMethod]
    public async Task InitializeAsync_RestoresSessionAndInterruptedRun()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Backups", Path.GetTempPath());
        client.AddRun(session.Id, "interrupted");
        using var shell = CreateShell(client);

        await shell.InitializeAsync();

        Assert.IsTrue(shell.IsWorkspaceVisible);
        Assert.AreEqual("Backups", shell.DisplaySessionName);
        Assert.AreEqual(1, shell.History.Runs.Count);
        Assert.AreEqual("Interrupted", shell.Progress.Status);
    }

    [TestMethod]
    public async Task StartRunCommand_CreatesSessionAndShowsProgress()
    {
        var client = new TestWorkerClient();
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        var root = Directory.CreateTempSubdirectory("super-duper-shell-");
        try
        {
            await shell.Sessions.NewSessionCommand.ExecuteAsync(null);
            shell.Setup.Name = "Photos";
            shell.Setup.Roots.Clear();
            shell.Setup.Roots.Add(new SessionRootViewModel(root.FullName));

            await shell.StartRunCommand.ExecuteAsync(null);

            Assert.AreEqual(1, client.Sessions.Count);
            Assert.AreEqual(1, client.Runs.Count);
            Assert.IsTrue(shell.HasActiveRun);
            Assert.AreEqual(WorkspaceDestination.ScanProgress, shell.SelectedDestination);
            Assert.AreEqual("Scanning", shell.Progress.Status);
        }
        finally
        {
            root.Delete(recursive: true);
        }
    }

    [TestMethod]
    public async Task StartRunCommand_SnapshotsSelectedRepeatPolicyIntoRunHistory()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Repeat", Path.GetTempPath());
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.Setup.RepeatCachePolicy = RepeatCachePolicyNames.RevalidateContent;

        await shell.StartRunCommand.ExecuteAsync(null);

        Assert.AreEqual(RepeatCachePolicyNames.RevalidateContent, client.LastRepeatCachePolicy);
        Assert.AreEqual(
            RepeatCachePolicyNames.RevalidateContent,
            shell.History.SelectedRun?.Run.Parameters.RepeatCachePolicy);
        Assert.AreEqual(session.Id, shell.History.SelectedRun?.Run.SessionId);
    }

    [TestMethod]
    public async Task CancelledStaleStartCannotReplaceNewSetupOrPublishItsFailure()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("First", Path.GetTempPath());
        var completion = new TaskCompletionSource<WorkerRun>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var requestObserved = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        client.StartRunHandler = (_, _, _) =>
        {
            requestObserved.SetResult();
            return completion.Task;
        };
        using var shell = CreateShell(client);
        await shell.InitializeAsync();

        var staleStart = shell.StartRunCommand.ExecuteAsync(null);
        await requestObserved.Task.WaitAsync(TimeSpan.FromSeconds(5));
        await shell.Sessions.NewSessionCommand.ExecuteAsync(null);
        completion.SetException(new InvalidOperationException("stale start failed"));
        await staleStart;

        Assert.AreEqual("New session", shell.DisplaySessionName);
        Assert.IsFalse(shell.HasActiveRun);
        Assert.IsNull(shell.ContentErrorMessage);
        Assert.AreEqual(session.Id, client.Sessions.Single().Id);
    }

    [DataTestMethod]
    [DataRow("completed")]
    [DataRow("cancelled")]
    [DataRow("failed")]
    [DataRow("interrupted")]
    public async Task TerminalLifecycle_ImmediatelyReenablesRerunWithoutSetupEdit(string terminalStatus)
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Rerun", Path.GetTempPath());
        using var shell = CreateShell(client);
        await shell.InitializeAsync();

        await shell.StartRunCommand.ExecuteAsync(null);
        var active = client.Runs.Single();
        var terminal = active with
        {
            Status = terminalStatus,
            CompletedAt = DateTimeOffset.UtcNow,
            ErrorMessage = terminalStatus is "failed" or "interrupted" ? "Run stopped." : null,
        };
        client.RaiseLifecycle($"run.{terminalStatus}", terminal);

        Assert.IsFalse(shell.HasActiveRun);
        Assert.IsTrue(shell.Setup.CanStart);
        Assert.IsTrue(shell.StartRunCommand.CanExecute(null));
        Assert.AreEqual(active.Id, shell.History.SelectedRun?.Id);
    }

    [TestMethod]
    public async Task UnexpectedExit_OffersRestartReconcilesRunAndPreservesCompletedHistory()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Recovery", Path.GetTempPath());
        var completed = client.AddRun(session.Id, "completed");
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        await shell.StartRunCommand.ExecuteAsync(null);
        var abandoned = client.Runs.Single(run => run.Id != completed.Id);

        client.RaiseUnexpectedExit(23);

        Assert.IsTrue(shell.IsRecoveryRequired);
        Assert.IsFalse(shell.HasActiveRun);
        Assert.AreEqual("Interrupted", shell.Progress.Status);
        Assert.IsTrue(shell.RestartWorkerCommand.CanExecute(null));

        await shell.RestartWorkerCommand.ExecuteAsync(null);

        Assert.AreEqual(1, client.RestartCount);
        Assert.IsTrue(shell.IsConnected);
        Assert.AreEqual("interrupted", client.Runs.Single(run => run.Id == abandoned.Id).Status);
        Assert.AreEqual("completed", client.Runs.Single(run => run.Id == completed.Id).Status);
        Assert.IsTrue(shell.StartRunCommand.CanExecute(null));
    }

    private static ShellViewModel CreateShell(TestWorkerClient client) =>
        new(
            client,
            new TestFolderPicker(),
            new TestConfirmation(),
            new ImmediateDispatcher(),
            new TestClipboard(),
            new TestExplorer(),
            new TestCloudLocationService());
}
