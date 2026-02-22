using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using SuperDuper.Converters;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using SuperDuper.Services.Platform;
using System.Collections.ObjectModel;

namespace SuperDuper.ViewModels;

public partial class DirectoriesViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;
    private readonly IShellIntegrationService _shell;
    private readonly IDatabaseService _db;
    private readonly ScanService _scanService;
    private IReadOnlyList<DirectorySimilarityInfo> _allPairs = Array.Empty<DirectorySimilarityInfo>();

    public ObservableCollection<DirectoryPairViewModel> ExactPairs { get; } = new();
    public ObservableCollection<DirectoryPairViewModel> SubsetPairs { get; } = new();
    public ObservableCollection<DirectoryPairViewModel> OverlapPairs { get; } = new();

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ExactTabHeader), nameof(SubsetTabHeader), nameof(OverlapTabHeader))]
    public partial int ExactCount { get; set; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(SubsetTabHeader))]
    public partial int SubsetCount { get; set; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(OverlapTabHeader))]
    public partial int OverlapCount { get; set; }

    [ObservableProperty]
    public partial bool IsNestingRolledUp { get; set; } = true;

    partial void OnIsNestingRolledUpChanged(bool value) => ApplyNestingFilter();

    // Comparison panel state
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasSelectedPair))]
    public partial DirectoryPairViewModel? SelectedPair { get; set; }

    [ObservableProperty]
    public partial string SelectedPairDirA { get; set; } = "";

    [ObservableProperty]
    public partial string SelectedPairDirB { get; set; } = "";

    [ObservableProperty]
    public partial string ComparisonSummary { get; set; } = "";

    [ObservableProperty]
    public partial string MatchedCountLabel { get; set; } = "";

    [ObservableProperty]
    public partial string LeftOnlyCountLabel { get; set; } = "";

    [ObservableProperty]
    public partial string RightOnlyCountLabel { get; set; } = "";

    public Visibility HasSelectedPair => SelectedPair != null ? Visibility.Visible : Visibility.Collapsed;

    public ObservableCollection<ComparisonFileItem> LeftFiles { get; } = new();
    public ObservableCollection<ComparisonFileItem> RightFiles { get; } = new();

    public string ExactTabHeader => $"Exact ({ExactCount})";
    public string SubsetTabHeader => $"Subsets ({SubsetCount})";
    public string OverlapTabHeader => $"Overlaps ({OverlapCount})";

    public DirectoriesViewModel(EngineWrapper engine, IShellIntegrationService shell, IDatabaseService db, ScanService scanService)
    {
        _engine = engine;
        _shell = shell;
        _db = db;
        _scanService = scanService;
    }

    public async Task LoadAsync()
    {
        var pairs = await Task.Run(() => _engine.QuerySimilarDirectories(minScore: 0.0, offset: 0, limit: 500));
        _allPairs = pairs;
        ApplyNestingFilter();
    }

    private void ApplyNestingFilter()
    {
        ExactPairs.Clear();
        SubsetPairs.Clear();
        OverlapPairs.Clear();

        var filtered = IsNestingRolledUp
            ? _allPairs.Where(p => !IsNested(p, _allPairs)).ToList()
            : _allPairs.ToList();

        foreach (var p in filtered)
        {
            var vm = new DirectoryPairViewModel(p, _shell);
            switch (p.MatchType)
            {
                case "exact":
                    ExactPairs.Add(vm);
                    break;
                case "subset":
                    SubsetPairs.Add(vm);
                    break;
                default:
                    OverlapPairs.Add(vm);
                    break;
            }
        }

        ExactCount = ExactPairs.Count;
        SubsetCount = SubsetPairs.Count;
        OverlapCount = OverlapPairs.Count;
    }

    public async Task LoadComparisonAsync(DirectoryPairViewModel pair)
    {
        SelectedPair = pair;
        SelectedPairDirA = pair.DirADisplayPath;
        SelectedPairDirB = pair.DirBDisplayPath;

        LeftFiles.Clear();
        RightFiles.Clear();

        // Query files from both directories
        var sessionId = _scanService.ActiveSessionId ?? 0;
        var leftResult = await _db.QueryFilesInDirectoryAsync(pair.DirAPath, sessionId, 0, 500);
        var rightResult = await _db.QueryFilesInDirectoryAsync(pair.DirBPath, sessionId, 0, 500);

        // Build hash sets for comparison
        var leftHashMap = leftResult.Items.GroupBy(f => f.ContentHash)
            .ToDictionary(g => g.Key, g => g.ToList());
        var rightHashMap = rightResult.Items.GroupBy(f => f.ContentHash)
            .ToDictionary(g => g.Key, g => g.ToList());

        var rightHashes = new HashSet<long>(rightHashMap.Keys);

        int matched = 0, leftOnly = 0, rightOnly = 0;

        // Categorize left files
        foreach (var f in leftResult.Items)
        {
            var status = rightHashes.Contains(f.ContentHash) && f.ContentHash != 0
                ? MatchStatus.Matched : MatchStatus.LeftOnly;
            if (status == MatchStatus.Matched) matched++;
            else leftOnly++;
            LeftFiles.Add(new ComparisonFileItem(f.FileName, f.FileSize, status));
        }

        // Categorize right files
        var leftHashes = new HashSet<long>(leftHashMap.Keys);
        foreach (var f in rightResult.Items)
        {
            var status = leftHashes.Contains(f.ContentHash) && f.ContentHash != 0
                ? MatchStatus.Matched : MatchStatus.RightOnly;
            if (status == MatchStatus.RightOnly) rightOnly++;
            RightFiles.Add(new ComparisonFileItem(f.FileName, f.FileSize, status));
        }

        var total = leftResult.Items.Count + rightResult.Items.Count;
        var matchPercent = total > 0 ? (double)matched * 2 / total * 100 : 0;
        ComparisonSummary = $"{matched} of {Math.Max(leftResult.Items.Count, rightResult.Items.Count)} files match ({matchPercent:F1}%)";
        MatchedCountLabel = $"{matched} matched";
        LeftOnlyCountLabel = $"{leftOnly} left-only";
        RightOnlyCountLabel = $"{rightOnly} right-only";
    }

    private static bool IsNested(DirectorySimilarityInfo pair, IReadOnlyList<DirectorySimilarityInfo> allPairs)
    {
        // A pair is nested if one of its paths is an ancestor of a path in another pair
        return allPairs.Any(other =>
            other.Id != pair.Id &&
            (pair.DirAPath.StartsWith(other.DirAPath, StringComparison.OrdinalIgnoreCase) ||
             pair.DirAPath.StartsWith(other.DirBPath, StringComparison.OrdinalIgnoreCase) ||
             pair.DirBPath.StartsWith(other.DirAPath, StringComparison.OrdinalIgnoreCase) ||
             pair.DirBPath.StartsWith(other.DirBPath, StringComparison.OrdinalIgnoreCase)));
    }
}

