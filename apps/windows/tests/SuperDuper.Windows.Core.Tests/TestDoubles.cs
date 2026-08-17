using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

internal sealed class TestWorkerClient : IRestartableWorkerClient
{
    private long _nextSessionId;
    private long _nextRunId;

    public event EventHandler<WorkerRunProgressEventArgs>? RunProgress;

    public event EventHandler<WorkerRunLifecycleEventArgs>? RunLifecycleChanged;

    public event EventHandler<WorkerUnexpectedExitEventArgs>? UnexpectedExit;

    public string ExecutablePath => @"C:\test\super-duper-worker.exe";

    public string DiagnosticLogPath => @"C:\test\logs\worker.log";

    public List<WorkerSessionDefinition> Sessions { get; } = [];

    public List<WorkerRun> Runs { get; } = [];

    public Func<long, CancellationToken, Task<WorkerRun>>? CancelHandler { get; set; }

    public Func<DuplicateFileGroupQuery, CancellationToken, Task<WorkerDuplicateFileGroupPage>>? GroupPageHandler { get; set; }

    public Func<DuplicateFileSelectedRootFacetQuery, CancellationToken, Task<WorkerDuplicateFileSelectedRootFacetPage>>? RootFacetPageHandler { get; set; }

    public Func<DuplicateFileMemberQuery, CancellationToken, Task<WorkerDuplicateFileMemberPage>>? MemberPageHandler { get; set; }

    public Func<DuplicateFolderGroupQuery, CancellationToken, Task<WorkerDuplicateFolderGroupPage>>? FolderGroupPageHandler { get; set; }

    public Func<DuplicateFolderMemberQuery, CancellationToken, Task<WorkerDuplicateFolderMemberPage>>? FolderMemberPageHandler { get; set; }

    public int RestartCount { get; private set; }

    public Task<WorkerHelloResult> ConnectAsync(CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerHelloResult(1, "test-worker", "test-engine"));

    public Task<WorkerHelloResult> RestartAsync(CancellationToken cancellationToken = default)
    {
        RestartCount++;
        for (var index = 0; index < Runs.Count; index++)
        {
            if (Runs[index].Status is "pending" or "running" or "cancelling")
            {
                Runs[index] = Runs[index] with
                {
                    Status = "interrupted",
                    CompletedAt = DateTimeOffset.UtcNow,
                    ErrorMessage = "The worker exited before this run finished.",
                };
            }
        }
        return ConnectAsync(cancellationToken);
    }

    public Task<WorkerSessionPage> ListSessionsAsync(long offset = 0, int limit = 100, CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerSessionPage(Sessions.Skip((int)offset).Take(limit).ToArray(), Sessions.Count));

    public Task<WorkerSessionDefinition> GetSessionAsync(long sessionId, CancellationToken cancellationToken = default) =>
        Task.FromResult(Sessions.Single(session => session.Id == sessionId));

    public Task<WorkerSessionDefinition> CreateSessionAsync(
        string name,
        IReadOnlyList<string> roots,
        IReadOnlyList<string> ignorePatterns,
        string cloudPolicy,
        IReadOnlyList<string> manualLocationExclusions,
        IReadOnlyList<WorkerRegisteredCloudLocation> registeredCloudLocations,
        string cloudDetectionStatus,
        CancellationToken cancellationToken = default)
    {
        var now = DateTimeOffset.UtcNow;
        var session = new WorkerSessionDefinition(
            ++_nextSessionId,
            name,
            roots.ToArray(),
            ignorePatterns.ToArray(),
            cloudPolicy,
            manualLocationExclusions.ToArray(),
            registeredCloudLocations.ToArray(),
            cloudDetectionStatus,
            now,
            now);
        Sessions.Add(session);
        return Task.FromResult(session);
    }

