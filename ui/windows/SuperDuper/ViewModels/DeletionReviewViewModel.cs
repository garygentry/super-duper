using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.NativeMethods;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Linq;
using System.Threading.Tasks;

namespace SuperDuper.ViewModels;

public partial class DeletionReviewViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;
    private readonly Services.SettingsService _settings;

    [ObservableProperty]
    public partial long FileCount { get; set; }

    [ObservableProperty]
    public partial long TotalBytes { get; set; }

    [ObservableProperty]
    public partial string FormattedTotalBytes { get; set; } = "0 B";

    [ObservableProperty]
    public partial string StatusMessage { get; set; } = "Review files marked for deletion before executing.";

    [ObservableProperty]
    public partial bool IsExecuting { get; set; }

    [ObservableProperty]
    public partial bool IsLoadingFiles { get; set; }

    [ObservableProperty]
    public partial bool HasNoFiles { get; set; }

    [ObservableProperty]
    public partial uint LastSuccessCount { get; set; }

    [ObservableProperty]
    public partial uint LastErrorCount { get; set; }

    public ObservableCollection<MarkedFileViewModel> MarkedFiles { get; } = new();

    public event EventHandler<(string Title, string Detail)>? ErrorOccurred;

    public bool UseTrash => _settings.UseTrashForDeletion;

    public DeletionReviewViewModel(EngineWrapper engine, Services.SettingsService settings)
    {
        _engine = engine;
        _settings = settings;
        RefreshSummary();
        LoadMarkedFilesAsync().FireAndForget(nameof(DeletionReviewViewModel) + ".ctor");
    }

    [RelayCommand]
    private void RefreshSummary()
    {
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
        _engine.AutoMarkForDeletion();
        RefreshSummary();
        StatusMessage = $"Auto-marked duplicates. {FileCount} files ({FormattedTotalBytes}) ready for deletion.";
        LoadMarkedFilesAsync().FireAndForget(nameof(DeletionReviewViewModel) + "." + nameof(AutoMark));
    }

    // Called by DeletionReviewPage code-behind after confirmation dialog.
    internal async Task ExecuteDeletionAsync()
    {
        if (IsExecuting || FileCount == 0) return;

        IsExecuting = true;
        try
        {
            bool useTrash = UseTrash;
            var (success, errors) = await Task.Run(() => _engine.ExecuteDeletionPlan(useTrash));
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
            LoadMarkedFilesAsync().FireAndForget(nameof(DeletionReviewViewModel) + "." + nameof(ExecuteDeletionAsync));
        }
        finally
        {
            IsExecuting = false;
        }
    }

    private async Task LoadMarkedFilesAsync()
    {
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
    public string DisplayPath => CanonicalPath.StartsWith(@"\\?\") ? CanonicalPath[4..] : CanonicalPath;
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

    [RelayCommand]
    private void RevealInExplorer()
    {
        try { Process.Start("explorer.exe", $"/select,\"{CanonicalPath}\""); }
        catch { }
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
