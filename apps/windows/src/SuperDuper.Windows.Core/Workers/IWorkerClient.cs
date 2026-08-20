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

    Task<WorkerPreflight?> GetLatestPreflightAsync(
        long runId,
        CancellationToken cancellationToken = default);

    Task<WorkerPreflight> GetPreflightAsync(
        long preflightId,
        CancellationToken cancellationToken = default);

    Task<WorkerPreflightStartResult> StartPreflightAsync(
        string operationId,
        long runId,
        long expectedReviewRevision,
        CancellationToken cancellationToken = default);

    Task<WorkerPreflightItemPage> GetPreflightItemsAsync(
        PreflightItemQuery query,
        CancellationToken cancellationToken = default);

    Task<WorkerPreflight> CancelPreflightAsync(
        long preflightId,
        CancellationToken cancellationToken = default);

    Task<WorkerPreferenceRulePage> ListPreferenceRulesAsync(
        long offset = 0,
        int limit = 200,
        CancellationToken cancellationToken = default);

    Task<WorkerPreferenceRule> GetPreferenceRuleAsync(
        long ruleId,
        CancellationToken cancellationToken = default);

    Task<WorkerPreferenceRuleSaveResult> SavePreferenceRuleAsync(
        string operationId,
        long? ruleId,
        string name,
        IReadOnlyList<string> roots,
        long expectedRevision,
        CancellationToken cancellationToken = default);

    Task<WorkerPreferencePreviewPage> GetPreferencePreviewAsync(
        PreferencePreviewQuery query,
        CancellationToken cancellationToken = default);

    Task<WorkerPreferenceApplicationResult> ApplyPreferenceRuleAsync(
        string operationId,
        long runId,
        long ruleId,
        long ruleRevision,
        long sourceReviewRevision,
        string previewSignature,
        PreferencePreviewScope scope,
        CancellationToken cancellationToken = default);

    Task<WorkerPreferenceApplicationPage> GetPreferenceApplicationsAsync(
        long runId,
        long? ruleId,
        string state,
        int pageSize,
        string? cursor = null,
        CancellationToken cancellationToken = default);

    Task<WorkerPreferenceApplication> GetPreferenceApplicationAsync(
        long runId,
        long applicationId,
        CancellationToken cancellationToken = default);

    Task<WorkerPreferenceReversalResult> ReversePreferenceApplicationAsync(
        string operationId,
        long runId,
        long applicationId,
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

public interface IRecycleOperationWorkerClient
{
    Task<WorkerRecycleOperationResult> PrepareRecycleOperationAsync(
        string operationId,
        long runId,
        long preflightId,
        long expectedReviewRevision,
        CancellationToken cancellationToken = default);

    Task<WorkerRecycleOperation?> GetLatestRecycleOperationAsync(
        long runId,
        CancellationToken cancellationToken = default);

    Task<WorkerRecycleOperation> GetRecycleOperationAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default);

    Task<WorkerRecycleOperationItemPage> GetRecycleOperationItemsAsync(
        RecycleOperationItemQuery query,
        CancellationToken cancellationToken = default);

    Task<WorkerRecycleOperationResult> ReportRecycleEligibilityAsync(
        string reportOperationId,
        long recycleOperationId,
        IReadOnlyList<RecycleEligibilityObservation> items,
        CancellationToken cancellationToken = default);

    Task<WorkerRecycleOperationResult> ConfirmRecycleOperationAsync(
        string reportOperationId,
        long recycleOperationId,
        string confirmationSignature,
        CancellationToken cancellationToken = default);

    Task<WorkerRecycleOperation> CancelRecycleOperationAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default);

    Task<WorkerRecycleOperationBatchResult> GetNextRecycleOperationBatchAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default);

    Task<WorkerRecycleOperationResult> BeginRecycleOperationBatchAsync(
        string reportOperationId,
        long recycleOperationId,
        long batchId,
        string shellAttemptId,
        CancellationToken cancellationToken = default);

    Task<WorkerRecycleOperationResult> ReportRecycleOperationBatchAsync(
        string reportOperationId,
        long recycleOperationId,
        long batchId,
        IReadOnlyList<RecycleItemResultObservation> items,
        CancellationToken cancellationToken = default);
}
