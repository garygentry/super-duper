using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Core.Validation;

public enum ScanRootKind
{
    Fixed,
    Removable,
    MappedNetwork,
    UncNetwork,
    Other,
}

public sealed record SessionValidationResult(
    IReadOnlyList<string> Roots,
    IReadOnlyList<string> IgnorePatterns,
    IReadOnlyList<string> Warnings,
    IReadOnlyList<string> Errors,
    bool HasReachableRoot)
{
    public bool IsValid => Errors.Count == 0;
}

/// <summary>
/// A root's drive classification and availability, from <see cref="SessionDefinitionValidator.EvaluateRootAvailability"/>.
/// Producing this touches the file system and, for a removable or slow drive, can block; see
/// <see cref="SessionDefinitionValidator.ProbeRootAvailabilityAsync"/> for the bounded, off-thread way to get it.
/// </summary>
public sealed record RootAvailability(ScanRootKind Kind, bool Reachable, string? Warning);

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

    /// <summary>
    /// Full validation: syntax plus each surviving root's drive classification and availability.
    /// Touches the file system (<see cref="EvaluateRootAvailability"/>) and can block on a
    /// removable, network, or otherwise slow drive, so callers on the WPF dispatcher must not call
    /// this directly — use <see cref="ValidateSyntax"/> for immediate feedback and
    /// <see cref="ProbeRootAvailabilityAsync"/>, debounced, off the dispatcher.
    /// </summary>
    public static SessionValidationResult Validate(
        string name,
        IEnumerable<string> roots,
        IEnumerable<string> ignorePatterns,
        IEnumerable<string> otherSessionNames)
    {
        var syntax = ValidateSyntax(name, roots, ignorePatterns, otherSessionNames);
        var warnings = new List<string>(syntax.Warnings);
        var hasReachableRoot = false;
        foreach (var path in syntax.Roots)
        {
            var availability = EvaluateRootAvailability(path);
            hasReachableRoot |= availability.Reachable;
            if (availability.Warning is { } warning)
            {
                warnings.Add(warning);
            }
        }
        return syntax with { Warnings = warnings, HasReachableRoot = hasReachableRoot };
    }

    /// <summary>
    /// Pure, synchronous validation: name, path syntax, duplicate and nested-root collapsing, and
    /// ignore patterns. Touches no file, drive or network, so it is safe to call on the WPF
    /// dispatcher on every keystroke. <see cref="SessionValidationResult.HasReachableRoot"/> is
    /// optimistic (<c>true</c> whenever there is at least one syntactically valid root) until a
    /// <see cref="ProbeRootAvailabilityAsync"/> result narrows it.
    /// </summary>
    public static SessionValidationResult ValidateSyntax(
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
            errors.Add("Enter a saved scan name.");
        }
        else if (trimmedName.Length > MaximumNameLength)
        {
            errors.Add($"Saved scan names may contain at most {MaximumNameLength} characters.");
        }
        else if (otherSessionNames.Any(existing =>
                     string.Equals(existing.Trim(), trimmedName, StringComparison.OrdinalIgnoreCase)))
        {
            errors.Add("Another saved scan already uses this name.");
        }

        var normalizedRoots = NormalizeRootsSyntax(roots, errors, warnings);
        var patterns = NormalizeIgnorePatterns(ignorePatterns, errors);
        return new SessionValidationResult(
            normalizedRoots,
            patterns,
            warnings,
            errors,
            HasReachableRoot: normalizedRoots.Count > 0);
    }

    /// <summary>
    /// Classifies a root's drive type and checks whether it currently exists. Blocks the calling
    /// thread on a removable, network, or otherwise slow drive; never call this from the WPF
    /// dispatcher (see <see cref="ProbeRootAvailabilityAsync"/>).
    /// </summary>
    public static RootAvailability EvaluateRootAvailability(string fullPath)
    {
        var kind = ClassifyRoot(fullPath);
        // Network reachability can block for the SMB timeout and is authoritatively checked by
        // run.start in the worker instead.
        var reachable = kind is ScanRootKind.MappedNetwork or ScanRootKind.UncNetwork
            || Directory.Exists(fullPath);
        var warning = LocationWarning(fullPath, kind, reachable)
            ?? (reachable ? null : $"Root is currently unavailable: {DisplayPaths.Plain(fullPath)}");
        return new RootAvailability(kind, reachable, warning);
    }

    /// <summary>
    /// Evaluates each root's availability off the calling thread, bounded to <paramref name="timeout"/>
    /// in total. A root whose probe has not finished by the deadline is left out of the result
    /// (its thread-pool work item runs to completion in the background and is discarded): the
    /// operating system, not this method, decides when a stuck removable or network probe gives up.
    /// </summary>
    public static Task<IReadOnlyDictionary<string, RootAvailability>> ProbeRootAvailabilityAsync(
        IReadOnlyList<string> roots,
        TimeSpan timeout,
        CancellationToken cancellationToken = default) =>
        ProbeRootAvailabilityAsync(roots, timeout, EvaluateRootAvailability, cancellationToken);

    // The probe delegate is overridable so tests can simulate a stuck removable or network drive
    // without an actual one; production always uses EvaluateRootAvailability.
    internal static async Task<IReadOnlyDictionary<string, RootAvailability>> ProbeRootAvailabilityAsync(
        IReadOnlyList<string> roots,
        TimeSpan timeout,
        Func<string, RootAvailability> probe,
        CancellationToken cancellationToken = default)
    {
        var distinctRoots = roots.Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
        if (distinctRoots.Length == 0)
        {
            return new Dictionary<string, RootAvailability>(StringComparer.OrdinalIgnoreCase);
        }

        var probes = distinctRoots.ToDictionary(
            root => root,
            root => Task.Run(() => probe(root), cancellationToken),
            StringComparer.OrdinalIgnoreCase);

        await Task.WhenAny(Task.WhenAll(probes.Values), Task.Delay(timeout, cancellationToken))
            .ConfigureAwait(false);

        var results = new Dictionary<string, RootAvailability>(StringComparer.OrdinalIgnoreCase);
        foreach (var (root, probeTask) in probes)
        {
            if (probeTask.IsCompletedSuccessfully)
            {
                results[root] = probeTask.Result;
            }
        }
        return results;
    }

    public static IReadOnlyList<string> NormalizeRootsSyntax(
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
            errors?.Add($"A saved scan may contain at most {MaximumRoots} scan roots.");
        }

        var absolute = new List<string>();
        foreach (var candidate in candidates)
        {
            try
            {
                if (!Path.IsPathFullyQualified(candidate))
                {
                    errors?.Add($"Scan root must be an absolute path: {DisplayPaths.Plain(candidate)}");
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
                    warnings?.Add($"{DisplayPaths.Plain(fullPath)} scans an entire drive and may take a long time.");
                }
                if (!absolute.Contains(fullPath, StringComparer.OrdinalIgnoreCase))
                {
                    absolute.Add(fullPath);
                }
            }
            catch (Exception exception) when (
                exception is ArgumentException or NotSupportedException or PathTooLongException)
            {
                errors?.Add($"Scan root is not a valid Windows path: {DisplayPaths.Plain(candidate)}");
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
                warnings?.Add($"Removed nested root {DisplayPaths.Plain(candidate)}; it is already covered by {DisplayPaths.Plain(parent)}.");
                continue;
            }
            result.Add(candidate);
        }
        return result;
    }

    public static ScanRootKind ClassifyRoot(string fullPath)
    {
        if (fullPath.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase)
            || (fullPath.StartsWith(@"\\", StringComparison.Ordinal)
                && !fullPath.StartsWith(@"\\?\", StringComparison.Ordinal)))
        {
            return ScanRootKind.UncNetwork;
        }

        try
        {
            var root = Path.GetPathRoot(fullPath);
            if (string.IsNullOrWhiteSpace(root))
            {
                return ScanRootKind.Other;
            }
            // The worker persists canonical extended paths. DriveInfo expects the ordinary
            // drive root; normalize only its lookup, preserving the original scan path.
            if (root.Length == 7 && root.StartsWith(@"\\?\", StringComparison.Ordinal)
                && char.IsAsciiLetter(root[4]) && root[5] == ':' && root[6] == '\\')
            {
                root = root[4..];
            }
            return new DriveInfo(root).DriveType switch
            {
                DriveType.Fixed => ScanRootKind.Fixed,
                DriveType.Removable => ScanRootKind.Removable,
                DriveType.Network => ScanRootKind.MappedNetwork,
                _ => ScanRootKind.Other,
            };
        }
        catch (Exception exception) when (exception is ArgumentException or IOException or UnauthorizedAccessException)
        {
            return ScanRootKind.Other;
        }
    }

    // Messages name the plain spelling; the classified path itself is not rewritten.
    public static string? LocationWarning(string path, ScanRootKind kind, bool reachable) =>
        LocationWarningText(DisplayPaths.Plain(path), kind, reachable);

    private static string? LocationWarningText(string path, ScanRootKind kind, bool reachable) => kind switch
    {
        ScanRootKind.Removable =>
            $"Removable root availability may change during a scan; disconnects are reported as warnings: {path}",
        ScanRootKind.MappedNetwork =>
            $"Mapped network root is best-effort and uses the worker process account's drive mapping: {path}",
        ScanRootKind.UncNetwork =>
            $"UNC network root is best-effort; latency, credentials, and disconnects may produce warnings: {path}",
        ScanRootKind.Other when reachable =>
            $"Root uses a filesystem type that has not been classified as fixed, removable, or network: {path}",
        _ => null,
    };

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
            errors?.Add($"A saved scan may contain at most {MaximumIgnorePatterns} ignore patterns.");
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
