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

    public ObservableCollection<DirectorySimilarityInfo> SimilarPairs { get; } = new();

    public void Initialize(EngineWrapper engine)
    {
        _engine = engine;
        _currentOffset = 0;
        SimilarPairs.Clear();
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
            {
                SimilarPairs.Add(p);
            }
            _currentOffset += pairs.Count;
        }
        finally
        {
            IsLoading = false;
        }
    }

    [RelayCommand]
    private void LoadNextPage() => LoadPage();

    [RelayCommand]
    private void MarkDirectoryForDeletion(string directoryPath)
    {
        _engine?.MarkDirectoryForDeletion(directoryPath);
    }
}
