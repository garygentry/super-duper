using Microsoft.Win32;
using SuperDuper.NativeMethods;
using System.Diagnostics;
using System.Reflection;

namespace SuperDuper.Services.Platform.Windows;

/// <summary>
/// Windows implementation of shell integration using Process.Start, registry, and WinRT JumpList.
/// </summary>
public class WindowsShellService : IShellIntegrationService
{
    private const string ContextMenuKeyPath =
        @"Software\Classes\Directory\shell\SuperDuper";

    public void RevealInExplorer(string path)
    {
        if (!File.Exists(path) && !Directory.Exists(path)) return;
        try
        {
            Process.Start("explorer.exe", $"/select,\"{path}\"");
        }
        catch { /* swallow — not critical */ }
    }

    public void OpenFile(string path)
    {
        if (!File.Exists(path)) return;
        try
        {
            Process.Start(new ProcessStartInfo(path) { UseShellExecute = true });
        }
        catch { /* swallow */ }
    }

    public bool RegisterContextMenu()
    {
        try
        {
            var exePath = Assembly.GetExecutingAssembly().Location;
            // Replace .dll extension with .exe for self-contained publish
            exePath = Path.ChangeExtension(exePath, ".exe");

            using var key = Registry.CurrentUser.CreateSubKey(ContextMenuKeyPath);
            key.SetValue("", "Scan for duplicates with Super Duper");
            key.SetValue("Icon", exePath);

            using var cmdKey = key.CreateSubKey("command");
            cmdKey.SetValue("", $"\"{exePath}\" --scan-path \"%1\"");

            return true;
        }
        catch
        {
            return false;
        }
    }

    public void UnregisterContextMenu()
    {
        try
        {
            Registry.CurrentUser.DeleteSubKeyTree(ContextMenuKeyPath, throwOnMissingSubKey: false);
        }
        catch { /* swallow */ }
    }

    public void UpdateJumpList(IReadOnlyList<SessionInfo> recentSessions)
    {
        // JumpList update requires Windows packaging context for full WinRT support.
        // In unpackaged mode (WindowsPackageType=None), we use the Windows Shell API via
        // ICustomDestinationList COM interface. For now, this is a no-op placeholder
        // to keep ViewModels decoupled from platform capabilities.
        // TODO: Implement via SHAddToRecentDocs or ICustomDestinationList when targeting packaged mode.
    }
}
