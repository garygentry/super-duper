using SuperDuper.Models;
using System.Text.RegularExpressions;

namespace SuperDuper.Services;

public record SuggestionResult(
    long? SuggestedDeleteFileId,
    long? SuggestedKeepFileId,
    string Reason,
    string HeuristicLabel
);

/// <summary>
/// Heuristic engine that suggests which copy of a duplicate to keep or delete.
/// Rules are evaluated in priority order — first match wins.
/// </summary>
public partial class SuggestionEngine
{
    [GeneratedRegex(@"\\d{4}\\", RegexOptions.IgnoreCase)]
    private static partial Regex YearSegmentRegex();

    [GeneratedRegex(@"(backup|oldlaptop|old_laptop|old-laptop|archive)", RegexOptions.IgnoreCase)]
    private static partial Regex BackupPathRegex();

    [GeneratedRegex(@"(\\Users\\[^\\]+\\(Documents|Desktop|Projects|Source|dev|code)\\)", RegexOptions.IgnoreCase)]
    private static partial Regex ActiveWorkspaceRegex();

    public SuggestionResult? Suggest(IReadOnlyList<DbFileInfo> copies)
    {
        if (copies.Count < 2) return null;

        // Rule 1: Never suggest if only one copy
        if (copies.Count == 1) return null;

        // Rule 2: Backup root heuristic
        var backupCopies = copies.Where(f => IsBackupPath(f.CanonicalPath)).ToList();
        var nonBackupCopies = copies.Except(backupCopies).ToList();
        if (backupCopies.Count > 0 && nonBackupCopies.Count > 0)
        {
            return new SuggestionResult(
                backupCopies.First().FileId,
                nonBackupCopies.First().FileId,
                "One copy is in a backup location",
                "Backup root"
            );
        }

        // Rule 3: Active workspace
        var activeCopies = copies.Where(f => ActiveWorkspaceRegex().IsMatch(f.CanonicalPath)).ToList();
        if (activeCopies.Count > 0 && activeCopies.Count < copies.Count)
        {
            var deleteCandidate = copies.Except(activeCopies).First();
            return new SuggestionResult(
                deleteCandidate.FileId,
                activeCopies.First().FileId,
                "One copy is in an active workspace",
                "Active workspace"
            );
        }

        // Rule 5: Newest copy (modified >7 days apart)
        var ordered = copies.OrderByDescending(f => ParseDate(f.LastModified)).ToList();
        if (ordered.Count >= 2)
        {
            var newest = ordered.First();
            var oldest = ordered.Last();
            var newestDate = ParseDate(newest.LastModified);
            var oldestDate = ParseDate(oldest.LastModified);
            if (newestDate.HasValue && oldestDate.HasValue &&
                (newestDate.Value - oldestDate.Value).TotalDays > 7)
            {
                return new SuggestionResult(
                    oldest.FileId,
                    newest.FileId,
                    "Keeping the more recently modified copy",
                    "Most recent"
                );
            }
        }

        // Rule 6: Shortest path (fewest segments = likely more canonical location)
        var shortestPath = copies.OrderBy(f => f.CanonicalPath.Count(c => c == '\\' || c == '/')).First();
        var longestPath = copies.OrderByDescending(f => f.CanonicalPath.Count(c => c == '\\' || c == '/')).First();
        if (shortestPath.FileId != longestPath.FileId)
        {
            return new SuggestionResult(
                longestPath.FileId,
                shortestPath.FileId,
                "Keeping the copy with the shorter (more canonical) path",
                "Shortest path"
            );
        }

        return null;
    }

    public string GetHeuristicLabel(string canonicalPath)
    {
        if (IsBackupPath(canonicalPath)) return "Backup root";
        if (ActiveWorkspaceRegex().IsMatch(canonicalPath)) return "Active workspace";
        if (YearSegmentRegex().IsMatch(canonicalPath)) return "Archive";
        return "";
    }

    private static bool IsBackupPath(string path) =>
        BackupPathRegex().IsMatch(path) || YearSegmentRegex().IsMatch(path);

    private static DateTime? ParseDate(string? dateStr)
    {
        if (string.IsNullOrEmpty(dateStr)) return null;
        if (DateTime.TryParse(dateStr, out var dt)) return dt;
        if (long.TryParse(dateStr, out var unix))
            return DateTimeOffset.FromUnixTimeSeconds(unix).DateTime;
        return null;
    }
}
