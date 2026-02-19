using Microsoft.UI.Xaml.Data;
using SuperDuper.Models;
using SuperDuper.Services;

namespace SuperDuper.Converters;

/// <summary>
/// Converts a long (bytes) to a human-readable size string.
/// Respects SizeDisplayMode (Decimal GB vs Binary GiB) via SettingsService.
/// </summary>
public class FileSizeConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        if (value is not long bytes) return "—";
        return FormatBytes(bytes);
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotImplementedException();

    public static string FormatBytes(long bytes, SizeDisplayMode mode = SizeDisplayMode.Decimal)
    {
        if (bytes < 0) return "—";
        if (mode == SizeDisplayMode.Binary)
        {
            string[] sizes = { "B", "KiB", "MiB", "GiB", "TiB" };
            double len = bytes;
            int order = 0;
            while (len >= 1024 && order < sizes.Length - 1) { order++; len /= 1024; }
            return $"{len:0.#} {sizes[order]}";
        }
        else
        {
            if (bytes < 1000) return $"{bytes} B";
            if (bytes < 1_000_000) return $"{bytes / 1000.0:0.#} KB";
            if (bytes < 1_000_000_000) return $"{bytes / 1_000_000.0:0.#} MB";
            if (bytes < 1_000_000_000_000L) return $"{bytes / 1_000_000_000.0:0.##} GB";
            return $"{bytes / 1_000_000_000_000.0:0.##} TB";
        }
    }
}
