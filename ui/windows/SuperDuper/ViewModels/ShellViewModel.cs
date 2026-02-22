using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Services;

namespace SuperDuper.ViewModels;

/// <summary>
/// Drives the MainWindow shell: bottom status bar, global undo/redo, deletion queue count.
/// Subscribes to UndoService.StackChanged and DatabaseService deletion queue changes.
/// </summary>
public partial class ShellViewModel : ObservableObject
{
    private readonly IDatabaseService _db;
    private readonly IUndoService _undo;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsDeletionQueueNonEmpty))]
    public partial int DeletionQueueCount { get; set; }

    [ObservableProperty]
    public partial string DeletionQueueSummary { get; set; } = "No files marked for deletion";

    public bool IsDeletionQueueNonEmpty => DeletionQueueCount > 0;

    [ObservableProperty]
    public partial bool CanUndo { get; set; }

    [ObservableProperty]
    public partial bool CanRedo { get; set; }

    [ObservableProperty]
    public partial string? UndoDescription { get; set; }

    [ObservableProperty]
    public partial string? RedoDescription { get; set; }

    public ShellViewModel(IDatabaseService db, IUndoService undo)
    {
        _db = db;
        _undo = undo;
        _undo.StackChanged += OnStackChanged;
        OnStackChanged(null, EventArgs.Empty);
    }

    [RelayCommand]
    private async Task UndoAsync()
    {
        await _undo.UndoAsync();
        await RefreshDeletionCountAsync();
    }

    [RelayCommand]
    private async Task RedoAsync()
    {
        await _undo.RedoAsync();
        await RefreshDeletionCountAsync();
    }

    [RelayCommand]
    private static void OpenDeletionDialog()
    {
        // Handled by MainWindow via event/messenger pattern
        // ShellViewModel raises a request; MainWindow opens DeletionConfirmationDialog
        OpenDeletionDialogRequested?.Invoke(null, EventArgs.Empty);
    }

    public static event EventHandler? OpenDeletionDialogRequested;

    public async Task RefreshDeletionCountAsync()
    {
        var count = await _db.GetDeletionQueueCountAsync();
        DeletionQueueCount = count;
        DeletionQueueSummary = count == 0
            ? "No files marked for deletion"
            : $"{count:N0} file{(count == 1 ? "" : "s")} marked for deletion";
    }

    private void OnStackChanged(object? sender, EventArgs e)
    {
        CanUndo = _undo.CanUndo;
        CanRedo = _undo.CanRedo;
        UndoDescription = _undo.UndoDescription;
        RedoDescription = _undo.RedoDescription;
    }
}
