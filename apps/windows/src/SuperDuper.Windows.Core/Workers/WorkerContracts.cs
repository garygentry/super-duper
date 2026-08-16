namespace SuperDuper.Windows.Core.Workers;

public sealed record WorkerSessionDefinition(
    long Id,
    string Name,
    IReadOnlyList<string> Roots,
    IReadOnlyList<string> IgnorePatterns,
    DateTimeOffset CreatedAt,
    DateTimeOffset UpdatedAt);

public sealed record WorkerRunParameters(
    IReadOnlyList<string> Roots,
    IReadOnlyList<string> IgnorePatterns,
    ushort DirectorySimilarityThresholdMillis);

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
    string? ErrorMessage,
    string EngineVersion);

public sealed record WorkerSessionPage(IReadOnlyList<WorkerSessionDefinition> Sessions, long Total);

public sealed record WorkerRunPage(IReadOnlyList<WorkerRun> Runs, long Total);

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

public sealed record DuplicateFileGroupFilter(string Search, string MinimumSize);

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
    string RepresentativeType);

public sealed record WorkerDuplicateFileGroupPage(
    IReadOnlyList<WorkerDuplicateFileGroup> Groups,
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
    string ModifiedTimeUnixNanos);

public sealed record WorkerDuplicateFileMemberPage(
    IReadOnlyList<WorkerDuplicateFileMember> Members,
    long Total,
    string? NextCursor,
    string? PreviousCursor);

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

public sealed record WorkerDuplicateFolderMember(long Id, long GroupId, string Path);

public sealed record WorkerDuplicateFolderMemberPage(
    IReadOnlyList<WorkerDuplicateFolderMember> Members,
    long Total,
    string? NextCursor,
    string? PreviousCursor);

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
