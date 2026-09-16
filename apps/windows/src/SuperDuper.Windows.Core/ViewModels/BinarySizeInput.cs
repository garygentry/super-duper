using System.Globalization;
using System.Numerics;

namespace SuperDuper.Windows.Core.ViewModels;

/// <summary>Converts a binary-unit editor value to an exact signed 64-bit byte count.</summary>
internal static class BinarySizeInput
{
    public static IReadOnlyList<string> Units { get; } = ["B", "KiB", "MiB", "GiB", "TiB"];

    public static bool TryConvertToBytes(string text, string unit, out long bytes)
    {
        bytes = 0;
        var multiplier = unit switch
        {
            "B" => 1L,
            "KiB" => 1024L,
            "MiB" => 1_048_576L,
            "GiB" => 1_073_741_824L,
            "TiB" => 1_099_511_627_776L,
            _ => 0L,
        };

        // Rational arithmetic avoids decimal/double rounding, including one byte in TiB.
        if (text.Length is 0 or > 256 || multiplier == 0)
        {
            return false;
        }

        var parts = text.Split('.');
        if (parts.Length > 2 || parts.Any(part => part.Length == 0 || part.Any(c => c is < '0' or > '9')))
        {
            return false;
        }

        var numerator = BigInteger.Parse(string.Concat(parts), CultureInfo.InvariantCulture) * multiplier;
        var denominator = BigInteger.Pow(10, parts.Length == 2 ? parts[1].Length : 0);
        var whole = BigInteger.DivRem(numerator, denominator, out var remainder);
        if (!remainder.IsZero || whole > long.MaxValue)
        {
            return false;
        }

        bytes = (long)whole;
        return true;
    }
}
