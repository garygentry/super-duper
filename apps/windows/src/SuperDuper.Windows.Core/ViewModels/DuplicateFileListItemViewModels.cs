using System.Globalization;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class DuplicateFileSelectedRootFacetListItemViewModel
{
    public DuplicateFileSelectedRootFacetListItemViewModel(
        WorkerDuplicateFileSelectedRootFacet? facet = null,
        string? selectedValue = null)
    {
        Facet = facet;
        Value = facet?.Value ?? selectedValue;
    }

    public WorkerDuplicateFileSelectedRootFacet? Facet { get; }

    public string? Value { get; }

    public string DisplayText => Facet is null
        ? Value is null
            ? "All selected roots"
            : $"{Value} (selected; outside this page)"
        : $"{Facet.Value} · {Facet.MatchingGroupCount:N0} {Pluralize(Facet.MatchingGroupCount, "set", "sets")}";

    private static string Pluralize(long value, string singular, string plural) =>
        value == 1 ? singular : plural;
}

public sealed class DuplicateFileDriveFacetListItemViewModel
{
    public DuplicateFileDriveFacetListItemViewModel(
        WorkerDuplicateFileDriveFacet? facet = null,
        string? selectedValue = null)
    {
        Facet = facet;
        Value = facet?.Value ?? selectedValue;
    }

    public WorkerDuplicateFileDriveFacet? Facet { get; }

    public string? Value { get; }

    public string DisplayText => Facet is null
        ? Value is null
            ? "All drives"
            : $"{Value} (selected; outside this page)"
        : $"{Facet.Value} · {Facet.MatchingGroupCount:N0} {Pluralize(Facet.MatchingGroupCount, "set", "sets")}";

    private static string Pluralize(long value, string singular, string plural) =>
        value == 1 ? singular : plural;
}

public sealed class DuplicateFileGroupListItemViewModel(WorkerDuplicateFileGroup group)
{
    public WorkerDuplicateFileGroup Group { get; } = group;

    public long Id => Group.Id;

    public string RepresentativeName => Group.RepresentativeName;

    public string RepresentativeType => Group.RepresentativeType;

    public string GroupSize => DisplayFormatting.Bytes(Group.GroupSize);

    public string CopyCount => Group.CopyCount.ToString("N0");

    public string RecoverableBytes => DisplayFormatting.Bytes(Group.RecoverableBytes);

    public string LocationSpan
    {
        get
        {
            var roots = Group.DistinctSelectedRootCount switch
            {
                0 => "Selected root unavailable",
                1 => "1 selected root",
                _ => $"{Group.DistinctSelectedRootCount:N0} selected roots",
            };
            var drives = Group.DistinctDriveCount switch
            {
                0 => "no drive label",
                1 => "1 drive",
                _ => $"across {Group.DistinctDriveCount:N0} drives",
            };
            return $"{roots} · {drives}";
        }
    }
}

public sealed class DuplicateFileMemberListItemViewModel
{
    public DuplicateFileMemberListItemViewModel(
        WorkerDuplicateFileMember member,
        IReadOnlyList<string>? runRoots = null)
    {
        Member = member;
        SelectedRootLabel = SelectedRootLabelFormatter.Format(member.RootPath, runRoots);
    }

    public WorkerDuplicateFileMember Member { get; }

    public long Id => Member.Id;

    public string Path => Member.Path;

    public string SelectedRoot => Member.RootPath;

    public string SelectedRootLabel { get; }

    public string RelativePath => Member.RelativePath;

    public string Drive => string.IsNullOrWhiteSpace(Member.DriveLetter) ? "Other" : Member.DriveLetter;

    public string Size => DisplayFormatting.Bytes(Member.Size);

    public string Decision => Member.Decision switch
    {
        "keep" => "Keep",
        "remove" => "Remove",
        _ => "Undecided",
    };

    public string LiveState => Member.ValidationState switch
    {
        "present" when Member.InvalidatedDecision is not null =>
            $"Present; {DecisionLabel(Member.InvalidatedDecision)} decision remains invalidated until reviewed again",
        "present" => "Present; matches scan metadata",
        "missing" => $"Missing{InvalidatedSuffix}",
        "changed" => $"Changed since scan{InvalidatedSuffix}",
        "unavailable" => "Unavailable; decision retained until validation can complete",
        "validation_pending" => "Validation pending after a coalesced filesystem hint",
        _ => "Not validated in this working view",
    };

    public string LiveStateAutomationName =>
        $"Working file state for {Path}: {LiveState}. Immutable scan result unchanged.";

    public bool CanRecordCurrentDecision =>
        Member.ValidationState is null or "present";

    public bool CanClearDecision => Member.Decision != "undecided" || Member.InvalidatedDecision is not null;

    private string InvalidatedSuffix => Member.InvalidatedDecision is { } decision
        ? $"; prior {DecisionLabel(decision)} decision invalidated"
        : string.Empty;

    private static string DecisionLabel(string decision) => decision switch
    {
        "keep" => "Keep",
        "remove" => "Remove",
        _ => "review",
    };

    public string Modified
    {
        get
        {
            if (!long.TryParse(
                    Member.ModifiedTimeUnixNanos,
                    NumberStyles.Integer,
                    CultureInfo.InvariantCulture,
                    out var nanoseconds))
            {
                return Member.ModifiedTimeUnixNanos;
            }
            try
            {
                return DateTimeOffset
                    .FromUnixTimeSeconds(nanoseconds / 1_000_000_000)
                    .ToLocalTime()
                    .ToString("g");
            }
            catch (ArgumentOutOfRangeException)
            {
                return "Unknown";
            }
        }
    }
}

internal static class SelectedRootLabelFormatter
{
    public static string Format(string rootPath, IReadOnlyList<string>? runRoots)
    {
        if (string.IsNullOrWhiteSpace(rootPath)) return "Selected root unavailable";
        var root = Normalize(rootPath);
        var choices = (runRoots ?? [])
            .Where(path => !string.IsNullOrWhiteSpace(path))
            .Select(Normalize)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();
        if (!choices.Contains(root, StringComparer.OrdinalIgnoreCase)) return rootPath;

        var parts = choices.Select(path => new RootParts(path)).ToArray();
        var selected = parts.First(part => string.Equals(part.Path, root, StringComparison.OrdinalIgnoreCase));
        var maximumLength = parts.Max(part => part.Segments.Length);
        for (var length = 1; length <= maximumLength; length++)
        {
            var label = selected.Suffix(length);
            if (parts.Where(part => !ReferenceEquals(part, selected))
                .All(part => !string.Equals(label, part.Suffix(length), StringComparison.OrdinalIgnoreCase)))
                return label;
        }
        return rootPath;
    }

    private static string Normalize(string path)
    {
        var normalized = path.Replace('/', '\\');
        if (normalized.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase))
            normalized = @"\\" + normalized[8..];
        else if (normalized.StartsWith(@"\\?\", StringComparison.OrdinalIgnoreCase))
            normalized = normalized[4..];
        return normalized.TrimEnd('\\');
    }

    private sealed class RootParts(string path)
    {
        public string Path { get; } = path;
        public string[] Segments { get; } = path.Split('\\', StringSplitOptions.RemoveEmptyEntries);

        public string Suffix(int length)
        {
            if (Segments.Length == 0) return Path;
            var start = Math.Max(0, Segments.Length - length);
            var label = string.Join('\\', Segments[start..]);
            if (start == 0 && Path.StartsWith(@"\\", StringComparison.Ordinal))
                return @"\\" + label;
            if (start == 0 && label.EndsWith(':'))
                return label + '\\';
            return label;
        }
    }
}
