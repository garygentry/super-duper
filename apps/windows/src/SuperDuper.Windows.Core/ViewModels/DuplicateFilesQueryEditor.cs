using System.ComponentModel;
using System.Globalization;
using System.Numerics;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed record FileFilterChip(string Key, string Text)
{
    public string AutomationName => $"Remove applied filter: {Text}";
}

public sealed partial class DuplicateFilesViewModel
{
    private sealed record FilterDraft(string Search, bool Exact, string Size, string Unit, bool GiB,
        bool ThreeCopies, bool Across, string Extension, bool NoExtension, bool AllExtensions,
        string? Root, string? Drive);

    private DuplicateFileGroupFilter _appliedFilter = new(string.Empty, "0");
    private FilterDraft _appliedDraft = new("", false, "", "B", false, false, false, "", false, false, null, null);
    private FilterDraft? _requestedDraft;
    private bool _hasFilteredResults;
    private DuplicateFileGroupSortField _appliedSortField = DuplicateFileGroupSortField.RecoverableBytes;
    private WorkerSortDirection _appliedSortDirection = WorkerSortDirection.Descending;
    private string _minimumSizeUnit = "B";
    private IReadOnlyList<FileFilterChip> _appliedFilters = [];

    public IReadOnlyList<string> SizeUnits { get; } = ["B", "KiB", "MiB", "GiB", "TiB"];
    public int RootFacetSortIndex
    {
        get => _rootFacetSortField == DuplicateFileSelectedRootFacetSortField.Value ? 1 : 0;
        set => _ = ApplyRootFacetSortAsync(value == 1 ? DuplicateFileSelectedRootFacetSortField.Value : DuplicateFileSelectedRootFacetSortField.MatchingGroupCount,
            value == 1 ? WorkerSortDirection.Ascending : WorkerSortDirection.Descending);
    }
    public int DriveFacetSortIndex
    {
        get => _driveFacetSortField == DuplicateFileDriveFacetSortField.Value ? 1 : 0;
        set => _ = ApplyDriveFacetSortAsync(value == 1 ? DuplicateFileDriveFacetSortField.Value : DuplicateFileDriveFacetSortField.MatchingGroupCount,
            value == 1 ? WorkerSortDirection.Ascending : WorkerSortDirection.Descending);
    }
    public string MinimumSizeUnit
    {
        get => _minimumSizeUnit;
        set => SetProperty(ref _minimumSizeUnit, value);
    }
    public IReadOnlyList<FileFilterChip> AppliedFilters
    {
        get => _appliedFilters;
        private set => SetProperty(ref _appliedFilters, value);
    }
    public IAsyncRelayCommand<FileFilterChip> RemoveFilterCommand { get; private set; } = null!;
    public string FilteredSetsText => _hasFilteredResults ? MatchingSetCountText : "—";
    public string FilteredCopiesText => _hasFilteredResults ? MatchingCopyCountText : "—";
    public string FilteredSavingsText => _hasFilteredResults ? PotentialRecoverableText : "—";
    public string FilteredSavingsExactText => _hasFilteredResults ? $"{Summary.PotentialRecoverableBytes} bytes" : "Results have not loaded";
    public string QueryEditorStatus => IsUnavailable ? StateMessage : IsLoading
        ? _hasFilteredResults ? "Updating results… Showing the previous applied query." : "Loading filtered results…"
        : HasError ? _hasFilteredResults ? "Query not replaced. Previous applied results remain; correct the filters or retry Apply." : "Results unavailable. Correct the filters or retry Apply."
        : ReadDraft() != _appliedDraft ? "Unapplied changes · use Apply or Enter."
        : "Filtered results · totals cover matching sets.";

    private FilterDraft ReadDraft() => new(SearchText, ExactPathMatch, MinimumSizeText, MinimumSizeUnit,
        OneGigabyteOrLarger, ThreeOrMoreCopies, AcrossDrives, ExtensionText, WithoutExtension,
        AllMembersMustMatchExtension, SelectedRootFacet?.Value, SelectedDriveFacet?.Value);

    protected override void OnPropertyChanged(PropertyChangedEventArgs e)
    {
        base.OnPropertyChanged(e);
        if (e.PropertyName is nameof(SearchText) or nameof(ExactPathMatch) or nameof(MinimumSizeText)
            or nameof(MinimumSizeUnit) or nameof(OneGigabyteOrLarger) or nameof(ThreeOrMoreCopies)
            or nameof(AcrossDrives) or nameof(ExtensionText) or nameof(WithoutExtension)
            or nameof(AllMembersMustMatchExtension) or nameof(SelectedRootFacet) or nameof(SelectedDriveFacet)
            or nameof(IsLoading) or nameof(ErrorMessage) or nameof(StateMessage) or nameof(Run))
            OnPropertyChanged(nameof(QueryEditorStatus));
        if (e.PropertyName == nameof(Summary)) RaiseFilteredTotals();
    }

    private void RaiseFilteredTotals()
    {
        OnPropertyChanged(nameof(FilteredSetsText));
        OnPropertyChanged(nameof(FilteredCopiesText));
        OnPropertyChanged(nameof(FilteredSavingsText));
        OnPropertyChanged(nameof(FilteredSavingsExactText));
    }

