namespace SuperDuper.Windows.Core.Workers;

public interface IWorkerClient : IAsyncDisposable
{
    event EventHandler<WorkerRunProgressEventArgs>? RunProgress;

    event EventHandler<WorkerRunLifecycleEventArgs>? RunLifecycleChanged;

    string ExecutablePath { get; }

    string DiagnosticLogPath { get; }

    Task<WorkerHelloResult> ConnectAsync(CancellationToken cancellationToken = default);

    Task<WorkerSessionPage> ListSessionsAsync(
        long offset = 0,
        int limit = 100,
        CancellationToken cancellationToken = default);

    Task<WorkerSessionDefinition> GetSessionAsync(
        long sessionId,
        CancellationToken cancellationToken = default);

    Task<WorkerSessionDefinition> CreateSessionAsync(
        string name,
        IReadOnlyList<string> roots,
        IReadOnlyList<string> ignorePatterns,
        string cloudPolicy,
        IReadOnlyList<string> manualLocationExclusions,
        IReadOnlyList<WorkerRegisteredCloudLocation> registeredCloudLocations,
        string cloudDetectionStatus,
        CancellationToken cancellationToken = default);

    Task<WorkerSessionDefinition> UpdateSessionAsync(
        long sessionId,
        string name,
        IReadOnlyList<string> roots,
        IReadOnlyList<string> ignorePatterns,
        string cloudPolicy,
        IReadOnlyList<string> manualLocationExclusions,
        IReadOnlyList<WorkerRegisteredCloudLocation> registeredCloudLocations,
        string cloudDetectionStatus,
        CancellationToken cancellationToken = default);

    Task DeleteSessionAsync(long sessionId, CancellationToken cancellationToken = default);

    Task<WorkerRunPage> ListRunsAsync(
        long? sessionId = null,
        long offset = 0,
        int limit = 100,
        CancellationToken cancellationToken = default);

    Task<WorkerRun> GetRunAsync(long runId, CancellationToken cancellationToken = default);

    Task<WorkerRunExclusionPage> GetRunExclusionsAsync(
        long runId,
        long offset = 0,
        int limit = 100,
        CancellationToken cancellationToken = default);

    Task<WorkerRun> StartRunAsync(long sessionId, CancellationToken cancellationToken = default);

    Task<WorkerRun> CancelRunAsync(long runId, CancellationToken cancellationToken = default);

    Task<WorkerDuplicateFileGroupPage> GetDuplicateFileGroupsAsync(
        DuplicateFileGroupQuery query,
        CancellationToken cancellationToken = default);

    Task<WorkerDuplicateFileSelectedRootFacetPage> GetDuplicateFileSelectedRootFacetsAsync(
        DuplicateFileSelectedRootFacetQuery query,
        CancellationToken cancellationToken = default);

    Task<WorkerDuplicateFileDriveFacetPage> GetDuplicateFileDriveFacetsAsync(
        DuplicateFileDriveFacetQuery query,
        CancellationToken cancellationToken = default);

    Task<WorkerDuplicateFileMemberPage> GetDuplicateFileGroupMembersAsync(
        DuplicateFileMemberQuery query,
        CancellationToken cancellationToken = default);

    Task<WorkerReviewPlanView> GetReviewPlanAsync(
        long runId,
        CancellationToken cancellationToken = default);

    Task<WorkerReviewGroupPage> GetReviewGroupsAsync(
        long runId,
        int pageSize,
        string? cursor = null,
        CancellationToken cancellationToken = default);

    Task<WorkerReviewDecisionMutation> SetReviewDecisionAsync(
        string operationId,
        long runId,
        long groupId,
        long fileId,
        string decision,
        long expectedRevision,
        CancellationToken cancellationToken = default);

    Task<WorkerReviewFolderGroupPage> GetReviewFolderGroupsAsync(
        long runId,
        int pageSize,
        string? cursor = null,
        CancellationToken cancellationToken = default);

    Task<WorkerReviewFolderDecisionMutation> SetReviewFolderDecisionAsync(
        string operationId,
        long runId,
        long folderGroupId,
        long folderMemberId,
        string decision,
        long expectedRevision,
        CancellationToken cancellationToken = default);

    Task<WorkerDuplicateFolderGroupPage> GetDuplicateFolderGroupsAsync(
        DuplicateFolderGroupQuery query,
        CancellationToken cancellationToken = default);

    Task<WorkerDuplicateFolderMemberPage> GetDuplicateFolderGroupMembersAsync(
        DuplicateFolderMemberQuery query,
        CancellationToken cancellationToken = default);
}

public interface IRestartableWorkerClient : IWorkerClient
{
    event EventHandler<WorkerUnexpectedExitEventArgs>? UnexpectedExit;

    Task<WorkerHelloResult> RestartAsync(CancellationToken cancellationToken = default);
}
