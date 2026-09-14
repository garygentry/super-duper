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
        shell.OpenScanCommand.Execute(null);
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
        Assert.IsFalse(shell.DuplicateFiles.IsLoading);
        Assert.AreEqual(0, groupRequests);
        shell.SelectedDestination = WorkspaceDestination.FileResults;
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
        Assert.IsNull(shell.DuplicateFiles.Run);
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

        Assert.AreEqual("New saved scan", shell.DisplaySessionName);
        Assert.IsTrue(shell.Setup.CanEdit);
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

    [TestMethod]
    public async Task HighlightAndHistoryRefreshDoNotOpenOrResetWorkspacePanes()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Archive", Path.GetTempPath());
        var old = client.AddRun(session.Id, "completed");
        var latest = client.AddRun(session.Id, "completed");
        var files = 0;
        var folders = 0;
        var reviews = 0;
        client.GroupPageHandler = (_, _) => { files++; return Task.FromResult(new WorkerDuplicateFileGroupPage([], 0, null, null)); };
        client.FolderGroupPageHandler = (_, _) => { folders++; return Task.FromResult(new WorkerDuplicateFolderGroupPage([], 0, null, null)); };
        client.LatestPreflightHandler = (_, _) => { reviews++; return Task.FromResult<WorkerPreflight?>(null); };
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        Assert.AreEqual(0, files + folders + reviews);
        shell.SelectedDestination = WorkspaceDestination.FileResults;
        shell.DuplicateFiles.SearchText = "family";
        var groups = shell.DuplicateFiles.Groups;
        shell.SelectedDestination = WorkspaceDestination.FolderResults;
        shell.SelectedDestination = WorkspaceDestination.Review;
        shell.SelectedDestination = WorkspaceDestination.History;
        shell.History.SelectedRun = shell.History.Runs.Single(run => run.Id == old.Id);
        await shell.History.RefreshCommand.ExecuteAsync(null);
        Assert.AreEqual(latest.Id, shell.SelectedRun?.Id);
        Assert.AreEqual(latest.Id, client.ObservedLiveRun?.Id);
        shell.SelectedDestination = WorkspaceDestination.FileResults;
        Assert.AreSame(groups, shell.DuplicateFiles.Groups);
        Assert.AreEqual("family", shell.DuplicateFiles.SearchText);
        Assert.AreEqual(1, files);
        Assert.AreEqual(1, folders);
        Assert.AreEqual(1, reviews);
        shell.OpenScanCommand.Execute(null);
        Assert.AreEqual(old.Id, shell.SelectedRun?.Id);
        Assert.AreEqual(old.Id, shell.DuplicateFiles.Run?.Id);
        Assert.IsNull(shell.DuplicateFolders.Run);
        Assert.IsFalse(shell.Preflight.HasRun);
        Assert.AreEqual(2, files);
        Assert.IsFalse(shell.Preflight.Operation.CanSubmit);
    }

    [DataTestMethod]
    [DataRow(false)]
    [DataRow(true)]
    public async Task DelayedOpenCannotReplaceNewRunOrStealNavigation(bool fail)
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Archive", Path.GetTempPath());
        var old = client.AddRun(session.Id, "completed");
        var latest = client.AddRun(session.Id, "completed");
        var delayed = new TaskCompletionSource<WorkerDuplicateFileGroupPage>();
        client.GroupPageHandler = (query, _) => query.RunId == old.Id
            ? delayed.Task : Task.FromResult(new WorkerDuplicateFileGroupPage([], 0, null, null));
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.History.SelectedRun = shell.History.Runs.Single(run => run.Id == old.Id);
        shell.OpenScanCommand.Execute(null);
        Assert.IsTrue(shell.DuplicateFiles.IsLoading);
        shell.History.SelectedRun = shell.History.Runs.Single(run => run.Id == latest.Id);
        shell.OpenScanCommand.Execute(null);
        shell.SelectedDestination = WorkspaceDestination.Review;
        var focusVersion = shell.FocusRequestVersion;
        if (fail) delayed.SetException(new InvalidOperationException("old files failed"));
        else delayed.SetResult(new WorkerDuplicateFileGroupPage([], 0, null, null));
        await Task.Yield();
        Assert.AreEqual(latest.Id, shell.SelectedRun?.Id);
        Assert.AreEqual(latest.Id, shell.DuplicateFiles.Run?.Id);
        Assert.IsNull(shell.DuplicateFiles.ErrorMessage);
        Assert.IsNull(shell.ContentErrorMessage);
        Assert.AreEqual(WorkspaceDestination.Review, shell.SelectedDestination);
        Assert.AreEqual(focusVersion, shell.FocusRequestVersion);
    }

    [TestMethod]
    public async Task CurrentWarningsAcrossSessionsKeepOpenedResultsAndActiveExitIndependent()
    {
        var client = new TestWorkerClient();
        var first = client.AddSession("Active locations", Path.GetTempPath());
        var second = client.AddSession("Archive", Path.GetTempPath());
        var active = client.AddRun(first.Id, "running", "discovering") with { WarningCount = 1 };
        client.Runs[0] = active;
        var old = client.AddRun(second.Id, "completed");
        long? warningRun = null;
        client.RunWarningsHandler = (query, _) =>
        {
            warningRun = query.RunId;
            return Task.FromResult(new WorkerRunWarningPage([], 0, 0, 1, 1, "active", "running", TestWorkerClient.DiagnosticLog, null, false));
        };
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.Sessions.SelectedSession = shell.Sessions.Find(second.Id);
        shell.SelectedDestination = WorkspaceDestination.FileResults;
        var groups = shell.DuplicateFiles.Groups;
        await shell.Progress.OpenWarningsCommand.ExecuteAsync(null);
        Assert.AreEqual(active.Id, warningRun);
        Assert.AreEqual(active.Id, shell.History.SelectedRun?.Id);
        Assert.AreEqual(old.Id, shell.SelectedRun?.Id);
        Assert.AreEqual("Archive", shell.WorkspaceSessionName);
        StringAssert.Contains(shell.HistoryContext, "Active locations");
        Assert.AreEqual("Start scan: Active locations", shell.StartRunLabel);
        Assert.AreSame(groups, shell.DuplicateFiles.Groups);
        StringAssert.Contains(shell.History.WarningContextIdentity, $"Scan {active.Id}");
        Assert.AreEqual("_Return to progress", shell.History.WarningReturnLabel);
        shell.History.CloseWarningsCommand.Execute(null);
        Assert.AreEqual(WorkspaceDestination.ScanProgress, shell.SelectedDestination);
        Assert.AreEqual("progress-warnings", shell.FocusTarget);
        Assert.AreEqual(active.Id, shell.Progress.Run?.Id);
        client.RaiseUnexpectedExit(23);
        Assert.AreEqual(old.Id, shell.SelectedRun?.Id);
        Assert.AreEqual(old.Id, shell.DuplicateFiles.Run?.Id);
        Assert.AreEqual(WorkspaceDestination.ScanProgress, shell.SelectedDestination);
        Assert.IsTrue(shell.IsRecoveryRequired);
        Assert.IsFalse(shell.HasActiveRun);
    }

    [TestMethod]
    public async Task DelayedCrossSessionWarningEntryDoesNotNavigateAfterUserLeaves()
    {
        var client = new TestWorkerClient();
        var first = client.AddSession("Active", Path.GetTempPath());
        var second = client.AddSession("Archive", Path.GetTempPath());
        var active = client.AddRun(first.Id, "running", "discovering") with { WarningCount = 1 };
        client.Runs[0] = active;
        var old = client.AddRun(second.Id, "completed");
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.Sessions.SelectedSession = shell.Sessions.Find(second.Id);
        var delayed = new TaskCompletionSource<WorkerSessionDefinition>();
        client.SessionHandler = (_, _) => delayed.Task;
        var opening = shell.Progress.OpenWarningsCommand.ExecuteAsync(null);
        shell.SelectedDestination = WorkspaceDestination.Review;
        delayed.SetResult(first);
        await opening;
        Assert.AreEqual(old.Id, shell.SelectedRun?.Id);
        Assert.AreEqual(WorkspaceDestination.Review, shell.SelectedDestination);
        Assert.IsFalse(shell.History.IsWarningDrilldownOpen);
    }

    [DataTestMethod]
    [DataRow("cancelled")]
    [DataRow("failed")]
    [DataRow("interrupted")]
    public async Task OpeningIncompleteRunRoutesToScanSummary(string status)
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Archive", Path.GetTempPath());
        var stopped = client.AddRun(session.Id, status);
        var active = client.AddRun(session.Id, "running", "discovering");
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.History.SelectedRun = shell.History.Runs.Single(run => run.Id == stopped.Id);
        shell.OpenScanCommand.Execute(null);
        Assert.AreEqual(WorkspaceArea.Scan, shell.SelectedArea);
        Assert.AreEqual(stopped.Id, shell.Summary.Run?.Id);
        Assert.AreEqual(active.Id, shell.Progress.Run?.Id);
        Assert.IsFalse(shell.Summary.CanCancel);
        Assert.AreEqual(WorkspaceDestination.ScanSummary, shell.SelectedDestination);
        Assert.IsNull(shell.DuplicateFiles.Run);
        Assert.IsFalse(shell.Preflight.HasRun);
    }

    [TestMethod]
    public async Task ScanAgainUsesCurrentSetupAndStartPreservesHistoricalRun()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Archive", Path.GetTempPath());
        var old = client.AddRun(session.Id, "completed");
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.SelectedDestination = WorkspaceDestination.FileResults;
        shell.ScanAgainCommand.Execute(null);
        Assert.AreEqual(WorkspaceDestination.ScanSetup, shell.SelectedDestination);
        Assert.AreEqual(old, shell.SelectedRun);
        Assert.AreEqual(session.Roots[0], shell.Setup.Roots[0].Path);
        shell.Setup.Name = "Updated archive";
        shell.Setup.IgnorePatternsText = "**/*.tmp";
        shell.Setup.RepeatCachePolicy = RepeatCachePolicyNames.RevalidateContent;
        shell.ScanAgainCommand.Execute(null);
        Assert.AreEqual("Updated archive", shell.Setup.Name, "Reopening setup must retain drafts.");
        await shell.StartRunCommand.ExecuteAsync(null);
        var current = shell.SelectedRun!;
        Assert.AreNotEqual(old.Id, current.Id);
        Assert.IsNotNull(current.StartedAt);
        Assert.AreEqual("Updated archive", client.Sessions.Single().Name);
        CollectionAssert.AreEqual(new[] { "**/*.tmp" }, current.Parameters.IgnorePatterns.ToArray());
        Assert.AreEqual(RepeatCachePolicyNames.RevalidateContent, current.Parameters.RepeatCachePolicy);
        Assert.AreEqual(old, client.Runs.Single(run => run.Id == old.Id));
        Assert.AreEqual(2, shell.History.Runs.Count);
        Assert.IsFalse(shell.StartRunCommand.CanExecute(null));
        shell.History.SelectedRun = shell.History.Runs.Single(run => run.Id == old.Id);
        shell.OpenScanCommand.Execute(null);
        Assert.AreEqual(old, shell.SelectedRun);
        Assert.AreEqual(current.Id, shell.Progress.Run?.Id);
    }

    [DataTestMethod]
    [DataRow("stay")]
    [DataRow("save")]
    [DataRow("discard")]
    public async Task LeavingDirtySetupRequiresAnExplicitChoice(string choice)
    {
        var client = new TestWorkerClient();
        var first = client.AddSession("First", Path.GetTempPath());
        var second = client.AddSession("Second", Path.GetTempPath());
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.Setup.Name = "Draft";
        shell.Sessions.SelectedSession = shell.Sessions.Find(second.Id);
        Assert.IsTrue(shell.HasSetupDeparture);
        Assert.AreEqual(first.Id, shell.Sessions.SelectedSession?.Id);
        if (choice == "stay") shell.StayInSetupCommand.Execute(null);
        else if (choice == "save") await shell.SaveSetupAndContinueCommand.ExecuteAsync(null);
        else await shell.DiscardSetupAndContinueCommand.ExecuteAsync(null);
        Assert.IsFalse(shell.HasSetupDeparture);
        Assert.AreEqual(choice == "stay" ? first.Id : second.Id, shell.Setup.SessionId);
        Assert.AreEqual(choice == "save" ? "Draft" : "First", client.Sessions.Single(s => s.Id == first.Id).Name);
        if (choice == "stay") Assert.AreEqual("Draft", shell.Setup.Name);
    }

    [TestMethod]
    public async Task InvalidDraftCannotStartOrSaveAndDepartureStaysPending()
    {
        var client = new TestWorkerClient();
        client.AddSession("First", Path.GetTempPath());
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.Setup.ManualLocationExclusionsText = "relative-path";
        shell.SelectedDestination = WorkspaceDestination.History;
        Assert.AreEqual(WorkspaceDestination.ScanSetup, shell.SelectedDestination);
        await shell.SaveSetupAndContinueCommand.ExecuteAsync(null);
        Assert.IsTrue(shell.HasSetupDeparture);
        Assert.IsTrue(shell.Setup.HasOperationError);
        await shell.StartRunCommand.ExecuteAsync(null);
        Assert.AreEqual(0, client.Runs.Count);
        await shell.DiscardSetupAndContinueCommand.ExecuteAsync(null);
        Assert.AreEqual(WorkspaceDestination.History, shell.SelectedDestination);
    }

    [TestMethod]
    public async Task PendingStartLocksEditsUntilFailureAndRetainsSavedSetup()
    {
        var client = new TestWorkerClient();
        client.AddSession("First", Path.GetTempPath());
        var pending = new TaskCompletionSource<WorkerRun>();
        client.StartRunHandler = (_, _, _) => pending.Task;
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.Setup.IgnorePatternsText = "**/*.tmp";
        var starting = shell.StartRunCommand.ExecuteAsync(null);
        Assert.IsFalse(shell.Setup.CanEdit);
        Assert.IsFalse(shell.StartRunCommand.CanExecute(null));
        pending.SetException(new InvalidOperationException("Start rejected"));
        await starting;
        Assert.IsTrue(shell.Setup.CanEdit);
        Assert.IsFalse(shell.Setup.IsDirty);
        Assert.AreEqual("**/*.tmp", shell.Setup.IgnorePatternsText);
        Assert.AreEqual("Start rejected", shell.ContentErrorMessage);
    }

    [TestMethod]
    public async Task ConfirmedDefinitionDeletionDoesNotOfferToSaveTheDeletedDraft()
    {
        var client = new TestWorkerClient();
        var first = client.AddSession("First", Path.GetTempPath());
        var second = client.AddSession("Second", Path.GetTempPath());
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        shell.Setup.Name = "Draft";
        await shell.Setup.DeleteCommand.ExecuteAsync(null);
        Assert.IsFalse(shell.HasSetupDeparture);
        Assert.IsFalse(client.Sessions.Any(s => s.Id == first.Id));
        Assert.AreEqual(second.Id, shell.Setup.SessionId);
        Assert.IsFalse(shell.Preflight.Operation.CanSubmit);
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
