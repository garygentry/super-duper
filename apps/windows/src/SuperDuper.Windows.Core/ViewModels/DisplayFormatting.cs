using System.Globalization;

namespace SuperDuper.Windows.Core.ViewModels;

internal static class DisplayFormatting
{
    public static string Bytes(string value)
    {
        if (!decimal.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out var bytes))
        {
            return value;
        }
        string[] units = ["B", "KB", "MB", "GB", "TB", "PB"];
        var unit = 0;
        while (bytes >= 1024 && unit < units.Length - 1)
        {
            bytes /= 1024;
            unit++;
        }
        return $"{bytes:0.##} {units[unit]}";
    }

    public static string Status(string status) => status switch
    {
        "pending" => "Pending",
        "running" => "Scanning",
        "cancelling" => "Cancelling",
        "completed" => "Completed",
        "cancelled" => "Cancelled",
        "failed" => "Failed",
        "interrupted" => "Interrupted",
        _ => status,
    };

    public static string Phase(string? phase) => phase switch
    {
        "discovering" => "Discovering files",
        "hashing" => "Hashing candidates",
        "persisting" => "Saving results",
        "analyzing_folders" => "Analyzing folders",
        "finalizing" => "Finalizing",
        null or "" => "Not started",
        _ => phase.Replace('_', ' '),
    };
}
