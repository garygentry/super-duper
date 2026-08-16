namespace SuperDuper.Windows.Core.Validation;

public sealed record SessionValidationResult(
    IReadOnlyList<string> Roots,
    IReadOnlyList<string> IgnorePatterns,
    IReadOnlyList<string> Warnings,
    IReadOnlyList<string> Errors,
    bool HasReachableRoot)
{
    public bool IsValid => Errors.Count == 0;
}

public static class SessionDefinitionValidator
{
    public const int MaximumNameLength = 200;
    public const int MaximumRoots = 64;
    public const int MaximumIgnorePatterns = 512;
    public const int MaximumIgnorePatternLength = 1024;

    public static readonly IReadOnlyList<string> SafeWindowsIgnorePatterns =
    [
        "**/node_modules/**",
        "*/$RECYCLE.BIN",
        "*/.bzvol",
        "*/System Volume Information",
        "*/Recovery",
    ];

    public static SessionValidationResult Validate(
        string name,
        IEnumerable<string> roots,
        IEnumerable<string> ignorePatterns,
        IEnumerable<string> otherSessionNames)
    {
        var errors = new List<string>();
        var warnings = new List<string>();
        var trimmedName = name.Trim();
        if (trimmedName.Length == 0)
        {
            errors.Add("Enter a session name.");
        }
        else if (trimmedName.Length > MaximumNameLength)
        {
            errors.Add($"Session names may contain at most {MaximumNameLength} characters.");
        }
        else if (otherSessionNames.Any(existing =>
                     string.Equals(existing.Trim(), trimmedName, StringComparison.OrdinalIgnoreCase)))
        {
            errors.Add("Another session already uses this name.");
        }

        var normalizedRoots = NormalizeRoots(roots, errors, warnings);
        var patterns = NormalizeIgnorePatterns(ignorePatterns, errors);
        var hasPotentiallyReachableRoot = normalizedRoots.Any(path =>
            path.StartsWith(@"\\", StringComparison.Ordinal) || Directory.Exists(path));
        return new SessionValidationResult(
            normalizedRoots,
            patterns,
            warnings,
            errors,
            hasPotentiallyReachableRoot);
    }

    public static IReadOnlyList<string> NormalizeRoots(
        IEnumerable<string> roots,
        ICollection<string>? errors = null,
        ICollection<string>? warnings = null)
    {
        var candidates = roots
            .Select(root => root.Trim())
            .Where(root => root.Length > 0)
            .ToList();

        if (candidates.Count == 0)
        {
            errors?.Add("Add at least one scan root.");
            return [];
        }
        if (candidates.Count > MaximumRoots)
        {
            errors?.Add($"A session may contain at most {MaximumRoots} scan roots.");
        }

        var absolute = new List<string>();
        foreach (var candidate in candidates)
        {
            try
            {
                if (!Path.IsPathFullyQualified(candidate))
                {
                    errors?.Add($"Scan root must be an absolute path: {candidate}");
                    continue;
                }

                var fullPath = Path.GetFullPath(candidate);
                var root = Path.GetPathRoot(fullPath);
                if (root is null || !string.Equals(fullPath, root, StringComparison.OrdinalIgnoreCase))
                {
                    fullPath = fullPath.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
                }
                if (root is { Length: 3 }
                    && root[1] == ':'
                    && string.Equals(
                        fullPath.TrimEnd(Path.DirectorySeparatorChar),
                        root?.TrimEnd(Path.DirectorySeparatorChar),
                        StringComparison.OrdinalIgnoreCase))
                {
                    warnings?.Add($"{fullPath} scans an entire drive and may take a long time.");
                }
                if (fullPath.StartsWith(@"\\", StringComparison.Ordinal))
                {
                    warnings?.Add($"Network root availability will be verified before scanning: {fullPath}");
                }
                else if (!Directory.Exists(fullPath))
                {
                    warnings?.Add($"Root is currently unavailable: {fullPath}");
                }
                if (!absolute.Contains(fullPath, StringComparer.OrdinalIgnoreCase))
                {
                    absolute.Add(fullPath);
                }
            }
            catch (Exception exception) when (
                exception is ArgumentException or NotSupportedException or PathTooLongException)
            {
                errors?.Add($"Scan root is not a valid Windows path: {candidate}");
            }
        }

        absolute.Sort((left, right) =>
        {
            var byLength = left.Length.CompareTo(right.Length);
            return byLength != 0 ? byLength : StringComparer.OrdinalIgnoreCase.Compare(left, right);
        });

        var result = new List<string>();
        foreach (var candidate in absolute)
        {
            var parent = result.FirstOrDefault(existing => IsSameOrDescendant(candidate, existing));
            if (parent is not null)
            {
                warnings?.Add($"Removed nested root {candidate}; it is already covered by {parent}.");
                continue;
            }
            result.Add(candidate);
        }
        return result;
    }

    public static IReadOnlyList<string> NormalizeIgnorePatterns(
        IEnumerable<string> ignorePatterns,
        ICollection<string>? errors = null)
    {
        var result = new List<string>();
        foreach (var rawPattern in ignorePatterns)
        {
            var pattern = rawPattern.Trim();
            if (pattern.Length == 0)
            {
                continue;
            }
            if (pattern.Length > MaximumIgnorePatternLength)
            {
                errors?.Add($"Ignore patterns may contain at most {MaximumIgnorePatternLength} characters.");
                continue;
            }
            if (pattern.Contains('\0') || pattern.Contains('\r') || pattern.Contains('\n'))
            {
                errors?.Add("Ignore patterns cannot contain control characters.");
                continue;
            }
            if (!HasBalancedCharacterClass(pattern))
            {
                errors?.Add($"Ignore pattern has an unmatched character class: {pattern}");
                continue;
            }
            if (!result.Contains(pattern, StringComparer.Ordinal))
            {
                result.Add(pattern);
            }
        }
        if (result.Count > MaximumIgnorePatterns)
        {
            errors?.Add($"A session may contain at most {MaximumIgnorePatterns} ignore patterns.");
        }
        return result;
    }

    private static bool IsSameOrDescendant(string candidate, string parent)
    {
        if (string.Equals(candidate, parent, StringComparison.OrdinalIgnoreCase))
        {
            return true;
        }
        var prefix = parent.EndsWith(Path.DirectorySeparatorChar)
            ? parent
            : parent + Path.DirectorySeparatorChar;
        return candidate.StartsWith(prefix, StringComparison.OrdinalIgnoreCase);
    }

    private static bool HasBalancedCharacterClass(string pattern)
    {
        var inClass = false;
        var escaped = false;
        foreach (var character in pattern)
        {
            if (escaped)
            {
                escaped = false;
                continue;
            }
            if (character == '\\')
            {
                escaped = true;
                continue;
            }
            if (character == '[')
            {
                inClass = true;
            }
            else if (character == ']' && inClass)
            {
                inClass = false;
            }
        }
        return !inClass;
    }
}
