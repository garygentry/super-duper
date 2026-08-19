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

    private static ShellViewModel CreateViewModel(IWorkerClient client) =>
        new(
            client,
            new TestFolderPicker(),
            new TestConfirmation(),
            new ImmediateDispatcher(),
            new TestClipboard(),
            new TestExplorer(),
            new TestCloudLocationService());

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

        public Task<WorkerRun> StartRunAsync(long sessionId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerRun> CancelRunAsync(long runId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerDuplicateFileGroupPage> GetDuplicateFileGroupsAsync(DuplicateFileGroupQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerDuplicateFileSelectedRootFacetPage> GetDuplicateFileSelectedRootFacetsAsync(DuplicateFileSelectedRootFacetQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<WorkerDuplicateFileDriveFacetPage> GetDuplicateFileDriveFacetsAsync(DuplicateFileDriveFacetQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerDuplicateFileMemberPage> GetDuplicateFileGroupMembersAsync(DuplicateFileMemberQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewPlanView> GetReviewPlanAsync(long runId, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewGroupPage> GetReviewGroupsAsync(long runId, int pageSize, string? cursor = null, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewDecisionMutation> SetReviewDecisionAsync(string operationId, long runId, long groupId, long fileId, string decision, long expectedRevision, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewFolderGroupPage> GetReviewFolderGroupsAsync(long runId, int pageSize, string? cursor = null, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerReviewFolderDecisionMutation> SetReviewFolderDecisionAsync(string operationId, long runId, long folderGroupId, long folderMemberId, string decision, long expectedRevision, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerDuplicateFolderGroupPage> GetDuplicateFolderGroupsAsync(DuplicateFolderGroupQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public Task<WorkerDuplicateFolderMemberPage> GetDuplicateFolderGroupMembersAsync(DuplicateFolderMemberQuery query, CancellationToken cancellationToken = default) => throw new NotSupportedException();

        public ValueTask DisposeAsync() => ValueTask.CompletedTask;
    }
}
