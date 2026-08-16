namespace SuperDuper.Windows.Infrastructure;

internal static class WindowsShellPath
{
    private const string ExtendedPrefix = @"\\?\";
    private const string ExtendedUncPrefix = @"\\?\UNC\";

    public static string ToParsingPath(string path)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);

        if (path.StartsWith(ExtendedUncPrefix, StringComparison.OrdinalIgnoreCase))
        {
            return @"\\" + path[ExtendedUncPrefix.Length..];
        }

        if (path.StartsWith(ExtendedPrefix, StringComparison.OrdinalIgnoreCase)
            && path.Length >= ExtendedPrefix.Length + 3
            && char.IsAsciiLetter(path[ExtendedPrefix.Length])
            && path[ExtendedPrefix.Length + 1] == ':'
            && IsDirectorySeparator(path[ExtendedPrefix.Length + 2]))
        {
            return path[ExtendedPrefix.Length..];
        }

        return path;
    }

    private static bool IsDirectorySeparator(char value) => value is '\\' or '/';
}
