using CommunityToolkit.Mvvm.ComponentModel;
using SuperDuper.Models;
using SuperDuper.Services;

namespace SuperDuper.ViewModels;

public partial class ExplorerViewModel : ObservableObject
{
    private readonly IDatabaseService _db;
    private readonly IUndoService _undo;

    [ObservableProperty]
    private string? _selectedDirectory;

    [ObservableProperty]
    private DbFileInfo? _selectedFile;

    public ExplorerViewModel(IDatabaseService db, IUndoService undo)
    {
        _db = db;
        _undo = undo;
    }

    public async Task SetDecisionAsync(long fileId, long groupId, ReviewAction action, long? sessionId = null)
    {
        var old = await _db.GetDecisionAsync(fileId);
        await _db.UpsertDecisionAsync(fileId, groupId, action, sessionId);
        _undo.Push(new SetDecisionAction(
            _db, fileId, groupId, action, old, sessionId,
            SelectedFile?.FileName ?? fileId.ToString()));
    }
}
