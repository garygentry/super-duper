namespace SuperDuper.Windows.Core.Workers;

public static class CloudPolicyNames
{
    public const string ExcludeRegisteredRoots = "exclude_registered_roots";
    public const string IncludeSyncRootsSkipPlaceholders = "include_sync_roots_skip_placeholders";
    public const string AllowCloudAccess = "allow_cloud_access";
}

public static class CloudDetectionStatusNames
{
    public const string Complete = "complete";
    public const string Unsupported = "unsupported";
    public const string Unavailable = "unavailable";
}

public sealed record WorkerRegisteredCloudLocation(string Path, string ProviderId, string DisplayName);

public sealed record WorkerSessionDefinition(
    long Id,
    string Name,
    IReadOnlyList<string> Roots,
    IReadOnlyList<string> IgnorePatterns,
    string CloudPolicy,
    IReadOnlyList<string> ManualLocationExclusions,
    IReadOnlyList<WorkerRegisteredCloudLocation> RegisteredCloudLocations,
    string CloudDetectionStatus,
    DateTimeOffset CreatedAt,
    DateTimeOffset UpdatedAt);

public sealed record WorkerRunParameters(
    IReadOnlyList<string> Roots,
    IReadOnlyList<string> IgnorePatterns,
    ushort DirectorySimilarityThresholdMillis,
    string CloudPolicy,
    IReadOnlyList<string> ManualLocationExclusions,
    IReadOnlyList<WorkerRegisteredCloudLocation> RegisteredCloudLocations,
    string CloudDetectionStatus);

public sealed record WorkerRun(
    long Id,
    long SessionId,
    WorkerRunParameters Parameters,
    string Status,
    string? Phase,
    DateTimeOffset CreatedAt,
    DateTimeOffset? StartedAt,
    DateTimeOffset? CompletedAt,
    long FilesDiscovered,
    string BytesDiscovered,
    long FilesHashed,
    long DuplicateFileGroups,
    long DuplicateFolderGroups,
    string WastedBytes,
    long WarningCount,
    long ExcludedSubtreeCount,
    string? ErrorMessage,
    string EngineVersion);

public sealed record WorkerSessionPage(IReadOnlyList<WorkerSessionDefinition> Sessions, long Total);

public sealed record WorkerRunPage(IReadOnlyList<WorkerRun> Runs, long Total);

public sealed record WorkerRunExclusion(
    long Id,
    long RunId,
    string Path,
    string ReasonCode,
    string? ProviderId,
    string? ProviderName,
    long OccurrenceCount);

public sealed record WorkerRunExclusionPage(IReadOnlyList<WorkerRunExclusion> Exclusions, long Total);

public enum WorkerSortDirection
{
    Ascending,
    Descending,
}

public enum DuplicateFileGroupSortField
{
    RecoverableBytes,
    GroupSize,
    CopyCount,
    RepresentativeName,
}

public enum DuplicateFileMemberSortField
{
    Path,
    ModifiedTime,
    Size,
}

public enum DuplicateFilePathMatchMode
{
    Substring,
    Exact,
}

public enum DuplicateFileExtensionMatchMode
{
    AnyMember,
    AllMembers,
}

public sealed record DuplicateFileGroupFilter(
    string Search,
    string MinimumSize,
    bool AcrossDrives = false,
    string? SelectedRoot = null,
    string? SelectedDrive = null,
    long MinimumCopyCount = 2,
    DuplicateFilePathMatchMode PathMatch = DuplicateFilePathMatchMode.Substring,
    string? Extension = null,
    DuplicateFileExtensionMatchMode ExtensionMatch = DuplicateFileExtensionMatchMode.AnyMember);

public sealed record DuplicateFileGroupQuery(
    long RunId,
    int PageSize,
    DuplicateFileGroupSortField SortField,
    WorkerSortDirection SortDirection,
    DuplicateFileGroupFilter Filter,
    string? Cursor = null);

public sealed record WorkerDuplicateFileGroup(
    long Id,
    long RunId,
    string GroupSize,
    long CopyCount,
    string RecoverableBytes,
    string RepresentativeName,
    string RepresentativeType)
{
    public long DistinctSelectedRootCount { get; init; }

    public long DistinctDriveCount { get; init; }
}

