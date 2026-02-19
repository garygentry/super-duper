using Microsoft.UI.Xaml.Data;
using SuperDuper.Models;

namespace SuperDuper.Converters;

/// <summary>
/// Converts a DensityLevel enum to a brush resource key (Bg or Fg variant).
/// Parameter: "Bg" for background, "Fg" for foreground (default).
/// </summary>
public class DensityToColorConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        var variant = parameter as string ?? "Fg";
        if (value is DensityLevel level)
        {
            return level switch
            {
                DensityLevel.Low    => $"DensityLow{variant}",
                DensityLevel.Medium => $"DensityMed{variant}",
                DensityLevel.High   => $"DensityHigh{variant}",
                _                   => $"DensityLow{variant}"
            };
        }
        return $"DensityLow{variant}";
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotImplementedException();
}

/// <summary>Computes DensityLevel from a (dupeCount, totalCount) tuple.</summary>
public static class DensityCalculator
{
    public static DensityLevel Calculate(int dupeCount, int totalCount)
    {
        if (totalCount == 0) return DensityLevel.Low;
        var ratio = (double)dupeCount / totalCount;
        if (ratio >= 0.5) return DensityLevel.High;
        if (ratio >= 0.2) return DensityLevel.Medium;
        return DensityLevel.Low;
    }
}

/// <summary>Converts bool to Visibility.</summary>
public class BoolToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        var invert = parameter as string == "Invert";
        var boolVal = value is bool b && b;
        if (invert) boolVal = !boolVal;
        return boolVal ? Microsoft.UI.Xaml.Visibility.Visible : Microsoft.UI.Xaml.Visibility.Collapsed;
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotImplementedException();
}

/// <summary>Converts null to Visibility.Collapsed (visible if not null).</summary>
public class NullToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        return value is null
            ? Microsoft.UI.Xaml.Visibility.Collapsed
            : Microsoft.UI.Xaml.Visibility.Visible;
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotImplementedException();
}
