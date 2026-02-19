using SuperDuper.Models;

namespace SuperDuper.Services;

public interface IUndoService
{
    bool CanUndo { get; }
    bool CanRedo { get; }
    string? UndoDescription { get; }
    string? RedoDescription { get; }

    event EventHandler? StackChanged;

    void Push(IUndoableAction action);
    Task UndoAsync();
    Task RedoAsync();
    void Clear();
}
