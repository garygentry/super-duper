using Microsoft.Win32;
using System.Diagnostics;
using System.Reflection;

namespace SuperDuper.Services.Platform.Windows;

/// <summary>
/// Windows implementation of shell integration using Process.Start and registry.
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
        catch (Exception ex) { Debug.WriteLine($"[WindowsShellService.RevealInExplorer] {ex}"); }
    }

    public void OpenFile(string path)
    {
        if (!File.Exists(path)) return;
        try
        {
            Process.Start(new ProcessStartInfo(path) { UseShellExecute = true });
        }
        catch (Exception ex) { Debug.WriteLine($"[WindowsShellService.OpenFile] {ex}"); }
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
        catch (Exception ex)
        {
            Debug.WriteLine($"[WindowsShellService.RegisterContextMenu] {ex}");
            return false;
        }
    }

    public void UnregisterContextMenu()
    {
        try
        {
            Registry.CurrentUser.DeleteSubKeyTree(ContextMenuKeyPath, throwOnMissingSubKey: false);
        }
        catch (Exception ex) { Debug.WriteLine($"[WindowsShellService.UnregisterContextMenu] {ex}"); }
    }
}