    public Task<WorkerSessionDefinition> UpdateSessionAsync(
        long sessionId,
        string name,
        IReadOnlyList<string> roots,
        IReadOnlyList<string> ignorePatterns,
        string cloudPolicy,
        IReadOnlyList<string> manualLocationExclusions,
        IReadOnlyList<WorkerRegisteredCloudLocation> registeredCloudLocations,
        string cloudDetectionStatus,
        CancellationToken cancellationToken = default)
    {
        var index = Sessions.FindIndex(session => session.Id == sessionId);
        var updated = Sessions[index] with
        {
            Name = name,
            Roots = roots.ToArray(),
            IgnorePatterns = ignorePatterns.ToArray(),
            CloudPolicy = cloudPolicy,
            ManualLocationExclusions = manualLocationExclusions.ToArray(),
            RegisteredCloudLocations = registeredCloudLocations.ToArray(),
            CloudDetectionStatus = cloudDetectionStatus,
            UpdatedAt = DateTimeOffset.UtcNow,
        };
        Sessions[index] = updated;
        return Task.FromResult(updated);
    }

    public Task DeleteSessionAsync(long sessionId, CancellationToken cancellationToken = default)
    {
        Sessions.RemoveAll(session => session.Id == sessionId);
        Runs.RemoveAll(run => run.SessionId == sessionId);
        return Task.CompletedTask;
    }

    public Task<WorkerRunPage> ListRunsAsync(
        long? sessionId = null,
        long offset = 0,
        int limit = 100,
        CancellationToken cancellationToken = default)
    {
        var matching = Runs
            .Where(run => sessionId is null || run.SessionId == sessionId)
            .OrderByDescending(run => run.Id)
            .ToArray();
        return Task.FromResult(new WorkerRunPage(matching.Skip((int)offset).Take(limit).ToArray(), matching.Length));
    }

    public Task<WorkerRun> GetRunAsync(long runId, CancellationToken cancellationToken = default) =>
        Task.FromResult(Runs.Single(run => run.Id == runId));

    public Task<WorkerRunExclusionPage> GetRunExclusionsAsync(
        long runId,
        long offset = 0,
        int limit = 100,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerRunExclusionPage([], 0));

    public Task<WorkerRun> StartRunAsync(long sessionId, CancellationToken cancellationToken = default)
    {
        var session = Sessions.Single(value => value.Id == sessionId);
        var now = DateTimeOffset.UtcNow;
        var run = CreateRun(++_nextRunId, session.Id, "running", "discovering", now);
        run = run with
        {
            Parameters = new WorkerRunParameters(
                session.Roots,
                session.IgnorePatterns,
                500,
                session.CloudPolicy,
                session.ManualLocationExclusions,
                session.RegisteredCloudLocations,
                session.CloudDetectionStatus),
        };
        Runs.Add(run);
        return Task.FromResult(run);
    }

    public Task<WorkerRun> CancelRunAsync(long runId, CancellationToken cancellationToken = default)
    {
        if (CancelHandler is not null)
        {
            return CancelHandler(runId, cancellationToken);
        }
        var index = Runs.FindIndex(run => run.Id == runId);
        var run = Runs[index] with { Status = "cancelling" };
        Runs[index] = run;
        return Task.FromResult(run);
    }

    public Task<WorkerDuplicateFileGroupPage> GetDuplicateFileGroupsAsync(
        DuplicateFileGroupQuery query,
        CancellationToken cancellationToken = default) =>
        GroupPageHandler?.Invoke(query, cancellationToken)
        ?? Task.FromResult(new WorkerDuplicateFileGroupPage([], 0, null, null));

    public Task<WorkerDuplicateFileMemberPage> GetDuplicateFileGroupMembersAsync(
        DuplicateFileMemberQuery query,
        CancellationToken cancellationToken = default) =>
        MemberPageHandler?.Invoke(query, cancellationToken)
        ?? Task.FromResult(new WorkerDuplicateFileMemberPage([], 0, null, null));

    public Task<WorkerDuplicateFileSelectedRootFacetPage> GetDuplicateFileSelectedRootFacetsAsync(
        DuplicateFileSelectedRootFacetQuery query,
        CancellationToken cancellationToken = default) =>
        RootFacetPageHandler?.Invoke(query, cancellationToken)
        ?? Task.FromResult(new WorkerDuplicateFileSelectedRootFacetPage([], 0, null, null));

    public Task<WorkerDuplicateFolderGroupPage> GetDuplicateFolderGroupsAsync(
        DuplicateFolderGroupQuery query,
        CancellationToken cancellationToken = default) =>
        FolderGroupPageHandler?.Invoke(query, cancellationToken)
        ?? Task.FromResult(new WorkerDuplicateFolderGroupPage([], 0, null, null));

