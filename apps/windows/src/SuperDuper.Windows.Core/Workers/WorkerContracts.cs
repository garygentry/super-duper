using System.Diagnostics.CodeAnalysis;
using System.Text.Json.Serialization;

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

public sealed record WorkerPerformanceRun(
    long Id,
    string OperationId,
    long? ProductRunId,
    uint MetricsContractVersion,
    string EngineVersion,
    string? WorkerVersion,
    string? AppVersion,
    long? ProductSchemaVersion,
    string InputSignature,
    string State,
    long? StartedUnixMillis,
    long? CompletedUnixMillis,
    ulong LastMonotonicNanos,
    ulong LastSequence,
    string? ErrorCode,
    string? ErrorMessage);

public sealed record WorkerPerformanceRunPage(
    IReadOnlyList<WorkerPerformanceRun> Runs,
    long? NextBeforeId,
    bool ExecutorEnabled);

public sealed record WorkerPerformanceCounter(string Metric, ulong Value, ulong UpdatedSequence);

public sealed record WorkerPerformancePhase(
    string Phase,
    string State,
    ulong? StartedMonotonicNanos,
    ulong? CompletedMonotonicNanos,
    ulong ActiveNanos);

public sealed record WorkerHostPerformanceSample(
    ulong Sequence,
    long ObservedUnixMillis,
    ulong MonotonicNanos,
    string? Phase,
    ulong? ProcessCpuNanos,
    ulong? ProcessPrivateBytes,
    ulong? ProcessWorkingSetBytes,
    ulong? ProcessPeakWorkingSetBytes,
    ulong? ProcessReadOperations,
    ulong? ProcessReadBytes,
    ulong? ProcessWriteOperations,
    ulong? ProcessWriteBytes,
    uint? SystemCpuBasisPoints,
    ulong? SystemAvailableMemoryBytes,
    ulong? SystemCommittedMemoryBytes,
    uint UnavailableCounterCount);

public sealed record WorkerHostPerformanceSummary(
    WorkerHostPerformanceSample? Latest,
    ulong? PeakProcessPrivateBytes,
    ulong? PeakProcessWorkingSetBytes,
    uint? PeakSystemCpuBasisPoints,
    ulong? MinimumSystemAvailableMemoryBytes);

public sealed record WorkerDeviceDescriptor(
    string DeviceKey,
    string VolumeKey,
    string? Filesystem,
    ulong? CapacityBytes,
    ulong? FreeBytesAtStart,
    string? BusType,
    string? MediaType,
    string? Model);

public sealed record WorkerDevicePerformanceSample(
    ulong Sequence,
    string DeviceKey,
    ulong? ReadBytesPerSecond,
    ulong? ReadIopsMillis,
    ulong? AverageReadLatencyMicros,
    uint? ActiveMillisPerSecond,
    ulong? QueueDepthMillis,
    uint UnavailableCounterCount);

public sealed record WorkerDevicePerformanceSummary(
    WorkerDeviceDescriptor Descriptor,
    WorkerDevicePerformanceSample? Latest,
    ulong? PeakReadBytesPerSecond,
    ulong? PeakReadIopsMillis,
    ulong? PeakAverageReadLatencyMicros,
    uint? PeakActiveMillisPerSecond,
    ulong? PeakQueueDepthMillis);

public sealed record WorkerPerformanceSnapshot(
    WorkerPerformanceRun Run,
    IReadOnlyList<WorkerPerformanceCounter> Counters,
    IReadOnlyList<WorkerPerformancePhase> Phases,
    WorkerHostPerformanceSummary Host,
    IReadOnlyList<WorkerDevicePerformanceSummary> Devices,
    bool ExecutorEnabled);

public sealed record WorkerRunExclusion(
    long Id,
    long RunId,
    string Path,
    string ReasonCode,
    string? ProviderId,
    string? ProviderName,
    long OccurrenceCount);

public sealed record WorkerRunExclusionPage(IReadOnlyList<WorkerRunExclusion> Exclusions, long Total);

public sealed record WorkerRunWarningAggregate(
    long Id,
    long RunId,
    string Phase,
    string Category,
    string Code,
    string Severity,
    string Message,
    long OccurrenceCount,
    IReadOnlyList<string> Examples);

public sealed record WorkerDiagnosticLogMetadata(
    string State,
    string? LocationKind,
    string? Path,
    string? Reason,
    string Relationship);

public sealed record WorkerRunWarningPage(
    IReadOnlyList<WorkerRunWarningAggregate> Warnings,
    long Total,
    long WarningCount,
    long AccountedWarningCount,
    long SnapshotRevision,
    string SnapshotState,
    string RunStatus,
    WorkerDiagnosticLogMetadata DiagnosticLog,
    string? NextCursor,
    bool ExecutorEnabled);

public enum RunWarningSortField
{
    Phase,
    OccurrenceCount,
    Message,
}