public sealed record WorkerDuplicateFileGroupPage(
    IReadOnlyList<WorkerDuplicateFileGroup> Groups,
    long Total,
    string? NextCursor,
    string? PreviousCursor)
{
    public WorkerDuplicateFileReviewSummary Summary { get; init; } =
        new(0, 0, "0", "0");
}

public sealed record WorkerDuplicateFileReviewSummary(
    long MatchingGroupCount,
    long MatchingCopyCount,
    string PotentialRecoverableBytes,
    string LargestRecoverableBytes)
{
    public long DistinctSelectedRootCount { get; init; }

    public long DistinctDriveCount { get; init; }

    public long AcrossDriveGroupCount { get; init; }
}

public enum DuplicateFileSelectedRootFacetSortField
{
    MatchingGroupCount,
    Value,
}

public sealed record DuplicateFileSelectedRootFacetFilter(
    string Search,
    string MinimumSize,
    bool AcrossDrives = false,
    string? SelectedDrive = null,
    long MinimumCopyCount = 2,
    DuplicateFilePathMatchMode PathMatch = DuplicateFilePathMatchMode.Substring,
    string? Extension = null,
    DuplicateFileExtensionMatchMode ExtensionMatch = DuplicateFileExtensionMatchMode.AnyMember);

public sealed record DuplicateFileSelectedRootFacetQuery(
    long RunId,
    int PageSize,
    DuplicateFileSelectedRootFacetSortField SortField,
    WorkerSortDirection SortDirection,
    DuplicateFileSelectedRootFacetFilter Filter,
    string? Cursor = null);

public sealed record WorkerDuplicateFileSelectedRootFacet(
    string Value,
    long MatchingGroupCount);

public sealed record WorkerDuplicateFileSelectedRootFacetPage(
    IReadOnlyList<WorkerDuplicateFileSelectedRootFacet> Facets,
    long Total,
    string? NextCursor,
    string? PreviousCursor);

public enum DuplicateFileDriveFacetSortField
{
    MatchingGroupCount,
    Value,
}

public sealed record DuplicateFileDriveFacetFilter(
    string Search,
    string MinimumSize,
    bool AcrossDrives = false,
    string? SelectedRoot = null,
    long MinimumCopyCount = 2,
    DuplicateFilePathMatchMode PathMatch = DuplicateFilePathMatchMode.Substring,
    string? Extension = null,
    DuplicateFileExtensionMatchMode ExtensionMatch = DuplicateFileExtensionMatchMode.AnyMember);

public sealed record DuplicateFileDriveFacetQuery(
    long RunId,
    int PageSize,
    DuplicateFileDriveFacetSortField SortField,
    WorkerSortDirection SortDirection,
    DuplicateFileDriveFacetFilter Filter,
    string? Cursor = null);

public sealed record WorkerDuplicateFileDriveFacet(
    string Value,
    long MatchingGroupCount);

public sealed record WorkerDuplicateFileDriveFacetPage(
    IReadOnlyList<WorkerDuplicateFileDriveFacet> Facets,
    long Total,
    string? NextCursor,
    string? PreviousCursor);

public sealed record DuplicateFileMemberFilter(string Search);

public sealed record DuplicateFileMemberQuery(
    long RunId,
    long GroupId,
    int PageSize,
    DuplicateFileMemberSortField SortField,
    WorkerSortDirection SortDirection,
    DuplicateFileMemberFilter Filter,
    string? Cursor = null);

public sealed record WorkerDuplicateFileMember(
    long Id,
    long GroupId,
    string Path,
    string FileName,
    string ParentPath,
    string Size,
    string ModifiedTimeUnixNanos)
{
    public string RootPath { get; init; } = string.Empty;

    public string RelativePath { get; init; } = string.Empty;

    public string DriveLetter { get; init; } = string.Empty;

    public string Decision { get; init; } = "undecided";

    public string? DecisionProvenance { get; init; }

    public string? DecisionAt { get; init; }

    public long? DecisionApplicationId { get; init; }

    public string? ValidationState { get; init; }

    public string? ValidationReasonCode { get; init; }

    public string? ValidationObservedAt { get; init; }

    public string? InvalidatedDecision { get; init; }
}

