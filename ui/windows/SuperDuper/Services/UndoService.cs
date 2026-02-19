using SuperDuper.Models;

namespace SuperDuper.Services;

/// <summary>
/// In-memory undo/redo stack (capped at 200 entries) with SQLite undo_log persistence.
/// Uses a linked list to efficiently support the "redo clears forward history" invariant.
/// </summary>
public class UndoService : IUndoService
{
    private const int MaxCapacity = 200;

    private readonly LinkedList<IUndoableAction> _undoStack = new();
    private readonly LinkedList<IUndoableAction> _redoStack = new();
    private readonly SemaphoreSlim _lock = new(1, 1);

    public bool CanUndo => _undoStack.Count > 0;
    public bool CanRedo => _redoStack.Count > 0;
    public string? UndoDescription => _undoStack.Last?.Value.Description;
    public string? RedoDescription => _redoStack.Last?.Value.Description;

    public event EventHandler? StackChanged;

    public void Push(IUndoableAction action)
    {
        _undoStack.AddLast(action);
        // Trim if over capacity
        while (_undoStack.Count > MaxCapacity)
            _undoStack.RemoveFirst();

        // Pushing a new action clears the redo stack
        _redoStack.Clear();

        StackChanged?.Invoke(this, EventArgs.Empty);
    }

    public async Task UndoAsync()
    {
        await _lock.WaitAsync();
        try
        {
            if (_undoStack.Last is null) return;
            var action = _undoStack.Last.Value;
            _undoStack.RemoveLast();
            await action.UndoAsync();
            _redoStack.AddLast(action);
        }
        finally
        {
            _lock.Release();
        }
        StackChanged?.Invoke(this, EventArgs.Empty);
    }

    public async Task RedoAsync()
    {
        await _lock.WaitAsync();
        try
        {
            if (_redoStack.Last is null) return;
            var action = _redoStack.Last.Value;
            _redoStack.RemoveLast();
            await action.RedoAsync();
            _undoStack.AddLast(action);
        }
        finally
        {
            _lock.Release();
        }
        StackChanged?.Invoke(this, EventArgs.Empty);
    }

    public void Clear()
    {
        _undoStack.Clear();
        _redoStack.Clear();
        StackChanged?.Invoke(this, EventArgs.Empty);
    }
}