public sealed record RunWarningQuery(
    long RunId,
    int PageSize,
    RunWarningSortField SortField,
    WorkerSortDirection SortDirection,
    string? Cursor = null);

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

public sealed record WorkerReviewLiveHintItem(long FileId, long GroupId, string Path);

public sealed class WorkerResultStateChangedEventArgs : EventArgs
{
    public required string Kind { get; init; }

    public required long RunId { get; init; }

    public required string RootPath { get; init; }

    public required long EventCount { get; init; }

    public required long CoalescedPathCount { get; init; }

    public IReadOnlyList<WorkerReviewLiveHintItem> Items { get; init; } = [];

    public WorkerReviewLiveRootState? Root { get; init; }

    public required bool ExecutorEnabled { get; init; }
}

public sealed record WorkerReviewLiveRootState(
    long RunId,
    string RootPath,
    string State,
    long DirtyRevision,
    string ReasonCode,
    string DirtyAt,
    long? ReconciliationCursorFileId,
    long ReconciledItemCount,
    string UpdatedAt,
    bool ReconciliationRequired);

public sealed record WorkerReviewLiveRootPage(
    long RunId,
    IReadOnlyList<WorkerReviewLiveRootState> Roots,
    long Total,
    bool ExecutorEnabled);

public sealed record ReviewLiveRootReconciliationRequest(
    string OperationId,
    long RunId,
    string RootPath,
    long ExpectedDirtyRevision,
    long ExpectedReviewRevision,
    int PageSize);

public sealed record WorkerReviewLiveRootReconciliationResult(
    long ReconciliationId,
    long RunId,
    string RootPath,
    long DirtyRevision,
    long ReviewRevision,
    bool Replayed,
    WorkerReviewLiveValidationSummary Summary,
    IReadOnlyList<WorkerReviewLiveValidationItem> Items,
    WorkerReviewLiveRootState Root,
    bool ExecutorEnabled);

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

    public required WorkerScanProgressSnapshot Progress { get; init; }

    public string? CurrentPath { get; init; }

    public string? Message { get; init; }
}

public sealed class WorkerScanProgressSnapshot
{
    public required uint ProgressContractVersion { get; init; }

    public required uint MetricsContractVersion { get; init; }

    public required ulong Revision { get; init; }

    public required ulong MonotonicNanos { get; init; }

    public required string Phase { get; init; }

    public required ulong PhaseElapsedNanos { get; init; }

    public required WorkerScanProgressCounters Counters { get; init; }

    public required WorkerProgressLogicalCounters Logical { get; init; }

    public required WorkerCandidateFunnelProgress Funnel { get; init; }

    public required WorkerProgressRates PartialReadRates { get; init; }

    public required WorkerProgressRates FullReadRates { get; init; }

    public uint? CacheHitRateBasisPoints { get; init; }

    public required ulong WarningCount { get; init; }

    public required WorkerActiveDeviceProgress ActiveDevices { get; init; }

    public WorkerRemainingKnownWork? RemainingKnownWork { get; init; }

    public required WorkerProgressEta Eta { get; init; }
}

public sealed record WorkerScanProgressCounters
{
    [SetsRequiredMembers]
    public WorkerScanProgressCounters()
    {
        DiscoveredBytes = "0";
        HardLinkAliasBytes = "0";
        SingletonSizeBytes = "0";
        CandidateBytes = "0";
        DuplicateCandidateBytes = "0";
        MetadataResolvedBytes = "0";
        PartialHashBytesRead = "0";
        PartialCollisionBytes = "0";
        FullHashBytesRead = "0";
        RecoverableBytes = "0";
    }