public sealed record WorkerDuplicateFileMemberPage(
    IReadOnlyList<WorkerDuplicateFileMember> Members,
    long Total,
    string? NextCursor,
    string? PreviousCursor)
{
    public long? ReviewPlanId { get; init; }

    public long ReviewRevision { get; init; }

    public WorkerReviewGroupSummary ReviewSummary { get; init; } = new(0, 0, 0, 0, 0);
}

public sealed record WorkerReviewPlan(
    long? Id,
    long RunId,
    string State,
    long Revision,
    string? CreatedAt,
    string? UpdatedAt);

public sealed record WorkerReviewPlanSummary(
    long DecidedGroupCount,
    long KeepCount,
    long RemoveCount,
    long UndecidedCount,
    string PlannedRemovalBytes,
    long RemainingPhysicalCopyCount)
{
    public long DecidedFolderGroupCount { get; init; }
    public long FolderKeepCount { get; init; }
    public long FolderRemoveCount { get; init; }
    public long FolderUndecidedCount { get; init; }
    public long EffectiveRemovalFileCount { get; init; }
    public long PlannedRemovalPhysicalItemCount { get; init; }
    public long IntactFolderCopyCount { get; init; }
    public long RuleKeepCount { get; init; }
    public long RuleRemoveCount { get; init; }
    public long ActiveRuleApplicationCount { get; init; }
}

public sealed record WorkerReviewPlanView(
    WorkerReviewPlan Plan,
    WorkerReviewPlanSummary Summary);

public sealed record WorkerReviewGroupSummary(
    long GroupId,
    long KeepCount,
    long RemoveCount,
    long UndecidedCount,
    long RemainingPhysicalCopyCount);

public sealed record WorkerReviewGroupPage(
    IReadOnlyList<WorkerReviewGroupSummary> Groups,
    long Total,
    long? PlanId,
    long Revision,
    string? NextCursor);

public sealed record WorkerReviewDecisionMutation(
    long PlanId,
    long AppliedRevision,
    bool Replayed,
    string Decision);

public sealed record ReviewLiveValidationRequest(
    string OperationId,
    long RunId,
    long GroupId,
    long ExpectedReviewRevision,
    string Scope,
    IReadOnlyList<long> FileIds);

public sealed record WorkerReviewLiveValidationSummary(
    long ItemCount,
    long PresentCount,
    long ChangedCount,
    long MissingCount,
    long UnavailableCount,
    long InvalidatedDecisionCount);

public sealed record WorkerReviewLiveValidationItem(
    long FileId,
    string State,
    string ReasonCode,
    bool DecisionInvalidated,
    string? InvalidatedDecision,
    string ObservedAt);

public sealed record WorkerReviewLiveValidationResult(
    long ValidationId,
    long RunId,
    long GroupId,
    long ReviewRevision,
    string Scope,
    bool Replayed,
    WorkerReviewLiveValidationSummary Summary,
    IReadOnlyList<WorkerReviewLiveValidationItem> Items);

public sealed record WorkerReviewFolderGroupSummary(
    long FolderGroupId,
    long KeepCount,
    long RemoveCount,
    long UndecidedCount,
    long IntactCopyCount);

public sealed record WorkerReviewFolderGroupPage(
    IReadOnlyList<WorkerReviewFolderGroupSummary> Groups,
    long Total,
    long? PlanId,
    long Revision,
    string? NextCursor);

public sealed record WorkerReviewFolderDecisionMutation(
    long PlanId,
    long AppliedRevision,
    bool Replayed,
    string Decision);

public sealed record WorkerPreflight(
    long Id,
    string OperationId,
    long RunId,
    long PlanId,
    long ReviewRevision,
    string SnapshotSignature,
    string Status,
    long LogicalRemovalCount,
    long PhysicalRemovalCount,
    long FolderRemovalCount,
    long AffectedGroupCount,
    string PlannedRemovalBytes,
    long TotalItemCount,
    long ProcessedItemCount,
    long ReadyCount,
    long ChangedCount,
    long MissingCount,
    long UnavailableCount,
    long ConflictCount,
    string CreatedAt,
    string? StartedAt,
    string? CompletedAt,
    string? ErrorCode,
    string? ErrorDetail,
    long CurrentReviewRevision,
    bool IsCurrent);