public partial class DirectoryPairViewModel : ObservableObject
{
    private readonly DirectorySimilarityInfo _info;
    private readonly IShellIntegrationService _shell;

    public string DirAPath => _info.DirAPath;
    public string DirBPath => _info.DirBPath;
    public string DirADisplayPath => _info.DirADisplayPath;
    public string DirBDisplayPath => _info.DirBDisplayPath;
    public string Summary => $"{_info.FormattedSharedBytes} shared · {_info.FormattedScore} match";

    public string DeleteLeftAccessibleName => $"Mark {DirADisplayPath} for deletion";
    public string DeleteRightAccessibleName => $"Mark {DirBDisplayPath} for deletion";
    public string ReviewDiffAccessibleName => $"Review differences between {Path.GetFileName(DirADisplayPath)} and {Path.GetFileName(DirBDisplayPath)}";

    public DirectoryPairViewModel(DirectorySimilarityInfo info, IShellIntegrationService shell)
    {
        _info = info;
        _shell = shell;
    }

    [RelayCommand]
    private void DeleteLeft() => _shell.RevealInExplorer(_info.DirAPath);

    [RelayCommand]
    private void DeleteRight() => _shell.RevealInExplorer(_info.DirBPath);

    [RelayCommand]
    private async Task ReviewDifferencesAsync()
    {
        // Opens the DirectoryDiffDialog — triggered from page code-behind for XamlRoot access
        ReviewDifferencesRequested?.Invoke(this, _info);
    }

    public event EventHandler<DirectorySimilarityInfo>? ReviewDifferencesRequested;
}

public enum MatchStatus { Matched, LeftOnly, RightOnly }

public class ComparisonFileItem
{
    public string FileName { get; }
    public long Size { get; }
    public MatchStatus Status { get; }

    public ComparisonFileItem(string fileName, long size, MatchStatus status)
    {
        FileName = fileName;
        Size = size;
        Status = status;
    }

    public string FormattedSize => Converters.FileSizeConverter.FormatBytes(Size);

    public string StatusIcon => Status switch
    {
        MatchStatus.Matched => "\uE73E",    // Checkmark
        MatchStatus.LeftOnly => "\uE72A",   // Left arrow
        MatchStatus.RightOnly => "\uE72A",  // Right arrow
        _ => ""
    };

    public SolidColorBrush StatusBrush => Status switch
    {
        MatchStatus.Matched => LookupBrush("SystemFillColorSuccessBrush"),
        MatchStatus.LeftOnly => LookupBrush("AccentFillColorDefaultBrush"),
        MatchStatus.RightOnly => LookupBrush("SystemFillColorCautionBrush"),
        _ => new SolidColorBrush()
    };

    private static SolidColorBrush LookupBrush(string key)
    {
        try
        {
            if (Microsoft.UI.Xaml.Application.Current.Resources[key] is SolidColorBrush b) return b;
            // Some theme resources are Brushes, not SolidColorBrush — try casting
            if (Microsoft.UI.Xaml.Application.Current.Resources[key] is Brush brush)
                return new SolidColorBrush(Microsoft.UI.Colors.Gray);
        }
        catch { }
        return new SolidColorBrush(Microsoft.UI.Colors.Gray);
    }
}
