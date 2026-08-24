using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

internal sealed class TestWorkerClient : IRestartableWorkerClient, IRecycleOperationWorkerClient,
    IReviewLiveStateWorkerClient
{
    private long _nextSessionId;
    private long _nextRunId;

    public event EventHandler<WorkerRunProgressEventArgs>? RunProgress;

    public event EventHandler<WorkerRunLifecycleEventArgs>? RunLifecycleChanged;

    public event EventHandler<WorkerUnexpectedExitEventArgs>? UnexpectedExit;

    public event EventHandler<WorkerResultStateChangedEventArgs>? ResultStateChanged;

    public string ExecutablePath => @"C:\test\super-duper-worker.exe";

    public string DiagnosticLogPath => @"C:\test\logs\worker.log";

    public List<WorkerSessionDefinition> Sessions { get; } = [];

    public List<WorkerRun> Runs { get; } = [];

    public Func<long, CancellationToken, Task<WorkerRun>>? CancelHandler { get; set; }

    public Func<DuplicateFileGroupQuery, CancellationToken, Task<WorkerDuplicateFileGroupPage>>? GroupPageHandler { get; set; }

    public Func<DuplicateFileSelectedRootFacetQuery, CancellationToken, Task<WorkerDuplicateFileSelectedRootFacetPage>>? RootFacetPageHandler { get; set; }

    public Func<DuplicateFileDriveFacetQuery, CancellationToken, Task<WorkerDuplicateFileDriveFacetPage>>? DriveFacetPageHandler { get; set; }

    public Func<DuplicateFileMemberQuery, CancellationToken, Task<WorkerDuplicateFileMemberPage>>? MemberPageHandler { get; set; }

    public Func<long, CancellationToken, Task<WorkerReviewPlanView>>? ReviewPlanHandler { get; set; }

    public Func<long, int, string?, CancellationToken, Task<WorkerReviewGroupPage>>? ReviewGroupPageHandler { get; set; }

    public Func<string, long, long, long, string, long, CancellationToken, Task<WorkerReviewDecisionMutation>>? ReviewDecisionHandler { get; set; }
    public Func<ReviewLiveValidationRequest, CancellationToken, Task<WorkerReviewLiveValidationResult>>? ReviewLiveValidationHandler { get; set; }

    public Func<long, CancellationToken, Task<WorkerReviewLiveRootPage>>? DirtyReviewRootsHandler { get; set; }

    public Func<ReviewLiveRootReconciliationRequest, CancellationToken, Task<WorkerReviewLiveRootReconciliationResult>>? DirtyRootReconciliationHandler { get; set; }

    public Func<long, int, string?, CancellationToken, Task<WorkerReviewFolderGroupPage>>? ReviewFolderGroupPageHandler { get; set; }

    public Func<string, long, long, long, string, long, CancellationToken, Task<WorkerReviewFolderDecisionMutation>>? ReviewFolderDecisionHandler { get; set; }

    public Func<long, CancellationToken, Task<WorkerPreflight?>>? LatestPreflightHandler { get; set; }

    public Func<long, CancellationToken, Task<WorkerPreflight>>? PreflightHandler { get; set; }

    public Func<string, long, long, CancellationToken, Task<WorkerPreflightStartResult>>? PreflightStartHandler { get; set; }

    public Func<PreflightItemQuery, CancellationToken, Task<WorkerPreflightItemPage>>? PreflightItemPageHandler { get; set; }

    public Func<long, CancellationToken, Task<WorkerPreflight>>? PreflightCancelHandler { get; set; }

    public Func<long, CancellationToken, Task<WorkerRecycleOperation?>>? LatestRecycleOperationHandler { get; set; }

    public Func<RecycleOperationItemQuery, CancellationToken, Task<WorkerRecycleOperationItemPage>>? RecycleOperationItemPageHandler { get; set; }

    public Func<long, CancellationToken, Task<WorkerRecoveryReviewResult>>? RecoveryReviewHandler { get; set; }

    public Func<RecoveryReviewObservationQuery, CancellationToken, Task<WorkerRecoveryReviewObservationPage>>? RecoveryReviewPageHandler { get; set; }

    public Func<RecoveryReviewObservationRecord, CancellationToken, Task<WorkerRecoveryReviewMutationResult>>? RecoveryReviewRecordHandler { get; set; }

    public Func<PreferencePreviewQuery, CancellationToken, Task<WorkerPreferencePreviewPage>>? PreferencePreviewHandler { get; set; }

    public Func<string, long, long, long, long, string, PreferencePreviewScope, CancellationToken, Task<WorkerPreferenceApplicationResult>>? PreferenceApplyHandler { get; set; }

    public Func<long, long?, string, int, string?, CancellationToken, Task<WorkerPreferenceApplicationPage>>? PreferenceApplicationPageHandler { get; set; }

    public Func<string, long, long, long, CancellationToken, Task<WorkerPreferenceReversalResult>>? PreferenceReverseHandler { get; set; }

    public List<WorkerPreferenceRule> PreferenceRules { get; } = [];

    public Func<DuplicateFolderGroupQuery, CancellationToken, Task<WorkerDuplicateFolderGroupPage>>? FolderGroupPageHandler { get; set; }

    public Func<DuplicateFolderMemberQuery, CancellationToken, Task<WorkerDuplicateFolderMemberPage>>? FolderMemberPageHandler { get; set; }

    public int RestartCount { get; private set; }

    public WorkerRun? ObservedLiveRun { get; private set; }

    public void ObserveReviewLiveState(WorkerRun? run) => ObservedLiveRun = run;

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

    public Func<long, int, string?, CancellationToken, Task<WorkerRunWarningPage>>? RunWarningsHandler { get; set; }

    public Task<WorkerRunWarningPage> GetRunWarningsAsync(
        long runId,
        int pageSize,
        string? cursor = null,
        CancellationToken cancellationToken = default) =>
        RunWarningsHandler?.Invoke(runId, pageSize, cursor, cancellationToken)
        ?? Task.FromResult(new WorkerRunWarningPage([], 0, 0, 0, null, false));

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

    public Task<WorkerDuplicateFileDriveFacetPage> GetDuplicateFileDriveFacetsAsync(
        DuplicateFileDriveFacetQuery query,
        CancellationToken cancellationToken = default) =>
        DriveFacetPageHandler?.Invoke(query, cancellationToken)
        ?? Task.FromResult(new WorkerDuplicateFileDriveFacetPage([], 0, null, null));

    public Task<WorkerReviewPlanView> GetReviewPlanAsync(
        long runId,
        CancellationToken cancellationToken = default) =>
        ReviewPlanHandler?.Invoke(runId, cancellationToken)
        ?? Task.FromResult(new WorkerReviewPlanView(
            new WorkerReviewPlan(null, runId, "notCreated", 0, null, null),
            new WorkerReviewPlanSummary(0, 0, 0, 0, "0", 0)));

    public Task<WorkerReviewGroupPage> GetReviewGroupsAsync(
        long runId,
        int pageSize,
        string? cursor = null,
        CancellationToken cancellationToken = default) =>
        ReviewGroupPageHandler?.Invoke(runId, pageSize, cursor, cancellationToken)
        ?? Task.FromResult(new WorkerReviewGroupPage([], 0, null, 0, null));

    public Task<WorkerReviewDecisionMutation> SetReviewDecisionAsync(
        string operationId,
        long runId,
        long groupId,
        long fileId,
        string decision,
        long expectedRevision,
        CancellationToken cancellationToken = default) =>
        ReviewDecisionHandler?.Invoke(
            operationId,
            runId,
            groupId,
            fileId,
            decision,
            expectedRevision,
            cancellationToken)
        ?? Task.FromResult(new WorkerReviewDecisionMutation(1, expectedRevision + 1, false, decision));

    public Task<WorkerReviewLiveValidationResult> ValidateReviewFilesAsync(
        ReviewLiveValidationRequest request,
        CancellationToken cancellationToken = default) =>
        ReviewLiveValidationHandler?.Invoke(request, cancellationToken)
        ?? Task.FromResult(new WorkerReviewLiveValidationResult(
            1,
            request.RunId,
            request.GroupId,
            request.ExpectedReviewRevision,
            request.Scope,
            false,
            new WorkerReviewLiveValidationSummary(request.FileIds.Count, request.FileIds.Count, 0, 0, 0, 0),
            request.FileIds.Select(id => new WorkerReviewLiveValidationItem(
                id, "present", "matched_snapshot", false, null, "2026-08-24T00:00:00Z")).ToArray()));

    public Task<WorkerReviewLiveRootPage> GetDirtyReviewRootsAsync(
        long runId,
        CancellationToken cancellationToken = default) =>
        DirtyReviewRootsHandler?.Invoke(runId, cancellationToken)
        ?? Task.FromResult(new WorkerReviewLiveRootPage(runId, [], 0, false));

    public Task<WorkerReviewLiveRootReconciliationResult> ReconcileDirtyReviewRootAsync(
        ReviewLiveRootReconciliationRequest request,
        CancellationToken cancellationToken = default) =>
        DirtyRootReconciliationHandler?.Invoke(request, cancellationToken)
        ?? Task.FromResult(new WorkerReviewLiveRootReconciliationResult(
            1,
            request.RunId,
            request.RootPath,
            request.ExpectedDirtyRevision,
            request.ExpectedReviewRevision,
            false,
            new WorkerReviewLiveValidationSummary(0, 0, 0, 0, 0, 0),
            [],
            new WorkerReviewLiveRootState(
                request.RunId,
                request.RootPath,
                "clean",
                request.ExpectedDirtyRevision,
                "watcher_overflow",
                "2026-08-24T00:00:00Z",
                null,
                0,
                "2026-08-24T00:00:00Z",
                false),
            false));

    public Task<WorkerReviewFolderGroupPage> GetReviewFolderGroupsAsync(
        long runId,
        int pageSize,
        string? cursor = null,
        CancellationToken cancellationToken = default) =>
        ReviewFolderGroupPageHandler?.Invoke(runId, pageSize, cursor, cancellationToken)
        ?? Task.FromResult(new WorkerReviewFolderGroupPage([], 0, null, 0, null));

    public Task<WorkerReviewFolderDecisionMutation> SetReviewFolderDecisionAsync(
        string operationId,
        long runId,
        long folderGroupId,
        long folderMemberId,
        string decision,
        long expectedRevision,
        CancellationToken cancellationToken = default) =>
        ReviewFolderDecisionHandler?.Invoke(
            operationId,
            runId,
            folderGroupId,
            folderMemberId,
            decision,
            expectedRevision,
            cancellationToken)
        ?? Task.FromResult(new WorkerReviewFolderDecisionMutation(1, expectedRevision + 1, false, decision));

    public Task<WorkerPreflight?> GetLatestPreflightAsync(
        long runId,
        CancellationToken cancellationToken = default) =>
        LatestPreflightHandler?.Invoke(runId, cancellationToken)
        ?? Task.FromResult<WorkerPreflight?>(null);

    public Task<WorkerPreflight> GetPreflightAsync(
        long preflightId,
        CancellationToken cancellationToken = default) =>
        PreflightHandler?.Invoke(preflightId, cancellationToken)
        ?? Task.FromResult(CreatePreflight(preflightId, 1, "completed", 1));

    public Task<WorkerPreflightStartResult> StartPreflightAsync(
        string operationId,
        long runId,
        long expectedReviewRevision,
        CancellationToken cancellationToken = default) =>
        PreflightStartHandler?.Invoke(operationId, runId, expectedReviewRevision, cancellationToken)
        ?? Task.FromResult(new WorkerPreflightStartResult(
            CreatePreflight(1, runId, "running", expectedReviewRevision), false));

    public Task<WorkerPreflightItemPage> GetPreflightItemsAsync(
        PreflightItemQuery query,
        CancellationToken cancellationToken = default) =>
        PreflightItemPageHandler?.Invoke(query, cancellationToken)
        ?? Task.FromResult(new WorkerPreflightItemPage([], 0, null));

    public Task<WorkerPreflight> CancelPreflightAsync(
        long preflightId,
        CancellationToken cancellationToken = default) =>
        PreflightCancelHandler?.Invoke(preflightId, cancellationToken)
        ?? Task.FromResult(CreatePreflight(preflightId, 1, "cancelling", 1));

    public Task<WorkerRecycleOperationResult> PrepareRecycleOperationAsync(
        string operationId,
        long runId,
        long preflightId,
        long expectedReviewRevision,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerRecycleOperationResult(
            CreateRecycleOperation(1, runId, preflightId, expectedReviewRevision), false, false));

    public Task<WorkerRecycleOperation?> GetLatestRecycleOperationAsync(
        long runId,
        CancellationToken cancellationToken = default) =>
        LatestRecycleOperationHandler?.Invoke(runId, cancellationToken)
        ?? Task.FromResult<WorkerRecycleOperation?>(null);

    public async Task<WorkerRecycleOperation> GetRecycleOperationAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default) =>
        await GetLatestRecycleOperationAsync(1, cancellationToken)
        ?? throw new InvalidOperationException("No test recycle operation was configured.");

    public Task<WorkerRecycleOperationItemPage> GetRecycleOperationItemsAsync(
        RecycleOperationItemQuery query,
        CancellationToken cancellationToken = default) =>
        RecycleOperationItemPageHandler?.Invoke(query, cancellationToken)
        ?? Task.FromResult(new WorkerRecycleOperationItemPage([], 0, null));

    public Task<WorkerRecycleOperationResult> ReportRecycleEligibilityAsync(
        string reportOperationId,
        long recycleOperationId,
        IReadOnlyList<RecycleEligibilityObservation> items,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerRecycleOperationResult(
            CreateRecycleOperation(recycleOperationId, 1, 1, 1) with { Status = "failed" },
            false,
            false));

    public Task<WorkerRecycleOperationResult> ConfirmRecycleOperationAsync(
        string reportOperationId,
        long recycleOperationId,
        string confirmationSignature,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerRecycleOperationResult(
            CreateRecycleOperation(recycleOperationId, 1, 1, 1) with { Status = "submitted" },
            false,
            false));

    public Task<WorkerRecycleOperation> CancelRecycleOperationAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(CreateRecycleOperation(recycleOperationId, 1, 1, 1) with { Status = "cancelled" });

    public Task<WorkerRecycleOperationBatchResult> GetNextRecycleOperationBatchAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerRecycleOperationBatchResult(null, false));

    public Task<WorkerRecycleOperationResult> BeginRecycleOperationBatchAsync(
        string reportOperationId,
        long recycleOperationId,
        long batchId,
        string shellAttemptId,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerRecycleOperationResult(
            CreateRecycleOperation(recycleOperationId, 1, 1, 1) with { Status = "executing" },
            false,
            false));

    public Task<WorkerRecycleOperationResult> ReportRecycleOperationBatchAsync(
        string reportOperationId,
        long recycleOperationId,
        long batchId,
        IReadOnlyList<RecycleItemResultObservation> items,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerRecycleOperationResult(
            CreateRecycleOperation(recycleOperationId, 1, 1, 1) with { Status = "completed" },
            false,
            false));

    public Task<WorkerRecoveryReviewResult> GetRecoveryReviewAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default) =>
        RecoveryReviewHandler?.Invoke(recycleOperationId, cancellationToken)
        ?? Task.FromResult(new WorkerRecoveryReviewResult(
            new WorkerRecoveryReview(recycleOperationId, "not_started", 0, 0),
            false));

    public Task<WorkerRecoveryReviewObservationPage> GetRecoveryReviewObservationsAsync(
        RecoveryReviewObservationQuery query,
        CancellationToken cancellationToken = default) =>
        RecoveryReviewPageHandler?.Invoke(query, cancellationToken)
        ?? Task.FromResult(new WorkerRecoveryReviewObservationPage([], 0, null, false));

    public Task<WorkerRecoveryReviewMutationResult> RecordRecoveryReviewObservationAsync(
        RecoveryReviewObservationRecord record,
        CancellationToken cancellationToken = default) =>
        RecoveryReviewRecordHandler?.Invoke(record, cancellationToken)
        ?? Task.FromResult(new WorkerRecoveryReviewMutationResult(
            new WorkerRecoveryReview(
                record.RecycleOperationId,
                "in_progress",
                1,
                1),
            new WorkerRecoveryReviewObservation(
                1,
                record.RequestId,
                record.RecycleOperationId,
                record.ItemId,
                record.Observation,
                record.ObservedAt,
                record.Note,
                record.EvidenceVersion,
                record.SupersedesObservationId,
                record.CorrectionReason,
                record.ObservedAt,
                null,
                true),
            false,
            false));

    public Task<WorkerPreferenceRulePage> ListPreferenceRulesAsync(
        long offset = 0,
        int limit = 200,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerPreferenceRulePage(
            PreferenceRules.Skip((int)offset).Take(limit).Select(rule => new WorkerPreferenceRuleSummary(
                rule.Id,
                rule.Name,
                rule.Kind,
                rule.Revision,
                rule.Roots.Count,
                rule.UpdatedAt)).ToArray(),
            PreferenceRules.Count));

    public Task<WorkerPreferenceRule> GetPreferenceRuleAsync(
        long ruleId,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(PreferenceRules.Single(rule => rule.Id == ruleId));

    public Task<WorkerPreferenceRuleSaveResult> SavePreferenceRuleAsync(
        string operationId,
        long? ruleId,
        string name,
        IReadOnlyList<string> roots,
        long expectedRevision,
        CancellationToken cancellationToken = default)
    {
        var now = DateTimeOffset.UtcNow.ToString("O");
        var id = ruleId ?? (PreferenceRules.Count == 0 ? 1 : PreferenceRules.Max(rule => rule.Id) + 1);
        var existing = PreferenceRules.FindIndex(rule => rule.Id == id);
        var saved = new WorkerPreferenceRule(
            id,
            name,
            "ordered_preferred_scan_roots",
            "active",
            expectedRevision + 1,
            roots.ToArray(),
            existing >= 0 ? PreferenceRules[existing].CreatedAt : now,
            now);
        if (existing >= 0)
        {
            PreferenceRules[existing] = saved;
        }
        else
        {
            PreferenceRules.Add(saved);
        }
        return Task.FromResult(new WorkerPreferenceRuleSaveResult(saved, false));
    }

    public Task<WorkerPreferencePreviewPage> GetPreferencePreviewAsync(
        PreferencePreviewQuery query,
        CancellationToken cancellationToken = default) =>
        PreferencePreviewHandler?.Invoke(query, cancellationToken)
        ?? Task.FromResult(new WorkerPreferencePreviewPage(
            [], 0, null, query.RuleId, query.RuleRevision, null, query.ReviewRevision,
            new WorkerPreferencePreviewSummary(0, 0, 0, "0", 0, 0, 0, 0, 0, "0", 0, 0, 0, 0, 0, 0, 0, 0)));

    public Task<WorkerPreferenceApplicationResult> ApplyPreferenceRuleAsync(
        string operationId,
        long runId,
        long ruleId,
        long ruleRevision,
        long sourceReviewRevision,
        string previewSignature,
        PreferencePreviewScope scope,
        CancellationToken cancellationToken = default) =>
        PreferenceApplyHandler?.Invoke(
            operationId, runId, ruleId, ruleRevision, sourceReviewRevision,
            previewSignature, scope, cancellationToken)
        ?? Task.FromResult(new WorkerPreferenceApplicationResult(
            new WorkerPreferenceApplication(
                1, 1, runId, ruleId, ruleRevision, "Rule", "ordered_preferred_scan_roots",
                [], "completed_run", sourceReviewRevision, sourceReviewRevision + 1, "active",
                DateTimeOffset.UtcNow.ToString("O"), null,
                new WorkerPreferenceApplicationSummary(0, 0, 0, 0, 0, 0, "0")),
            false));

    public Task<WorkerPreferenceApplicationPage> GetPreferenceApplicationsAsync(
        long runId,
        long? ruleId,
        string state,
        int pageSize,
        string? cursor = null,
        CancellationToken cancellationToken = default) =>
        PreferenceApplicationPageHandler?.Invoke(runId, ruleId, state, pageSize, cursor, cancellationToken)
        ?? Task.FromResult(new WorkerPreferenceApplicationPage([], 0, null, null, 0));

    public Task<WorkerPreferenceApplication> GetPreferenceApplicationAsync(
        long runId,
        long applicationId,
        CancellationToken cancellationToken = default) =>
        Task.FromResult(new WorkerPreferenceApplication(
            applicationId, 1, runId, 1, 1, "Rule", "ordered_preferred_scan_roots",
            [], "completed_run", 0, 1, "active", DateTimeOffset.UtcNow.ToString("O"), null,
            new WorkerPreferenceApplicationSummary(0, 0, 0, 0, 0, 0, "0")));

    public Task<WorkerPreferenceReversalResult> ReversePreferenceApplicationAsync(
        string operationId,
        long runId,
        long applicationId,
        long expectedRevision,
        CancellationToken cancellationToken = default) =>
        PreferenceReverseHandler?.Invoke(operationId, runId, applicationId, expectedRevision, cancellationToken)
        ?? Task.FromResult(new WorkerPreferenceReversalResult(
            applicationId, 1, expectedRevision + 1, false, "reversed", 0, 0));

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

    public void RaiseResultStateChanged(WorkerResultStateChangedEventArgs stateChanged) =>
        ResultStateChanged?.Invoke(this, stateChanged);

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

    public static WorkerPreflight CreatePreflight(
        long id,
        long runId,
        string status,
        long revision,
        long processed = 2,
        long total = 2,
        long ready = 2,
        long changed = 0,
        long missing = 0,
        long unavailable = 0,
        long conflict = 0) =>
        new(
            id, $"operation-{id}", runId, 1, revision, $"signature-{id}", status,
            1, 1, 0, 1, "100", total, processed, ready, changed, missing, unavailable,
            conflict, DateTimeOffset.UtcNow.ToString("O"), DateTimeOffset.UtcNow.ToString("O"),
            status is "completed" or "cancelled" or "failed" or "interrupted"
                ? DateTimeOffset.UtcNow.ToString("O") : null,
            null, null, revision, true);

    public static WorkerRecycleOperation CreateRecycleOperation(
        long id,
        long runId,
        long preflightId,
        long revision,
        string status = "prepared") =>
        new(
            id, $"recycle-operation-{id}", runId, 1, preflightId, revision,
            $"preflight-signature-{id}", $"intent-signature-{id}", 1, status,
            4, 3, 3, 0, 2, "4096", 2, 1, 0, 0, 3,
            0, 0, 0, 0, 3, DateTimeOffset.UtcNow.ToString("O"), null, null,
            null, null, false, null, null, revision, true);

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

    public Func<string, CancellationToken, Task>? Handler { get; set; }

    public Func<IReadOnlyList<string>, CancellationToken, Task<ExplorerSelectionResult>>? SelectionHandler { get; set; }

    public IReadOnlyList<string>? SelectedPaths { get; private set; }

    public int SelectionCallCount { get; private set; }

    public Task RevealAsync(string path, CancellationToken cancellationToken = default)
    {
        RevealedPath = path;
        if (Handler is not null)
        {
            return Handler(path, cancellationToken);
        }
        if (Error is not null)
        {
            return Task.FromException(Error);
        }
        return Task.CompletedTask;
    }

    public Task<ExplorerSelectionResult> SelectByParentAsync(
        IReadOnlyList<string> paths,
        CancellationToken cancellationToken = default)
    {
        SelectionCallCount++;
        SelectedPaths = paths.ToArray();
        if (SelectionHandler is not null)
        {
            return SelectionHandler(paths, cancellationToken);
        }

        var parentCount = paths
            .Select(path => Path.GetDirectoryName(path) ?? string.Empty)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .Count();
        return Task.FromResult(new ExplorerSelectionResult(
            paths.Count,
            parentCount,
            paths.Count,
            []));
    }
}