public sealed record WorkerPreflightStartResult(
    WorkerPreflight Preflight,
    bool Replayed);

public sealed record PreflightItemQuery(
    long PreflightId,
    int PageSize,
    string? Outcome = null,
    string? Cursor = null);

public sealed record WorkerPreflightItem(
    long Id,
    long PreflightId,
    long Ordinal,
    string TargetKind,
    string TargetRole,
    long? GroupId,
    long? FolderGroupId,
    long? FolderMemberId,
    long? SnapshotFileId,
    long? SnapshotDirectoryId,
    string Path,
    string Outcome,
    string? ReasonCode,
    string? ObservedFileSize,
    long? ObservedLastModified,
    long? OsError,
    string? ObservedAt,
    long SourceCount);

public sealed record WorkerPreflightItemPage(
    IReadOnlyList<WorkerPreflightItem> Items,
    long Total,
    string? NextCursor);

public sealed record WorkerRecycleOperation(
    long Id,
    string OperationId,
    long RunId,
    long PlanId,
    long PreflightId,
    long ReviewRevision,
    string PreflightSnapshotSignature,
    string IntentSignature,
    long PolicyVersion,
    string Status,
    long LogicalRemovalCount,
    long ShellItemCount,
    long PhysicalItemCount,
    long FolderItemCount,
    long AffectedGroupCount,
    string PlannedRemovalBytes,
    long AffectedLocationCount,
    long ExclusionCount,
    long EligibleCount,
    long NonRecyclableCount,
    long PendingEligibilityCount,
    long RecycledCount,
    long FailedCount,
    long CancelledCount,
    long UnknownCount,
    long PendingResultCount,
    string PreparedAt,
    string? ConfirmationSignature,
    string? ConfirmationExpiresAt,
    string? SubmittedAt,
    string? CompletedAt,
    bool CancellationRequested,
    string? ErrorCode,
    string? ErrorDetail,
    long CurrentReviewRevision,
    bool IsCurrent);

public sealed record WorkerRecycleOperationResult(
    WorkerRecycleOperation Operation,
    bool Replayed,
    bool ExecutorEnabled);

public sealed record WorkerRecycleOperationItem(
    long Id,
    long RecycleOperationId,
    long BatchId,
    long Ordinal,
    long PreflightItemId,
    long? PreflightSourceId,
    string TargetKind,
    string Path,
    long? GroupId,
    long? FolderGroupId,
    long? FolderMemberId,
    long? SnapshotFileId,
    long? SnapshotDirectoryId,
    string PlannedBytes,
    string EligibilityStatus,
    string? EligibilityCode,
    string ResultStatus,
    string? ResultCode,
    long? ShellHresult,
    bool? RecycledItemPresent,
    string? ResultAt,
    string? SnapshotFileIdentity = null,
    string? SnapshotFileSize = null,
    long? SnapshotLastModified = null);

public sealed record WorkerRecycleOperationBatch(
    long Id,
    long RecycleOperationId,
    long Ordinal,
    string ItemSignature,
    string Status,
    string? AdmissionExpiresAt,
    string? ShellAttemptId,
    string? StartedAt,
    string? ReportedAt,
    IReadOnlyList<WorkerRecycleOperationItem> Items);

public sealed record WorkerRecycleOperationBatchResult(
    WorkerRecycleOperationBatch? Batch,
    bool ExecutorEnabled);

public sealed record RecycleItemResultObservation(
    long ItemId,
    string Status,
    string? ReasonCode,
    long? ShellHresult,
    bool? RecycledItemPresent);

public sealed record RecycleBatchExecutionResult(
    IReadOnlyList<RecycleItemResultObservation> Items,
    long PerformHresult,
    long? FinishHresult,
    bool AnyOperationsAborted,
    long? AbortQueryHresult,
    bool ShellStarted);

public sealed record RecycleOperationItemQuery(
    long RecycleOperationId,
    int PageSize,
    string? ResultStatus = null,
    string? Cursor = null);

public sealed record WorkerRecycleOperationItemPage(
    IReadOnlyList<WorkerRecycleOperationItem> Items,
    long Total,
    string? NextCursor);

public sealed record RecycleEligibilityObservation(
    long ItemId,
    string Status,
    string? ReasonCode);

