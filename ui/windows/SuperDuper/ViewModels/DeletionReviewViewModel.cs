using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.NativeMethods;

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
    private uint _lastSuccessCount;

    [ObservableProperty]
    private uint _lastErrorCount;

    public event EventHandler<(string Title, string Detail)>? ErrorOccurred;

    public void Initialize(EngineWrapper engine)
    {
        _engine = engine;
        RefreshSummary();
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
    }

    [RelayCommand]
    private void ExecuteDeletion()
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
        }
        finally
        {
            IsExecuting = false;
        }
    }

    private static string FormatBytes(long bytes)
    {
        string[] sizes = { "B", "KB", "MB", "GB", "TB" };
        double len = bytes;
        int order = 0;
        while (len >= 1024 && order < sizes.Length - 1)
        {
            order++;
            len /= 1024;
        }
        return $"{len:0.##} {sizes[order]}";
    }
}
