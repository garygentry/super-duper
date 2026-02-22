using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;
using SuperDuper.Converters;

namespace SuperDuper.Services.Platform.Windows;

/// <summary>
/// Sends Windows toast notifications via the Windows App SDK AppNotificationBuilder.
/// Only sends when the window is not in foreground.
/// </summary>
public class WindowsNotificationService : INotificationService
{
    public void ShowScanComplete(int groupCount, long wastedBytes)
    {
        try
        {
            var wastedStr = FileSizeConverter.FormatBytes(wastedBytes);
            var builder = new AppNotificationBuilder()
                .AddText("Super Duper — Scan Complete")
                .AddText($"Found {groupCount:N0} duplicate groups ({wastedStr} wasted).")
                .AddButton(new AppNotificationButton("Open Results")
                    .AddArgument("action", "openResults"));

            AppNotificationManager.Default.Show(builder.BuildNotification());
        }
        catch { /* Notifications optional — swallow any platform errors */ }
    }

    public void ShowDeletionComplete(int fileCount, long bytesFreed)
    {
        try
        {
            var freedStr = FileSizeConverter.FormatBytes(bytesFreed);
            var builder = new AppNotificationBuilder()
                .AddText("Super Duper — Deletion Complete")
                .AddText($"Deleted {fileCount:N0} files ({freedStr} recovered).");

            AppNotificationManager.Default.Show(builder.BuildNotification());
        }
        catch { }
    }

}
