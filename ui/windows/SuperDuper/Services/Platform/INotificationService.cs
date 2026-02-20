namespace SuperDuper.Services.Platform;

public interface INotificationService
{
    /// <summary>Shows a toast when a scan completes (if window is not in foreground).</summary>
    void ShowScanComplete(int groupCount, long wastedBytes);

    /// <summary>Shows a toast when deletion completes.</summary>
    void ShowDeletionComplete(int fileCount, long bytesFreed);
}