    private static bool TryConvertSize(string text, string unit, out long bytes)
    {
        bytes = 0;
        var multiplier = unit switch
        {
            "B" => 1L, "KiB" => 1024L, "MiB" => 1048576L,
            "GiB" => 1073741824L, "TiB" => 1099511627776L, _ => 0L,
        };
        // Rational arithmetic avoids decimal/double rounding, including one byte in TiB.
        // Bound editor work separately from the worker's unchanged signed 64-bit byte ceiling.
        if (text.Length is 0 or > 256 || multiplier == 0) return false;
        var parts = text.Split('.');
        if (parts.Length > 2 || parts.Any(part => part.Length == 0 || part.Any(c => c is < '0' or > '9'))) return false;
        var numerator = BigInteger.Parse(string.Concat(parts), CultureInfo.InvariantCulture) * multiplier;
        var denominator = BigInteger.Pow(10, parts.Length == 2 ? parts[1].Length : 0);
        var whole = BigInteger.DivRem(numerator, denominator, out var remainder);
        if (!remainder.IsZero || whole > long.MaxValue) return false;
        bytes = (long)whole;
        return true;
    }

    // Paging, facet sorting and rule scope always use the accepted server query, never the editor.
    private bool TryGetAppliedFilter(out DuplicateFileGroupFilter filter)
    {
        filter = _appliedFilter;
        return true;
    }

    private void AcceptFilter(DuplicateFileGroupFilter filter)
    {
        var changed = !_hasFilteredResults || _appliedFilter != filter;
        _appliedFilter = filter;
        _appliedSortField = _sortField;
        _appliedSortDirection = _sortDirection;
        _appliedDraft = _requestedDraft ?? _appliedDraft;
        _hasFilteredResults = true;
        if (changed)
        {
            PreferenceRules.InvalidateFilter();
            var chips = new List<FileFilterChip>();
            if (filter.Search.Length > 0 || filter.PathMatch == DuplicateFilePathMatchMode.Exact)
                chips.Add(new("path", $"{(filter.PathMatch == DuplicateFilePathMatchMode.Exact ? "Exact path" : "Path contains")}: {filter.Search}"));
            if (filter.MinimumSize != "0") chips.Add(new("size", $"One copy ≥ {filter.MinimumSize} bytes"));
            if (filter.MinimumCopyCount > 2) chips.Add(new("copies", "Three or more copies"));
            if (filter.AcrossDrives) chips.Add(new("across", "Across drives"));
            if (filter.Extension is not null)
                chips.Add(new("extension", $"{(filter.ExtensionMatch == DuplicateFileExtensionMatchMode.AllMembers ? "All copies" : "Any copy")}: {(filter.Extension.Length == 0 ? "no extension" : "." + filter.Extension)}"));
            if (filter.SelectedRoot is not null) chips.Add(new("root", $"Root: {filter.SelectedRoot}"));
            if (filter.SelectedDrive is not null) chips.Add(new("drive", $"Drive: {filter.SelectedDrive}"));
            AppliedFilters = chips;
        }
        StateMessage = AppliedFilters.Count == 0 ? "No duplicate files in this completed run." : "No duplicate sets match the applied filters. Clear filters to see all sets.";
        RaiseFilteredTotals();
        OnPropertyChanged(nameof(QueryEditorStatus));
    }

    private async Task RemoveFilterAsync(FileFilterChip? chip)
    {
        if (chip is null || !AppliedFilters.Contains(chip) || Run?.Status != "completed") return;
        var filter = _appliedFilter;
        var draft = _appliedDraft;
        switch (chip.Key)
        {
            case "path":
                SearchText = ""; ExactPathMatch = false;
                filter = filter with { Search = "", PathMatch = DuplicateFilePathMatchMode.Substring };
                draft = draft with { Search = "", Exact = false }; break;
            case "size":
                MinimumSizeText = ""; MinimumSizeUnit = "B"; OneGigabyteOrLarger = false;
                filter = filter with { MinimumSize = "0" };
                draft = draft with { Size = "", Unit = "B", GiB = false }; break;
            case "copies":
                ThreeOrMoreCopies = false; filter = filter with { MinimumCopyCount = 2 };
                draft = draft with { ThreeCopies = false }; break;
            case "across":
                AcrossDrives = false; filter = filter with { AcrossDrives = false };
                draft = draft with { Across = false }; break;
            case "extension":
                ExtensionText = ""; WithoutExtension = false; AllMembersMustMatchExtension = false;
                filter = filter with { Extension = null, ExtensionMatch = DuplicateFileExtensionMatchMode.AnyMember };
                draft = draft with { Extension = "", NoExtension = false, AllExtensions = false }; break;
            case "root":
                SelectedRootFacet = SelectedRootFacetOptions.FirstOrDefault(x => x.Value is null) ?? new();
                filter = filter with { SelectedRoot = null }; draft = draft with { Root = null }; break;
            case "drive":
                SelectedDriveFacet = DriveFacetOptions.FirstOrDefault(x => x.Value is null) ?? new();
                filter = filter with { SelectedDrive = null }; draft = draft with { Drive = null }; break;
            default: return;
        }
        await ResetAndLoadGroupsAsync(preserveDisplayedResults: true, requestedFilter: filter, requestedDraft: draft);
    }
}