    public Task<WorkerDuplicateFolderMemberPage> GetDuplicateFolderGroupMembersAsync(
        DuplicateFolderMemberQuery query,
        CancellationToken cancellationToken = default) =>
        FolderMemberPageHandler?.Invoke(query, cancellationToken)
        ?? Task.FromResult(new WorkerDuplicateFolderMemberPage([], 0, null, null));

    public ValueTask DisposeAsync() => ValueTask.CompletedTask;

    public WorkerSessionDefinition AddSession(string name, params string[] roots)
    {
        var now = DateTimeOffset.UtcNow;
        var session = new WorkerSessionDefinition(
            ++_nextSessionId,
            name,
            roots,
            [],
            CloudPolicyNames.ExcludeRegisteredRoots,
            [],
            [],
            CloudDetectionStatusNames.Complete,
            now,
            now);
        Sessions.Add(session);
        return session;
    }

    public WorkerRun AddRun(long sessionId, string status, string? phase = "finalizing")
    {
        var run = CreateRun(++_nextRunId, sessionId, status, phase, DateTimeOffset.UtcNow.AddMinutes(-2));
        Runs.Add(run);
        return run;
    }

    public void RaiseProgress(WorkerRunProgressEventArgs progress) => RunProgress?.Invoke(this, progress);

    public void RaiseLifecycle(string eventName, WorkerRun run)
    {
        var index = Runs.FindIndex(existing => existing.Id == run.Id);
        if (index >= 0)
        {
            Runs[index] = run;
        }
        RunLifecycleChanged?.Invoke(
            this,
            new WorkerRunLifecycleEventArgs { EventName = eventName, Run = run });
    }

    public void RaiseUnexpectedExit(int exitCode = -1) => UnexpectedExit?.Invoke(
        this,
        new WorkerUnexpectedExitEventArgs
        {
            ExitCode = exitCode,
            Message = $"The worker exited unexpectedly with code {exitCode}.",
            ExecutablePath = ExecutablePath,
            DiagnosticLogPath = DiagnosticLogPath,
        });

    internal static WorkerRun CreateRun(
        long id,
        long sessionId,
        string status,
        string? phase,
        DateTimeOffset startedAt) =>
        new(
            id,
            sessionId,
            new WorkerRunParameters(
                [],
                [],
                500,
                CloudPolicyNames.ExcludeRegisteredRoots,
                [],
                [],
                CloudDetectionStatusNames.Complete),
            status,
            phase,
            startedAt,
            startedAt,
            status is "running" or "cancelling" ? null : startedAt.AddMinutes(1),
            12,
            "4096",
            7,
            2,
            0,
            "1024",
            1,
            0,
            status is "failed" or "interrupted" ? "Run did not finish." : null,
            "test-engine");
}

internal sealed class TestFolderPicker(string? selection = null) : IFolderPickerService
{
    public Task<string?> PickFolderAsync(CancellationToken cancellationToken = default) =>
        Task.FromResult(selection);
}

internal sealed class TestConfirmation(bool answer = true) : IUserConfirmationService
{
    public Task<bool> ConfirmAsync(string title, string message, CancellationToken cancellationToken = default) =>
        Task.FromResult(answer);
}

internal sealed class TestCloudLocationService : ICloudLocationService
{
    public TestCloudLocationService(
        string status = CloudDetectionStatusNames.Complete,
        IReadOnlyList<WorkerRegisteredCloudLocation>? locations = null,
        string? errorMessage = null)
    {
        Result = new CloudLocationDetectionResult(status, locations ?? [], errorMessage);
    }

    public CloudLocationDetectionResult Result { get; set; }

    public Task<CloudLocationDetectionResult> DetectAsync(CancellationToken cancellationToken = default) =>
        Task.FromResult(Result);
}

internal sealed class ImmediateDispatcher : IUiDispatcher
{
    public void Post(Action action) => action();
}

internal sealed class TestClipboard : IClipboardService
{
    public string? Text { get; private set; }

    public void CopyText(string text) => Text = text;
}

internal sealed class TestExplorer : IExplorerService
{
    public string? RevealedPath { get; private set; }

    public Exception? Error { get; set; }

    public Task RevealAsync(string path, CancellationToken cancellationToken = default)
    {
        if (Error is not null)
        {
            return Task.FromException(Error);
        }
        RevealedPath = path;
        return Task.CompletedTask;
    }
}
