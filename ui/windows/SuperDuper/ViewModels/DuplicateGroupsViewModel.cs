using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.NativeMethods;
using System.Collections.ObjectModel;

namespace SuperDuper.ViewModels;

/// <summary>
/// ViewModel for the DuplicateGroupsPage — virtual list of groups sorted by wasted bytes.
/// Expand a group to see all file paths with checkboxes for deletion marking.
/// </summary>
public partial class DuplicateGroupsViewModel : ObservableObject
{
    private EngineWrapper? _engine;
    private int _pageSize = 50;
    private int _currentOffset;
    private long _totalWastedBytes;

    [ObservableProperty]
    private bool _isLoading;

    [ObservableProperty]
    private int _totalGroups;

    public bool HasMoreGroups => _currentOffset < TotalGroups;

    [ObservableProperty]
    private bool _hasNoGroups;

    public string TotalWastedLabel => TotalGroups == 0
        ? "No duplicate groups found"
        : $"{TotalGroups} groups · {FormatBytes(_totalWastedBytes)} wasted";

    public ObservableCollection<DuplicateGroupViewModel> Groups { get; } = new();

    public void Initialize(EngineWrapper engine)
    {
        _engine = engine;
        _currentOffset = 0;
        _totalWastedBytes = 0;
        LoadPage();
    }

    [RelayCommand]
    private void LoadPage()
    {
        if (_engine == null || IsLoading) return;

        IsLoading = true;
        try
        {
            var (groups, total) = _engine.QueryDuplicateGroups(_currentOffset, _pageSize);
            TotalGroups = total;

            foreach (var g in groups)
            {
                Groups.Add(new DuplicateGroupViewModel(g, _engine));
                _totalWastedBytes += g.WastedBytes;
            }

            _currentOffset += groups.Count;
            OnPropertyChanged(nameof(HasMoreGroups));
            OnPropertyChanged(nameof(TotalWastedLabel));
            HasNoGroups = TotalGroups == 0;
        }
        finally
        {
            IsLoading = false;
        }
    }

    [RelayCommand]
    private void LoadNextPage()
    {
        if (HasMoreGroups)
        {
            LoadPage();
        }
    }
}

public partial class DuplicateGroupViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;

    public DuplicateGroupInfo Group { get; }

    public string FileCountLabel => $"{Group.FileCount} files";
    public string FileSizeLabel => FormatBytes(Group.FileSize) + " each";
    public string WastedBytesLabel => FormatBytes(Group.WastedBytes) + " wasted";

    [ObservableProperty]
    private bool _isExpanded;

    public ObservableCollection<FileViewModel> Files { get; } = new();

    public DuplicateGroupViewModel(DuplicateGroupInfo group, EngineWrapper engine)
    {
        Group = group;
        _engine = engine;
    }

    partial void OnIsExpandedChanged(bool value)
    {
        if (value && Files.Count == 0)
            LoadFiles();
    }

    private void LoadFiles()
    {
        var files = _engine.QueryFilesInGroup(Group.Id);
        foreach (var f in files)
            Files.Add(new FileViewModel(f, _engine));
    }

    private static string FormatBytes(long bytes)
    {
        string[] sizes = { "B", "KB", "MB", "GB", "TB" };
        double len = bytes;
        int order = 0;
        while (len >= 1024 && order < sizes.Length - 1) { order++; len /= 1024; }
        return $"{len:0.##} {sizes[order]}";
    }
}

public partial class FileViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;

    public NativeMethods.FileInfo File { get; }

    [ObservableProperty]
    private bool _isMarkedForDeletion;

    public FileViewModel(NativeMethods.FileInfo file, EngineWrapper engine)
    {
        File = file;
        _engine = engine;
        // Load initial state from DB without triggering the change handler
        _isMarkedForDeletion = file.IsMarkedForDeletion;
    }

    partial void OnIsMarkedForDeletionChanged(bool value)
    {
        try
        {
            if (value)
                _engine.MarkForDeletion(File.Id);
            else
                _engine.UnmarkForDeletion(File.Id);
        }
        catch
        {
            // Revert on failure without re-triggering the handler
            SetProperty(ref _isMarkedForDeletion, !value, nameof(IsMarkedForDeletion));
        }
    }
}
