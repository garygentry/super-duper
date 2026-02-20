using SuperDuper.Models;

namespace SuperDuper.Models;

public interface IUndoableAction
{
    string Description { get; }
    Task UndoAsync();
    Task RedoAsync();
}

/// <summary>Undo a single Keep/Delete/Skip decision.</summary>
public class SetDecisionAction : IUndoableAction
{
    private readonly Services.IDatabaseService _db;
    private readonly long _fileId;
    private readonly long _groupId;
    private readonly ReviewAction _newAction;
    private readonly ReviewAction? _previousAction;
    private readonly long? _sessionId;

    public string Description { get; }

    public SetDecisionAction(
        Services.IDatabaseService db,
        long fileId, long groupId,
        ReviewAction newAction, ReviewAction? previousAction,
        long? sessionId, string fileName)
    {
        _db = db;
        _fileId = fileId;
        _groupId = groupId;
        _newAction = newAction;
        _previousAction = previousAction;
        _sessionId = sessionId;
        Description = $"Set {fileName} to {newAction}";
    }

    public async Task UndoAsync()
    {
        if (_previousAction.HasValue)
            await _db.UpsertDecisionAsync(_fileId, _groupId, _previousAction.Value, _sessionId);
        // If no previous action, we'd ideally delete the row — but UPSERT doesn't support delete.
        // For now, set to Skip as a neutral state if no prior decision existed.
        else
            await _db.UpsertDecisionAsync(_fileId, _groupId, ReviewAction.Skip, _sessionId);
    }

    public async Task RedoAsync()
    {
        await _db.UpsertDecisionAsync(_fileId, _groupId, _newAction, _sessionId);
    }
}

/// <summary>Undo a bulk auto-select strategy applied to multiple groups.</summary>
public class BulkDecisionAction : IUndoableAction
{
    private readonly Services.IDatabaseService _db;
    private readonly IReadOnlyList<(long FileId, long GroupId, ReviewAction NewAction, ReviewAction? OldAction)> _changes;
    private readonly long? _sessionId;

    public string Description { get; }

    public BulkDecisionAction(
        Services.IDatabaseService db,
        IReadOnlyList<(long FileId, long GroupId, ReviewAction NewAction, ReviewAction? OldAction)> changes,
        long? sessionId,
        string strategyName)
    {
        _db = db;
        _changes = changes;
        _sessionId = sessionId;
        Description = $"Auto-select: {strategyName} ({changes.Count} files)";
    }

    public async Task UndoAsync()
    {
        foreach (var (fileId, groupId, _, oldAction) in _changes)
        {
            if (oldAction.HasValue)
                await _db.UpsertDecisionAsync(fileId, groupId, oldAction.Value, _sessionId);
            else
                await _db.UpsertDecisionAsync(fileId, groupId, ReviewAction.Skip, _sessionId);
        }
    }

    public async Task RedoAsync()
    {
        foreach (var (fileId, groupId, newAction, _) in _changes)
            await _db.UpsertDecisionAsync(fileId, groupId, newAction, _sessionId);
    }
}

/// <summary>Undo marking an entire directory for deletion.</summary>
public class DirMarkAction : IUndoableAction
{
    private readonly Services.IDatabaseService _db;
    private readonly IReadOnlyList<(long FileId, long GroupId)> _files;
    private readonly long? _sessionId;
    private readonly string _dirPath;

    public string Description => $"Mark directory for deletion: {_dirPath}";

    public DirMarkAction(
        Services.IDatabaseService db,
        IReadOnlyList<(long FileId, long GroupId)> files,
        long? sessionId,
        string dirPath)
    {
        _db = db;
        _files = files;
        _sessionId = sessionId;
        _dirPath = dirPath;
    }

    public async Task UndoAsync()
    {
        foreach (var (fileId, groupId) in _files)
            await _db.UpsertDecisionAsync(fileId, groupId, ReviewAction.Skip, _sessionId);
    }

    public async Task RedoAsync()
    {
        foreach (var (fileId, groupId) in _files)
            await _db.UpsertDecisionAsync(fileId, groupId, ReviewAction.Delete, _sessionId);
    }
}
