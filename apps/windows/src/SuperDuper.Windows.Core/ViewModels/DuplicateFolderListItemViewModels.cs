using System.Globalization;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class DuplicateFolderGroupListItemViewModel(WorkerDuplicateFolderGroup group)
{
    public WorkerDuplicateFolderGroup Group { get; } = group;

    public long Id => Group.Id;

    public string RepresentativePath => Group.RepresentativePath;

    public string TotalBytes => DisplayFormatting.Bytes(Group.TotalBytes);

    public string DescendantFileCount => Group.DescendantFileCount.ToString("N0");

    public string CopyCount => Group.CopyCount.ToString("N0");

    public string RecoverableBytes
    {
        get
        {
            if (!decimal.TryParse(
                    Group.TotalBytes,
                    NumberStyles.None,
                    CultureInfo.InvariantCulture,
                    out var bytesPerCopy))
            {
                return Group.TotalBytes;
            }

            var recoverableBytes = bytesPerCopy * Math.Max(0, Group.CopyCount - 1);
            return DisplayFormatting.Bytes(recoverableBytes.ToString(CultureInfo.InvariantCulture));
        }
    }

    public string RelationshipSummary =>
        $"{Group.CopyCount:N0} folder copies · {Group.DescendantFileCount:N0} files per copy · "
        + $"{TotalBytes} per copy · {RecoverableBytes} recoverable";
}

public sealed class DuplicateFolderMemberListItemViewModel
{
    internal const int MaximumDisplayedPathSegments = 7;

    private DuplicateFolderMemberListItemViewModel(
        WorkerDuplicateFolderMember member,
        IReadOnlyList<string> segments,
        int commonPrefixLength,
        int commonSuffixLength)
    {
        Member = member;
        var differingLength = Math.Max(0, segments.Count - commonPrefixLength - commonSuffixLength);
        var prefix = segments.Take(commonPrefixLength).ToArray();
        var differing = segments.Skip(commonPrefixLength).Take(differingLength).ToArray();
        var suffix = segments.Skip(commonPrefixLength + differingLength).ToArray();

        FolderName = segments.LastOrDefault() ?? member.Path;
        ParentLocation = Compact(segments.Take(Math.Max(0, segments.Count - 1)));
        SharedPathContext = FormatSharedContext(prefix, suffix);
        DifferingPathSegments = differing.Length == 0 ? "No differing segments" : Compact(differing);
        AutomationId = $"FolderLocationCard-{member.Id.ToString(CultureInfo.InvariantCulture)}";
        AutomationName = $"Folder copy {FolderName}; location {ParentLocation}; "
            + $"different path segments {DifferingPathSegments}; decision {Decision}";
    }

    public WorkerDuplicateFolderMember Member { get; }

    public long Id => Member.Id;

    public string Path => Member.Path;

    public string FolderName { get; }

    public string ParentLocation { get; }

    public string SharedPathContext { get; }

    public string DifferingPathSegments { get; }

    public string AutomationId { get; }

    public string AutomationName { get; }

    public string LocationLabel => $"{FolderName} at {ParentLocation}";

    public string KeepAutomationName => $"Keep folder copy {LocationLabel}";

    public string RemoveAutomationName => $"Remove folder copy {LocationLabel}";

    public string UndecideAutomationName => $"Clear folder decision for {LocationLabel}";

    public string CopyPathAutomationName => $"Copy full path for folder copy {LocationLabel}";

    public string FullPathAutomationName => $"Full immutable path for folder copy {LocationLabel}";

    public string RevealAutomationName => $"Show folder copy {LocationLabel} in Explorer";

    public string Decision => Member.Decision switch
    {
        "keep" => "Keep",
        "remove" => "Remove",
        _ => "Undecided",
    };

    public static IReadOnlyList<DuplicateFolderMemberListItemViewModel> CreatePage(
        IEnumerable<WorkerDuplicateFolderMember> members,
        int maximumItems)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(maximumItems);
        var page = members.Take(maximumItems).ToArray();
        if (page.Length == 0)
        {
            return [];
        }

        var tokenized = page.Select(member => Tokenize(member.Path)).ToArray();
        var commonPrefixLength = CommonPrefixLength(tokenized);
        var commonSuffixLength = CommonSuffixLength(tokenized, commonPrefixLength);
        return page.Select((member, index) => new DuplicateFolderMemberListItemViewModel(
            member,
            tokenized[index],
            commonPrefixLength,
            commonSuffixLength)).ToArray();
    }

    private static IReadOnlyList<string> Tokenize(string path)
    {
        var normalized = path.Replace('/', '\\').TrimEnd('\\');
        if (normalized.StartsWith("\\\\", StringComparison.Ordinal))
        {
            var uncSegments = normalized[2..].Split('\\', StringSplitOptions.RemoveEmptyEntries);
            if (uncSegments.Length >= 2)
            {
                return [$"\\\\{uncSegments[0]}\\{uncSegments[1]}", .. uncSegments.Skip(2)];
            }
        }

        if (normalized.Length >= 2 && normalized[1] == ':')
        {
            return [normalized[..2].ToUpperInvariant(), .. normalized[2..].Split('\\', StringSplitOptions.RemoveEmptyEntries)];
        }

        var segments = normalized.Split('\\', StringSplitOptions.RemoveEmptyEntries);
        return segments.Length == 0 ? [path] : segments;
    }

    private static int CommonPrefixLength(IReadOnlyList<string>[] paths)
    {
        var limit = paths.Min(path => path.Count);
        var length = 0;
        while (length < limit
               && paths.Skip(1).All(path => string.Equals(
                   paths[0][length],
                   path[length],
                   StringComparison.OrdinalIgnoreCase)))
        {
            length++;
        }
        return length;
    }

    private static int CommonSuffixLength(IReadOnlyList<string>[] paths, int commonPrefixLength)
    {
        var limit = paths.Min(path => path.Count) - commonPrefixLength;
        var length = 0;
        while (length < limit
               && paths.Skip(1).All(path => string.Equals(
                   paths[0][paths[0].Count - 1 - length],
                   path[path.Count - 1 - length],
                   StringComparison.OrdinalIgnoreCase)))
        {
            length++;
        }
        return length;
    }

    private static string FormatSharedContext(
        IReadOnlyList<string> prefix,
        IReadOnlyList<string> suffix)
    {
        if (prefix.Count == 0 && suffix.Count == 0)
        {
            return "No shared path segments on this page";
        }
        if (prefix.Count == 0)
        {
            return Compact(suffix);
        }
        if (suffix.Count == 0)
        {
            return Compact(prefix);
        }
        return $"{Compact(prefix)} · … · {Compact(suffix)}";
    }

    private static string Compact(IEnumerable<string> source)
    {
        var segments = source.ToArray();
        if (segments.Length == 0)
        {
            return "Top-level location";
        }
        if (segments.Length <= MaximumDisplayedPathSegments)
        {
            return string.Join(" › ", segments);
        }
        return string.Join(" › ", segments.Take(3))
            + " › … › "
            + string.Join(" › ", segments.TakeLast(3));
    }
}