    public required ulong DiscoveredFiles { get; init; }
    public required string DiscoveredBytes { get; init; }
    public required ulong ZeroByteFiles { get; init; }
    public required ulong HardLinkAliasFiles { get; init; }
    public required string HardLinkAliasBytes { get; init; }
    public required ulong SizeBuckets { get; init; }
    public required ulong SingletonSizeBuckets { get; init; }
    public required ulong SingletonSizeFiles { get; init; }
    public required string SingletonSizeBytes { get; init; }
    public required ulong CandidateSizeBuckets { get; init; }
    public required ulong CandidateFiles { get; init; }
    public required string CandidateBytes { get; init; }
    public required ulong DuplicateCandidateSizeBuckets { get; init; }
    public required ulong DuplicateCandidateFiles { get; init; }
    public required string DuplicateCandidateBytes { get; init; }
    public required ulong MetadataResolvedFiles { get; init; }
    public required string MetadataResolvedBytes { get; init; }
    public required ulong PartialHashesAttempted { get; init; }
    public required ulong PartialHashesSucceeded { get; init; }
    public required ulong PartialHashesFailed { get; init; }
    public required string PartialHashBytesRead { get; init; }
    public required ulong PartialHashCacheHits { get; init; }
    public required ulong PartialHashCacheMisses { get; init; }
    public required ulong PartialHashCacheErrors { get; init; }
    public required ulong PartialHashCacheStores { get; init; }
    public required ulong PartialCollisionBuckets { get; init; }
    public required ulong PartialCollisionFiles { get; init; }
    public required string PartialCollisionBytes { get; init; }
    public required ulong FullHashRequests { get; init; }
    public required ulong FullHashCacheHits { get; init; }
    public required ulong FullHashCacheMisses { get; init; }
    public required ulong FullHashCacheErrors { get; init; }
    public required ulong FullHashCacheStores { get; init; }
    public required ulong FullHashContentReadsStarted { get; init; }
    public required ulong FullHashContentReadsCompleted { get; init; }
    public required ulong FullHashContentReadsFailed { get; init; }
    public required string FullHashBytesRead { get; init; }
    public required ulong ConfirmedDuplicateGroups { get; init; }
    public required ulong ConfirmedLogicalCopies { get; init; }
    public required ulong ConfirmedPhysicalItems { get; init; }
    public required string RecoverableBytes { get; init; }
    public required ulong Warnings { get; init; }
    public required ulong CancelChecks { get; init; }
    public required ulong CancelledWorkItems { get; init; }
    public required ulong TelemetrySamplesLost { get; init; }
    public required ulong TelemetryFlushErrors { get; init; }
    public required ulong UnavailableCounters { get; init; }
}

public sealed record WorkerProgressLogicalCounters
{
    [SetsRequiredMembers]
    public WorkerProgressLogicalCounters()
    {
        PartialScreenedBytes = "0";
        FullHashRequestBytes = "0";
        FullHashSatisfiedBytes = "0";
        FullHashFailedBytes = "0";
        HashPipelineResolvedBytes = "0";
        ConfirmedLogicalBytes = "0";
    }

    public required ulong PartialScreenedFiles { get; init; }
    public required string PartialScreenedBytes { get; init; }
    public required string FullHashRequestBytes { get; init; }
    public required ulong FullHashSatisfiedFiles { get; init; }
    public required string FullHashSatisfiedBytes { get; init; }
    public required ulong FullHashFailedFiles { get; init; }
    public required string FullHashFailedBytes { get; init; }
    public required ulong HashPipelineResolvedFiles { get; init; }
    public required string HashPipelineResolvedBytes { get; init; }
    public required string ConfirmedLogicalBytes { get; init; }
}

public sealed class WorkerProgressQuantity
{
    public required ulong Files { get; init; }

    public required string LogicalBytes { get; init; }
}

public sealed class WorkerCandidateFunnelProgress
{
    public required WorkerProgressQuantity Discovered { get; init; }
    public required WorkerProgressQuantity MetadataResolved { get; init; }
    public required WorkerProgressQuantity HashPipelineCandidates { get; init; }
    public required WorkerProgressQuantity PartialScreened { get; init; }
    public required WorkerProgressQuantity SelectedForFullHash { get; init; }
    public required WorkerProgressQuantity FullHashSatisfied { get; init; }
    public required WorkerProgressQuantity FinalizedDuplicates { get; init; }
}

public sealed class WorkerProgressRate
{
    public required ulong FilesPerSecondMillis { get; init; }

    public required string PhysicalBytesPerSecond { get; init; }

    public required ulong WindowNanos { get; init; }
}

public sealed class WorkerProgressRateValue
{
    public required string State { get; init; }

    public WorkerProgressRate? Rate { get; init; }

    public string? Reason { get; init; }
}

public sealed class WorkerProgressRates
{
    public required WorkerProgressRateValue Cumulative { get; init; }

    public required WorkerProgressRateValue Recent { get; init; }
}

public sealed class WorkerActiveDeviceProgress
{
    public required string State { get; init; }

    public string? Reason { get; init; }

    [JsonPropertyName("device_key")]
    public string? DeviceKey { get; init; }

    [JsonPropertyName("device_keys")]
    public IReadOnlyList<string>? DeviceKeys { get; init; }
}

public sealed class WorkerRemainingKnownWork
{
    public required string Stage { get; init; }

    public required ulong Files { get; init; }

    public required string LogicalBytes { get; init; }
}

public sealed class WorkerProgressEta
{
    public required string State { get; init; }

    public string? Reason { get; init; }

    public string? Stage { get; init; }

    [JsonPropertyName("remaining_logical_bytes")]
    public string? RemainingLogicalBytes { get; init; }

    [JsonPropertyName("logical_bytes_per_second_millis")]
    public string? LogicalBytesPerSecondMillis { get; init; }

    [JsonPropertyName("estimated_seconds")]
    public ulong? EstimatedSeconds { get; init; }

    [JsonPropertyName("window_nanos")]
    public ulong? WindowNanos { get; init; }
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
