using System.Collections.ObjectModel;

namespace SuperDuper.Windows.Core.Services;

/// <summary>Chooses the last result view the operator used.</summary>
public enum ResultsDisplayMode
{
    Files,
    Folders,
}

/// <summary>
/// Local, non-worker presentation choices. This deliberately excludes scan data, paths,
/// decisions, and transient status or error state.
/// </summary>
public sealed record PresentationPreferences
{
    public const int CurrentVersion = 1;

    private static readonly IReadOnlyDictionary<string, bool> EmptySectionExpansion =
        new ReadOnlyDictionary<string, bool>(new Dictionary<string, bool>());

    public static PresentationPreferences Default { get; } = new();

    public int Version { get; init; } = CurrentVersion;

    public bool IsSavedScanSelectorExpanded { get; init; }

    public ResultsDisplayMode LastResultsMode { get; init; } = ResultsDisplayMode.Files;

    public IReadOnlyDictionary<string, bool> SectionExpansion { get; init; } = EmptySectionExpansion;

    public bool IsSectionExpanded(string sectionId)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(sectionId);
        return SectionExpansion.TryGetValue(sectionId, out var isExpanded) && isExpanded;
    }

    public PresentationPreferences WithSectionExpanded(string sectionId, bool isExpanded)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(sectionId);
        var updated = new Dictionary<string, bool>(SectionExpansion, StringComparer.Ordinal)
        {
            [sectionId] = isExpanded,
        };
        return this with { SectionExpansion = new ReadOnlyDictionary<string, bool>(updated) };
    }
}
