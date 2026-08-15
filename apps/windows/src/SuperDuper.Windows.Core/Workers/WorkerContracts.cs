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
