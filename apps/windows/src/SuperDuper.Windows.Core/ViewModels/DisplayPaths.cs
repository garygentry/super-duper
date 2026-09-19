namespace SuperDuper.Windows.Core.ViewModels;

/// <summary>
/// Display-only path spelling. The worker stores and returns Windows paths in verbatim form
/// (<c>\\?\C:\...</c>, <c>\\?\UNC\server\share\...</c>); users should see and copy the plain form.
/// Never use the result as an identity value: worker requests, review decisions, filter keys,
/// comparisons, Explorer reveal and watcher roots keep the exact stored string.
/// </summary>
public static class DisplayPaths
{
    private const string VerbatimPrefix = @"\\?\";
    private const string VerbatimUncPrefix = @"\\?\UNC\";

    /// <summary>
    /// Returns <paramref name="path"/> without a verbatim drive or UNC prefix. Other verbatim forms
    /// (volume GUID, GLOBALROOT) have no plain spelling and are returned unchanged. Null returns an
    /// empty string.
    /// </summary>
    public static string Plain(string? path)
    {
        if (string.IsNullOrEmpty(path)) return string.Empty;
        if (path.StartsWith(VerbatimUncPrefix, StringComparison.OrdinalIgnoreCase))
            return @"\\" + path[VerbatimUncPrefix.Length..];
        if (path.Length >= VerbatimPrefix.Length + 2
            && path.StartsWith(VerbatimPrefix, StringComparison.Ordinal)
            && char.IsAsciiLetter(path[VerbatimPrefix.Length])
            && path[VerbatimPrefix.Length + 1] == ':'
            && (path.Length == VerbatimPrefix.Length + 2 || path[VerbatimPrefix.Length + 2] == '\\'))
            return path[VerbatimPrefix.Length..];
        return path;
    }
}
