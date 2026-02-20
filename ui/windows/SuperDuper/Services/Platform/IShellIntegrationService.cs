using SuperDuper.NativeMethods;

namespace SuperDuper.Services.Platform;

public interface IShellIntegrationService
{
    /// <summary>Opens File Explorer with the specified path selected.</summary>
    void RevealInExplorer(string path);

    /// <summary>Opens the file with its default application.</summary>
    void OpenFile(string path);

    /// <summary>Registers the "Scan with Super Duper" context menu entry in HKCU.</summary>
    bool RegisterContextMenu();

    /// <summary>Removes the context menu registry entry.</summary>
    void UnregisterContextMenu();

    /// <summary>Updates the taskbar jump list with recent sessions.</summary>
    void UpdateJumpList(IReadOnlyList<SessionInfo> recentSessions);
}
