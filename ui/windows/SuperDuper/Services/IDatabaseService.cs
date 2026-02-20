using SuperDuper.Models;

namespace SuperDuper.Services;

public record FileForDeletion(
    long FileId,
    string CanonicalPath,
    string FileName,
    string ParentDir,
    string DriveLetter,
    long FileSize,
    long ContentHash,
    string? RetainedCopyPath
);

public record SiblingInfo(
    int TotalSiblings,
    int DuplicatedSiblings
);

public record PagedResult<T>(
    IReadOnlyList<T> Items,
    int TotalCount
);

public record GroupFilterOptions(
    string? TextSearch,
    string? FileTypeFilter,
    string? DriveFilter,
    ReviewStatus? ReviewStatusFilter,
    long? MinWastedBytes,
    long? MaxWastedBytes
);

public interface IDatabaseService
{
    Task EnsureSchemaAsync();

    // Review decisions
    Task UpsertDecisionAsync(long fileId, long groupId, ReviewAction action, long? sessionId = null);
    Task<ReviewAction?> GetDecisionAsync(long fileId);
    Task<ReviewStatus> GetGroupReviewStatusAsync(long groupId);
    Task<(int Reviewed, int Total)> GetReviewProgressAsync(long sessionId);

    // Deletion queue
    Task<IReadOnlyList<FileForDeletion>> GetDeletionQueueAsync();
    Task<int> GetDeletionQueueCountAsync();

    // File queries
    Task<PagedResult<DbFileInfo>> QueryFilesInDirectoryAsync(
        string dirPath, long sessionId, int offset, int limit,
        string sortColumn = "file_name", bool ascending = true);
    Task<(int DupeCount, int TotalCount)> GetDirectoryDensityAsync(string dirPath, long sessionId);
    Task<SiblingInfo> GetSiblingContextAsync(long fileId);

    // Quick wins
    Task<IReadOnlyList<QuickWinItem>> GetQuickWinsAsync(long sessionId);

    // Group queries
    Task<PagedResult<DbGroupInfo>> QueryGroupsFilteredAsync(
        long sessionId, GroupFilterOptions filter, string sortColumn = "wasted_bytes",
        bool ascending = false, int offset = 0, int limit = 50);

    // Scan profiles
    Task<IReadOnlyList<ScanProfile>> GetSavedProfilesAsync();
    Task UpsertProfileAsync(ScanProfile profile);
    Task DeleteProfileAsync(string profileId);
}
