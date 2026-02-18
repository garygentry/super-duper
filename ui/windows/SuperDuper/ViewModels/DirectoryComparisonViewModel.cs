using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.NativeMethods;
using System.Collections.ObjectModel;

namespace SuperDuper.ViewModels;

public partial class DirectoryComparisonViewModel : ObservableObject
{
    private EngineWrapper? _engine;
    private int _pageSize = 50;
    private int _currentOffset;

    [ObservableProperty]
    private bool _isLoading;

    [ObservableProperty]
    private double _minScore = 0.5;

    [ObservableProperty]
    private bool _hasNoPairs;

    [ObservableProperty]
    private bool _hasMoreResults;

    public ObservableCollection<SimilarPairViewModel> SimilarPairs { get; } = new();

    public void Initialize(EngineWrapper engine)
    {
        _engine = engine;
        Reload();
    }

    partial void OnMinScoreChanged(double value) => Reload();

    private void Reload()
    {
        _currentOffset = 0;
        SimilarPairs.Clear();
        HasNoPairs = false;
        HasMoreResults = false;
        LoadPage();
    }

    [RelayCommand]
    private void LoadPage()
    {
        if (_engine == null || IsLoading) return;

        IsLoading = true;
        try
        {
            var pairs = _engine.QuerySimilarDirectories(MinScore, _currentOffset, _pageSize);
            foreach (var p in pairs)
                SimilarPairs.Add(new SimilarPairViewModel(p, _engine));
            _currentOffset += pairs.Count;
            HasMoreResults = pairs.Count == _pageSize;
            HasNoPairs = SimilarPairs.Count == 0;
        }
        finally
        {
            IsLoading = false;
        }
    }

    [RelayCommand]
    private void LoadNextPage() => LoadPage();
}

public partial class SimilarPairViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;

    public string DirAPath { get; }
    public string DirBPath { get; }
    public string MatchType { get; }
    public string FormattedScore { get; }
    public string FormattedSharedBytes { get; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsDirANotMarked))]
    private bool _dirAMarked;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsDirBNotMarked))]
    private bool _dirBMarked;

    public bool IsDirANotMarked => !DirAMarked;
    public bool IsDirBNotMarked => !DirBMarked;

    public SimilarPairViewModel(DirectorySimilarityInfo info, EngineWrapper engine)
    {
        _engine = engine;
        DirAPath = info.DirAPath;
        DirBPath = info.DirBPath;
        MatchType = info.MatchType;
        FormattedScore = info.FormattedScore;
        FormattedSharedBytes = info.FormattedSharedBytes;
    }

    [RelayCommand]
    private void MarkAForDeletion()
    {
        _engine.MarkDirectoryForDeletion(DirAPath);
        DirAMarked = true;
    }

    [RelayCommand]
    private void MarkBForDeletion()
    {
        _engine.MarkDirectoryForDeletion(DirBPath);
        DirBMarked = true;
    }
}
