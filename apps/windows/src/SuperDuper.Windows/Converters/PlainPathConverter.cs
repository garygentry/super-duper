using System.Globalization;
using System.Windows.Data;
using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Converters;

/// <summary>
/// Shows a stored path in its plain spelling (<c>C:\...</c>, <c>\\server\share\...</c>) instead of
/// the worker's verbatim form. Display only: <see cref="ConvertBack"/> returns the value unchanged so
/// a two-way binding never rewrites a stored path the user did not edit.
/// </summary>
[ValueConversion(typeof(string), typeof(string))]
public sealed class PlainPathConverter : IValueConverter
{
    public object? Convert(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        value is string path ? DisplayPaths.Plain(path) : value;

    public object? ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture) =>
        value;
}
