using Microsoft.UI.Xaml.Data;
using SuperDuper.Models;

namespace SuperDuper.Converters;

/// <summary>Converts ReviewStatus enum to a Segoe MDL2 icon glyph.</summary>
public class ReviewStatusToIconConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        if (value is ReviewStatus status)
        {
            return status switch
            {
                ReviewStatus.Unreviewed => "\uE7BA",  // Radio button empty
                ReviewStatus.Partial    => "\uE73A",  // Half circle
                ReviewStatus.Decided   => "\uE73E",  // Checkbox checked (filled circle)
                _                       => "\uE7BA"
            };
        }
        return "\uE7BA";
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotImplementedException();
}

/// <summary>Converts ReviewAction to a color brush key for lookup in resources.</summary>
public class ReviewActionToColorConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        if (value is ReviewAction action)
        {
            return action switch
            {
                ReviewAction.Keep   => "KeepBrush",
                ReviewAction.Delete => "DeleteBrush",
                ReviewAction.Skip   => "SkipBrush",
                _                   => "SkipBrush"
            };
        }
        return "SkipBrush";
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotImplementedException();
}
