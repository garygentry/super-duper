using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.NativeMethods;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading.Tasks;

namespace SuperDuper.ViewModels;

public partial class DeletionReviewViewModel : ObservableObject
{
    private EngineWrapper? _engine;

    [ObservableProperty]
    private long _fileCount;

    [ObservableProperty]
    private long _totalBytes;

    [ObservableProperty]
    private string _formattedTotalBytes = "0 B";

    [ObservableProperty]
    private string _statusMessage = "Review files marked for deletion before executing.";

    [ObservableProperty]
    private bool _isExecuting;

    [ObservableProperty]
    private bool _isLoadingFiles;

    [ObservableProperty]
    private bool _hasNoFiles;

    [ObservableProperty]
    private uint _lastSuccessCount;

    [ObservableProperty]
    private uint _lastErrorCount;

    public ObservableCollection<MarkedFileViewModel> MarkedFiles { get; } = new();

    public event EventHandler<(string Title, string Detail)>? ErrorOccurred;

    public void Initialize(EngineWrapper engine)
    {
        _engine = engine;
        RefreshSummary();
        _ = LoadMarkedFilesAsync();
    }

    [RelayCommand]
    private void RefreshSummary()
    {
        if (_engine == null) return;

        var (count, bytes) = _engine.GetDeletionPlanSummary();
        FileCount = count;
        TotalBytes = bytes;
        FormattedTotalBytes = FormatBytes(bytes);
        StatusMessage = count > 0
            ? $"{count} files marked for deletion ({FormattedTotalBytes})"
            : "No files marked for deletion.";
    }

    [RelayCommand]
    private void AutoMark()
    {
        if (_engine == null) return;

        _engine.AutoMarkForDeletion();
        RefreshSummary();
        StatusMessage = $"Auto-marked duplicates. {FileCount} files ({FormattedTotalBytes}) ready for deletion.";
        _ = LoadMarkedFilesAsync();
    }

    // Called by DeletionReviewPage code-behind after confirmation dialog.
    internal void ExecuteDeletion()
    {
        if (_engine == null || IsExecuting || FileCount == 0) return;

        IsExecuting = true;
        try
        {
            var (success, errors) = _engine.ExecuteDeletionPlan();
            LastSuccessCount = success;
            LastErrorCount = errors;
            if (errors > 0)
            {
                StatusMessage = $"Deleted {success} files with {errors} errors.";
                ErrorOccurred?.Invoke(this, ("Deletion Errors", $"{errors} files could not be deleted."));
            }
            else
            {
                StatusMessage = $"Successfully deleted {success} files.";
            }
            RefreshSummary();
            _ = LoadMarkedFilesAsync();
        }
        finally
        {
            IsExecuting = false;
        }
    }

    private async Task LoadMarkedFilesAsync()
    {
        if (_engine == null) return;

        IsLoadingFiles = true;
        try
        {
            var marked = await Task.Run(() =>
            {
                var result = new List<MarkedFileViewModel>();
                var (groups, _) = _engine.QueryDuplicateGroups(0, 10000);
                foreach (var group in groups)
                {
                    var files = _engine.QueryFilesInGroup(group.Id);
                    foreach (var file in files.Where(f => f.IsMarkedForDeletion))
                        result.Add(new MarkedFileViewModel(file, _engine, this));
                }
                return result;
            });

            MarkedFiles.Clear();
            foreach (var vm in marked)
                MarkedFiles.Add(vm);
            HasNoFiles = MarkedFiles.Count == 0;
        }
        finally
        {
            IsLoadingFiles = false;
        }
    }

    internal void RemoveFile(MarkedFileViewModel vm)
    {
        MarkedFiles.Remove(vm);
        HasNoFiles = MarkedFiles.Count == 0;
        RefreshSummary();
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

public partial class MarkedFileViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;
    private readonly DeletionReviewViewModel _parent;

    public long Id { get; }
    public string CanonicalPath { get; }
    public string FileName { get; }
    public string FormattedSize { get; }

    public MarkedFileViewModel(NativeMethods.FileInfo file, EngineWrapper engine, DeletionReviewViewModel parent)
    {
        _engine = engine;
        _parent = parent;
        Id = file.Id;
        CanonicalPath = file.CanonicalPath;
        FileName = file.FileName;
        FormattedSize = FormatBytes(file.FileSize);
    }

    [RelayCommand]
    private void Unmark()
    {
        _engine.UnmarkForDeletion(Id);
        _parent.RemoveFile(this);
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
