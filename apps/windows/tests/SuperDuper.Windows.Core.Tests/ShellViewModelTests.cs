using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class ShellViewModelTests
{
    [TestMethod]
    public async Task InitializeAsync_ShowsStartingUntilHelloCompletes()
    {
        var completion = new TaskCompletionSource<WorkerHelloResult>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var viewModel = CreateViewModel(new FakeWorkerClient(_ => completion.Task));

        var initialize = viewModel.InitializeAsync();

        Assert.AreEqual(WorkerConnectionState.Starting, viewModel.ConnectionState);
        Assert.IsTrue(viewModel.IsStarting);

        completion.SetResult(new WorkerHelloResult(1, "0.1.0", "0.1.0"));
        await initialize;
    }

    [TestMethod]
    public async Task InitializeAsync_ChangesToConnectedAfterHello()
    {
        var client = new FakeWorkerClient(
            _ => Task.FromResult(new WorkerHelloResult(1, "0.2.0", "0.3.0")));
        var viewModel = CreateViewModel(client);

        await viewModel.InitializeAsync();

        Assert.AreEqual(WorkerConnectionState.Connected, viewModel.ConnectionState);
        Assert.IsTrue(viewModel.IsConnected);
        Assert.AreEqual("0.2.0", viewModel.WorkerVersion);
        Assert.AreEqual("0.3.0", viewModel.EngineVersion);
        StringAssert.Contains(viewModel.StatusDetail, "Protocol 1");
    }

    [TestMethod]
    public async Task InitializeAsync_ChangesToFailedAndKeepsDiagnostic()
    {
        var client = new FakeWorkerClient(
            _ => Task.FromException<WorkerHelloResult>(new InvalidOperationException("worker unavailable")));
        var viewModel = CreateViewModel(client);

        await viewModel.InitializeAsync();

        Assert.AreEqual(WorkerConnectionState.Failed, viewModel.ConnectionState);
        Assert.IsTrue(viewModel.IsFailed);
        StringAssert.Contains(viewModel.StatusDetail, "worker unavailable");
        Assert.AreEqual(FakeWorkerClient.Path, viewModel.WorkerExecutablePath);
    }

    [TestMethod]
    public async Task ConfirmCancelAndExitAsync_CancelsActivePreflightWithoutDeleting()
    {
        var client = new TestWorkerClient();
        var run = TestWorkerClient.CreateRun(17, 1, "completed", "finalizing", DateTimeOffset.UtcNow);
        client.ReviewPlanHandler = (_, _) => Task.FromResult(new WorkerReviewPlanView(
            new WorkerReviewPlan(4, 17, "active", 6, "created", "updated"),
            new WorkerReviewPlanSummary(1, 1, 1, 0, "100", 1)
            {
                EffectiveRemovalFileCount = 1,
                PlannedRemovalPhysicalItemCount = 1,
            }));
        var running = TestWorkerClient.CreatePreflight(23, 17, "running", 6, 0, 2, 0);
        client.LatestPreflightHandler = (_, _) => Task.FromResult<WorkerPreflight?>(running);
        client.PreflightHandler = async (_, cancellationToken) =>
        {
            await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken);
            return running;
        };
        long? cancelledId = null;
        client.PreflightCancelHandler = (preflightId, _) =>
        {
            cancelledId = preflightId;
            return Task.FromResult(running with { Status = "cancelling" });
        };
        using var viewModel = CreateViewModel(client);
        await viewModel.Preflight.ShowRunAsync(run);

        var shouldExit = await viewModel.ConfirmCancelAndExitAsync();

        Assert.IsTrue(shouldExit);
        Assert.AreEqual(23, cancelledId);
    }

    [TestMethod]
    public async Task RecoveryReviewFreshScanActionNavigatesToSetupAndRestoresStartFocus()
    {
        var client = new TestWorkerClient();
        using var viewModel = CreateViewModel(client);
        viewModel.SelectedTabIndex = 5;
        await viewModel.Preflight.Operation.RecoveryReview.ShowOperationAsync(
            TestWorkerClient.CreateRecycleOperation(8, 12, 7, 4) with
            {
                Status = "recovery_required",
                UnknownCount = 1,
            });

        await viewModel.Preflight.Operation.RecoveryReview.NavigateToFreshScanCommand.ExecuteAsync(null);

        Assert.AreEqual(0, viewModel.SelectedTabIndex);
        Assert.AreEqual("start-scan", viewModel.FocusTarget);
        Assert.IsTrue(viewModel.FocusRequestVersion > 0);
    }

    [TestMethod]
    public async Task HashWarningActionOpensItsImmutableDuplicateSetAndRequestsGroupFocus()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Warning target", @"C:\Data");
        var run = client.AddRun(session.Id, "completed") with { WarningCount = 1 };
        client.Runs[0] = run;
        var warning = new WorkerRunWarningAggregate(
            91,
            run.Id,
            "hashing",
            "scan",
            RunHistoryViewModel.HashWarningCode,
            "warning",
            "Some candidate files could not be hashed.",
            1,
            ["bounded example"]);
        client.RunWarningsHandler = (_, _) => Task.FromResult(
            new WorkerRunWarningPage(
                [warning], 1, 1, 1, 10, "terminal", "completed", TestWorkerClient.DiagnosticLog, null, false));
        using var viewModel = CreateViewModel(client);
        await viewModel.InitializeAsync();
        viewModel.SelectedTabIndex = 2;
        await viewModel.History.OpenWarningsCommand.ExecuteAsync(null);

        await viewModel.History.NavigateWarningCommand.ExecuteAsync(warning);

        Assert.AreEqual(3, viewModel.SelectedTabIndex);
        Assert.AreEqual(run.Id, viewModel.DuplicateFiles.Run?.Id);
        Assert.AreEqual("duplicate-file-groups", viewModel.FocusTarget);
        Assert.IsTrue(viewModel.FocusRequestVersion > 0);
        Assert.AreEqual(1, viewModel.History.Warnings.Count);
    }

    [TestMethod]
    public async Task OneCoalescedWorkerFrameProducesOneDispatcherUpdate()
    {
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([], 0, null, null)),
        };
        var dispatcher = new CountingDispatcher();
        using var viewModel = new ShellViewModel(
            client,
            new TestFolderPicker(),
            new TestConfirmation(),
            dispatcher,
            new TestClipboard(),
            new TestExplorer(),
            new TestCloudLocationService());
        await viewModel.DuplicateFiles.ShowRunAsync(
            TestWorkerClient.CreateRun(70, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        client.RaiseResultStateChanged(new WorkerResultStateChangedEventArgs
        {
            Kind = "hints",
            RunId = 70,
            RootPath = @"C:\Data",
            EventCount = 10_000,
            CoalescedPathCount = 200,
            ExecutorEnabled = false,
        });

        Assert.AreEqual(1, dispatcher.PostCount);
        StringAssert.Contains(viewModel.DuplicateFiles.LiveHintStatusMessage, "10,000 filesystem events");
    }

    [TestMethod]
    public async Task ThousandProgressFramesQueueOneDispatcherApplicationAndPreserveLatest()
    {
        var client = new TestWorkerClient();
        var dispatcher = new QueuedDispatcher();
        using var viewModel = CreateViewModel(client, dispatcher);
        var run = client.AddRun(1, "running", "discovering");
        viewModel.Progress.ShowRun(run);
        await Task.Delay(100);
        dispatcher.ExecuteAll();
        client.RaiseLifecycle("run.started", run);
        dispatcher.ExecuteAll();
        await Task.Delay(100);
        dispatcher.ExecuteAll();

        for (ulong sequence = 1; sequence <= 1_000; sequence++)
        {
            client.RaiseProgress(ProgressTestData.Discovery(
                run.Id,
                sequence,
                revision: sequence,
                discoveredFiles: sequence,
                monotonicNanos: sequence * 1_000_000));
        }

        await WaitUntilAsync(() => dispatcher.PendingCount == 1);
        Assert.AreEqual(
            run.FilesDiscovered.ToString(),
            viewModel.Progress.FilesDiscovered.Replace(",", string.Empty));
        dispatcher.ExecuteNext();
        await WaitUntilAsync(() =>
            viewModel.Progress.FilesDiscovered.Replace(",", string.Empty) == "1000");

        dispatcher.ExecuteAll();
        Assert.AreEqual("1000", viewModel.Progress.FilesDiscovered.Replace(",", string.Empty));
        Assert.AreEqual(
            1L,
            viewModel.Progress.ProgressAnnouncementVersion,
            "Only the one accepted dispatcher application may advance the progress announcement.");
    }

    [TestMethod]
    public async Task TerminalLifecycleInvalidatesAlreadyPostedProgressBeforeUiExecution()
    {
        var client = new TestWorkerClient();
        var dispatcher = new QueuedDispatcher();
        using var viewModel = CreateViewModel(client, dispatcher);
        var run = client.AddRun(1, "running", "discovering");
        viewModel.Progress.ShowRun(run);
        await Task.Delay(100);
        dispatcher.ExecuteAll();
        client.RaiseLifecycle("run.started", run);
        dispatcher.ExecuteAll();
        await Task.Delay(100);
        dispatcher.ExecuteAll();
        client.RaiseProgress(ProgressTestData.Discovery(
            run.Id,
            discoveredFiles: 99));
        await WaitUntilAsync(() => dispatcher.PendingCount == 1);

        var completed = run with
        {
            Status = "completed",
            CompletedAt = DateTimeOffset.UtcNow,
        };
        client.RaiseLifecycle("run.completed", completed);
        Assert.AreEqual(2, dispatcher.PendingCount);
        dispatcher.ExecuteAll();

        Assert.AreEqual("Completed", viewModel.Progress.Status);
        Assert.AreEqual(
            run.FilesDiscovered.ToString(),
            viewModel.Progress.FilesDiscovered.Replace(",", string.Empty));
        await Task.Delay(150);
        Assert.AreEqual(0, dispatcher.PendingCount);
    }

    private static ShellViewModel CreateViewModel(IWorkerClient client) =>
        CreateViewModel(client, new ImmediateDispatcher());

    private static ShellViewModel CreateViewModel(IWorkerClient client, IUiDispatcher dispatcher) =>
        new(
            client,
            new TestFolderPicker(),
            new TestConfirmation(),
            dispatcher,
            new TestClipboard(),
            new TestExplorer(),
            new TestCloudLocationService());

    private static async Task WaitUntilAsync(Func<bool> predicate)
    {
        for (var attempt = 0; attempt < 1_000 && !predicate(); attempt++)
        {
            await Task.Delay(1);
        }
        Assert.IsTrue(predicate(), "The progress application did not reach the expected state.");
    }

    private sealed class TestFolderPicker : IFolderPickerService
    {
        public Task<string?> PickFolderAsync(CancellationToken cancellationToken = default) =>
            Task.FromResult<string?>(null);
    }

    private sealed class TestConfirmation : IUserConfirmationService
    {
        public Task<bool> ConfirmAsync(string title, string message, CancellationToken cancellationToken = default) =>
            Task.FromResult(true);
    }

    private sealed class ImmediateDispatcher : IUiDispatcher
    {
        public void Post(Action action) => action();
    }

    private sealed class CountingDispatcher : IUiDispatcher
    {
        public int PostCount { get; private set; }

        public void Post(Action action)
        {
            PostCount++;
            action();
        }
    }

    private sealed class QueuedDispatcher : IUiDispatcher
    {
        private readonly object _gate = new();
        private readonly Queue<Action> _pending = new();

        public int PendingCount
        {
            get
            {
                lock (_gate)
                {
                    return _pending.Count;
                }
            }
        }

        public void Post(Action action)
        {
            lock (_gate)
            {
                _pending.Enqueue(action);
            }
        }

        public void ExecuteNext()
        {
            Action action;
            lock (_gate)
            {
                action = _pending.Dequeue();
            }
            action();
        }

        public void ExecuteAll()
        {
            while (PendingCount > 0)
            {
                ExecuteNext();
            }
        }
    }

    private sealed class FakeWorkerClient(
        Func<CancellationToken, Task<WorkerHelloResult>> connect) : IWorkerClient
    {
        public const string Path = @"C:\test\super-duper-worker.exe";

        public event EventHandler<WorkerRunProgressEventArgs>? RunProgress
        {
            add { }
            remove { }
        }

        public event EventHandler<WorkerRunLifecycleEventArgs>? RunLifecycleChanged
        {
            add { }
            remove { }
        }

        public string ExecutablePath => Path;

        public string DiagnosticLogPath => @"C:\test\logs\worker.log";

        public Task<WorkerHelloResult> ConnectAsync(CancellationToken cancellationToken = default) =>
            connect(cancellationToken);

        public Task<WorkerSessionPage> ListSessionsAsync(long offset = 0, int limit = 100, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerSessionDefinition> GetSessionAsync(long sessionId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerSessionDefinition> CreateSessionAsync(string name, IReadOnlyList<string> roots, IReadOnlyList<string> ignorePatterns, string cloudPolicy, IReadOnlyList<string> manualLocationExclusions, IReadOnlyList<WorkerRegisteredCloudLocation> registeredCloudLocations, string cloudDetectionStatus, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerSessionDefinition> UpdateSessionAsync(long sessionId, string name, IReadOnlyList<string> roots, IReadOnlyList<string> ignorePatterns, string cloudPolicy, IReadOnlyList<string> manualLocationExclusions, IReadOnlyList<WorkerRegisteredCloudLocation> registeredCloudLocations, string cloudDetectionStatus, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task DeleteSessionAsync(long sessionId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerRunPage> ListRunsAsync(long? sessionId = null, long offset = 0, int limit = 100, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerRun> GetRunAsync(long runId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerRunExclusionPage> GetRunExclusionsAsync(long runId, long offset = 0, int limit = 100, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<WorkerRunWarningPage> GetRunWarningsAsync(RunWarningQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerRun> StartRunAsync(long sessionId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerRun> CancelRunAsync(long runId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerDuplicateFileGroupPage> GetDuplicateFileGroupsAsync(DuplicateFileGroupQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerDuplicateFileSelectedRootFacetPage> GetDuplicateFileSelectedRootFacetsAsync(DuplicateFileSelectedRootFacetQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<WorkerDuplicateFileDriveFacetPage> GetDuplicateFileDriveFacetsAsync(DuplicateFileDriveFacetQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerDuplicateFileMemberPage> GetDuplicateFileGroupMembersAsync(DuplicateFileMemberQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewPlanView> GetReviewPlanAsync(long runId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewGroupPage> GetReviewGroupsAsync(long runId, int pageSize, string? cursor = null, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewDecisionMutation> SetReviewDecisionAsync(string operationId, long runId, long groupId, long fileId, string decision, long expectedRevision, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewLiveValidationResult> ValidateReviewFilesAsync(ReviewLiveValidationRequest request, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<WorkerReviewLiveRootPage> GetDirtyReviewRootsAsync(long runId, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<WorkerReviewLiveRootReconciliationResult> ReconcileDirtyReviewRootAsync(ReviewLiveRootReconciliationRequest request, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewFolderGroupPage> GetReviewFolderGroupsAsync(long runId, int pageSize, string? cursor = null, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewFolderDecisionMutation> SetReviewFolderDecisionAsync(string operationId, long runId, long folderGroupId, long folderMemberId, string decision, long expectedRevision, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreflight?> GetLatestPreflightAsync(long runId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreflight> GetPreflightAsync(long preflightId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreflightStartResult> StartPreflightAsync(string operationId, long runId, long expectedReviewRevision, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreflightItemPage> GetPreflightItemsAsync(PreflightItemQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreflight> CancelPreflightAsync(long preflightId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreferenceRulePage> ListPreferenceRulesAsync(long offset = 0, int limit = 200, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreferenceRule> GetPreferenceRuleAsync(long ruleId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreferenceRuleSaveResult> SavePreferenceRuleAsync(string operationId, long? ruleId, string name, IReadOnlyList<string> roots, long expectedRevision, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreferencePreviewPage> GetPreferencePreviewAsync(PreferencePreviewQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreferenceApplicationResult> ApplyPreferenceRuleAsync(string operationId, long runId, long ruleId, long ruleRevision, long sourceReviewRevision, string previewSignature, PreferencePreviewScope scope, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreferenceApplicationPage> GetPreferenceApplicationsAsync(long runId, long? ruleId, string state, int pageSize, string? cursor = null, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreferenceApplication> GetPreferenceApplicationAsync(long runId, long applicationId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerPreferenceReversalResult> ReversePreferenceApplicationAsync(string operationId, long runId, long applicationId, long expectedRevision, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerDuplicateFolderGroupPage> GetDuplicateFolderGroupsAsync(DuplicateFolderGroupQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerDuplicateFolderMemberPage> GetDuplicateFolderGroupMembersAsync(DuplicateFolderMemberQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public ValueTask DisposeAsync() => ValueTask.CompletedTask;
    }
}
