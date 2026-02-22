using Windows.System;

namespace SuperDuper.Models;

/// <summary>
/// Single source of truth for all keyboard shortcuts. MainWindow creates
/// KeyboardAccelerator instances from entries that have Bindings;
/// SettingsPage builds its display table from All.
/// </summary>
public static class ShortcutDefinitions
{
    /// <summary>
    /// A key + modifier pair that can create a KeyboardAccelerator.
    /// </summary>
    public record KeyBinding(VirtualKey Key, VirtualKeyModifiers Modifiers);

    /// <summary>
    /// A shortcut entry. <paramref name="Bindings"/> is non-empty for global
    /// accelerators (registered by MainWindow) and empty for contextual
    /// shortcuts (handled by individual pages/controls).
    /// </summary>
    public record ShortcutEntry(string ActionName, string DisplayKeys, KeyBinding[] Bindings);

    public static readonly ShortcutEntry Undo = new(
        "Undo", "Ctrl+Z",
        [new(VirtualKey.Z, VirtualKeyModifiers.Control)]);

    public static readonly ShortcutEntry Redo = new(
        "Redo", "Ctrl+Y / Ctrl+Shift+Z",
        [new(VirtualKey.Y, VirtualKeyModifiers.Control),
         new(VirtualKey.Z, VirtualKeyModifiers.Control | VirtualKeyModifiers.Shift)]);

    public static readonly ShortcutEntry OpenDeletionDialog = new(
        "Open deletion dialog", "Ctrl+D",
        [new(VirtualKey.D, VirtualKeyModifiers.Control)]);

    public static readonly ShortcutEntry FocusSearch = new(
        "Focus search", "Ctrl+F", []);

    public static readonly ShortcutEntry RefreshCurrentView = new(
        "Refresh current view", "F5",
        [new(VirtualKey.F5, VirtualKeyModifiers.None)]);

    public static readonly ShortcutEntry NavigateFileList = new(
        "Navigate file list", "\u2191 / \u2193", []);

    public static readonly ShortcutEntry KeepSelectedCopy = new(
        "Keep selected copy", "K", []);

    public static readonly ShortcutEntry DeleteSelectedCopy = new(
        "Delete selected copy", "D", []);

    public static readonly ShortcutEntry SkipSelectedCopy = new(
        "Skip selected copy", "S", []);

    public static readonly ShortcutEntry SelectCopyByOrdinal = new(
        "Select copy by ordinal", "1\u20139", []);

    public static readonly ShortcutEntry RevealInExplorer = new(
        "Reveal in Explorer", "Ctrl+E", []);

    public static readonly ShortcutEntry OpenFile = new(
        "Open file", "Ctrl+O", []);

    public static readonly ShortcutEntry MarkForDeletion = new(
        "Mark for deletion", "Del", []);

    /// <summary>
    /// All shortcuts in display order. SettingsPage reads this list.
    /// </summary>
    public static readonly IReadOnlyList<ShortcutEntry> All =
    [
        Undo,
        Redo,
        OpenDeletionDialog,
        FocusSearch,
        RefreshCurrentView,
        NavigateFileList,
        KeepSelectedCopy,
        DeleteSelectedCopy,
        SkipSelectedCopy,
        SelectCopyByOrdinal,
        RevealInExplorer,
        OpenFile,
        MarkForDeletion,
    ];
}