public sealed record WorkerRecoveryReview(
    long RecycleOperationId,
    string State,
    long UnknownItemCount,
    long ObservedItemCount);

public sealed record WorkerRecoveryReviewResult(
    WorkerRecoveryReview Review,
    bool ExecutorEnabled);

public sealed record WorkerRecoveryReviewObservation(
    long Id,
    string RequestId,
    long RecycleOperationId,
    long ItemId,
    string Observation,
    string ObservedAt,
    string? Note,
    long EvidenceVersion,
    long? SupersedesObservationId,
    string? CorrectionReason,
    string CreatedAt,
    long? SupersededByObservationId,
    bool IsCurrent);

public sealed record RecoveryReviewObservationQuery(
    long RecycleOperationId,
    int PageSize,
    bool CurrentOnly,
    string? Cursor = null);

public sealed record WorkerRecoveryReviewObservationPage(
    IReadOnlyList<WorkerRecoveryReviewObservation> Observations,
    long Total,
    string? NextCursor,
    bool ExecutorEnabled);

public sealed record RecoveryReviewObservationRecord(
    string RequestId,
    long RecycleOperationId,
    long ItemId,
    string Observation,
    string ObservedAt,
    string? Note,
    long EvidenceVersion,
    long? SupersedesObservationId = null,
    string? CorrectionReason = null);

public sealed record WorkerRecoveryReviewMutationResult(
    WorkerRecoveryReview Review,
    WorkerRecoveryReviewObservation Observation,
    bool Replayed,
    bool ExecutorEnabled);

public sealed record WorkerPreferenceRuleSummary(
    long Id,
    string Name,
    string Kind,
    long Revision,
    long RootCount,
    string UpdatedAt);

public sealed record WorkerPreferenceRule(
    long Id,
    string Name,
    string Kind,
    string State,
    long Revision,
    IReadOnlyList<string> Roots,
    string CreatedAt,
    string UpdatedAt);

public sealed record WorkerPreferenceRulePage(
    IReadOnlyList<WorkerPreferenceRuleSummary> Rules,
    long Total);

public sealed record WorkerPreferenceRuleSaveResult(
    WorkerPreferenceRule Rule,
    bool Replayed);

public enum PreferencePreviewScopeKind
{
    SelectedSets,
    CurrentFilter,
    CompletedRun,
}

public sealed record PreferencePreviewScope(
    PreferencePreviewScopeKind Kind,
    IReadOnlyList<long>? GroupIds = null,
    DuplicateFileGroupFilter? Filter = null);

public sealed record PreferencePreviewQuery(
    long RunId,
    long RuleId,
    long RuleRevision,
    long ReviewRevision,
    int PageSize,
    PreferencePreviewScope Scope,
    string? Cursor = null);

public sealed record WorkerPreferencePreviewGroup(
    long GroupId,
    string Status,
    long? BestRank,
    string? PreferredRoot,
    long TiedPreferredPathCount,
    long ProposedKeepPathCount,
    long ProposedRemovePathCount,
    long ProposedRemovePhysicalItemCount,
    string ProposedRemoveBytes,
    long ManualKeepCount,
    long ManualRemoveCount,
    string ExplanationCode,
    long? ConflictFileId,
    long? ConflictFolderMemberId);

public sealed record WorkerPreferencePreviewSummary(
    long ScopedGroupCount,
    long ScopedLogicalPathCount,
    long ScopedPhysicalItemCount,
    string ScopedBytes,
    long AffectedGroupCount,
    long BlockedGroupCount,
    long ProposedKeepPathCount,
    long ProposedRemovePathCount,
    long ProposedRemovePhysicalItemCount,
    string ProposedRemoveBytes,
    long ManualKeepPathCount,
    long ManualRemovePathCount,
    long TiedGroupCount,
    long NoRankedRootGroupCount,
    long MissingRuleRootCount,
    long OverlapConflictCount,
    long FileSurvivorConflictCount,
    long FolderSurvivorConflictCount);

public sealed record WorkerPreferencePreviewPage(
    IReadOnlyList<WorkerPreferencePreviewGroup> Groups,
    long Total,
    string? NextCursor,
    long RuleId,
    long RuleRevision,
    long? ReviewPlanId,
    long ReviewRevision,
    WorkerPreferencePreviewSummary Summary)
{
    public string PreviewSignature { get; init; } = string.Empty;
}

