using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Converters;
using SuperDuper.NativeMethods;
using SuperDuper.Services.Platform;
using System.Collections.ObjectModel;

namespace SuperDuper.ViewModels;

public partial class DirectoriesViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;
    private readonly IShellIntegrationService _shell;
    private IReadOnlyList<DirectorySimilarityInfo> _allPairs = Array.Empty<DirectorySimilarityInfo>();

    public ObservableCollection<DirectoryPairViewModel> ExactPairs { get; } = new();
    public ObservableCollection<DirectoryPairViewModel> SubsetPairs { get; } = new();
    public ObservableCollection<DirectoryPairViewModel> OverlapPairs { get; } = new();

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ExactTabHeader), nameof(SubsetTabHeader), nameof(OverlapTabHeader))]
    private int _exactCount;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(SubsetTabHeader))]
    private int _subsetCount;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(OverlapTabHeader))]
    private int _overlapCount;

    [ObservableProperty]
    private bool _isNestingRolledUp = true;

    partial void OnIsNestingRolledUpChanged(bool value) => ApplyNestingFilter();

    public string ExactTabHeader => $"Exact ({ExactCount})";
    public string SubsetTabHeader => $"Subsets ({SubsetCount})";
    public string OverlapTabHeader => $"Overlaps ({OverlapCount})";

    public DirectoriesViewModel(EngineWrapper engine, IShellIntegrationService shell)
    {
        _engine = engine;
        _shell = shell;
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