public sealed record WorkerPreferenceApplicationSummary(
    long ScopedGroupCount,
    long ApplicableGroupCount,
    long BlockedGroupCount,
    long RuleKeepPathCount,
    long RuleRemovePathCount,
    long RuleRemovePhysicalItemCount,
    string RuleRemoveBytes);

public sealed record WorkerPreferenceApplication(
    long Id,
    long PlanId,
    long RunId,
    long RuleId,
    long RuleRevision,
    string RuleName,
    string RuleKind,
    IReadOnlyList<string>? RuleRoots,
    string ScopeKind,
    long SourceReviewRevision,
    long AppliedRevision,
    string State,
    string CreatedAt,
    string? ReversedAt,
    WorkerPreferenceApplicationSummary Summary);

public sealed record WorkerPreferenceApplicationResult(
    WorkerPreferenceApplication Application,
    bool Replayed);

public sealed record WorkerPreferenceApplicationPage(
    IReadOnlyList<WorkerPreferenceApplication> Applications,
    long Total,
    string? NextCursor,
    long? PlanId,
    long Revision);

public sealed record WorkerPreferenceReversalResult(
    long ApplicationId,
    long PlanId,
    long AppliedRevision,
    bool Replayed,
    string State,
    long RemovedRuleKeepCount,
    long RemovedRuleRemoveCount);

public enum DuplicateFolderGroupSortField
{
    TotalBytes,
    CopyCount,
    FileCount,
    RepresentativePath,
}

public enum DuplicateFolderMemberSortField
{
    Path,
}

public sealed record DuplicateFolderGroupFilter(string Search, string MinimumSize);

public sealed record DuplicateFolderGroupQuery(
    long RunId,
    int PageSize,
    DuplicateFolderGroupSortField SortField,
    WorkerSortDirection SortDirection,
    DuplicateFolderGroupFilter Filter,
    string? Cursor = null);

public sealed record WorkerDuplicateFolderGroup(
    long Id,
    long RunId,
    string TotalBytes,
    long DescendantFileCount,
    long CopyCount,
    string RepresentativePath);

public sealed record WorkerDuplicateFolderGroupPage(
    IReadOnlyList<WorkerDuplicateFolderGroup> Groups,
    long Total,
    string? NextCursor,
    string? PreviousCursor);

public sealed record DuplicateFolderMemberFilter(string Search);

public sealed record DuplicateFolderMemberQuery(
    long RunId,
    long GroupId,
    int PageSize,
    DuplicateFolderMemberSortField SortField,
    WorkerSortDirection SortDirection,
    DuplicateFolderMemberFilter Filter,
    string? Cursor = null);

public sealed record WorkerDuplicateFolderMember(long Id, long GroupId, string Path)
{
    public string Decision { get; init; } = "undecided";
    public string? DecisionProvenance { get; init; }
    public string? DecisionAt { get; init; }
}

public sealed record WorkerDuplicateFolderMemberPage(
    IReadOnlyList<WorkerDuplicateFolderMember> Members,
    long Total,
    string? NextCursor,
    string? PreviousCursor)
{
    public long? ReviewPlanId { get; init; }
    public long ReviewRevision { get; init; }
    public WorkerReviewFolderGroupSummary ReviewSummary { get; init; } = new(0, 0, 0, 0, 0);
}

public sealed class WorkerRunProgressEventArgs : EventArgs
{
    public required long RunId { get; init; }

    public required ulong Sequence { get; init; }

    public required string Status { get; init; }

    public required string Phase { get; init; }

    public required long FilesDiscovered { get; init; }

    public required string BytesDiscovered { get; init; }

    public required long FilesHashed { get; init; }

    public required long WarningCount { get; init; }

    public string? CurrentPath { get; init; }

    public string? Message { get; init; }
}

public sealed class WorkerRunLifecycleEventArgs : EventArgs
{
    public required string EventName { get; init; }

    public required WorkerRun Run { get; init; }
}

public sealed class WorkerUnexpectedExitEventArgs : EventArgs
{
    public required int ExitCode { get; init; }

    public required string Message { get; init; }

    public required string ExecutablePath { get; init; }

    public required string DiagnosticLogPath { get; init; }
}
