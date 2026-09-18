namespace SuperDuper.Windows.Infrastructure;

public static class WorkerExecutableLocator
{
    internal const string EnvironmentVariableName = "SUPER_DUPER_WORKER_PATH";
    internal const string ExecutableName = "super-duper-worker.exe";

    // Release builds launch only the worker deployed beside the app. The environment override and
    // the repository target\debug fallback are development seams and must not choose which
    // executable an installed app starts.
#if DEBUG
    internal static readonly bool DevelopmentOverridesEnabled = true;
#else
    internal static readonly bool DevelopmentOverridesEnabled = false;
#endif

    public static string Resolve() => Resolve(
        AppContext.BaseDirectory,
        Environment.CurrentDirectory,
        Environment.GetEnvironmentVariable(EnvironmentVariableName),
        DevelopmentOverridesEnabled);

    internal static string Resolve(
        string baseDirectory,
        string currentDirectory,
        string? configuredPath,
        bool allowDevelopmentOverrides)
    {
        var deployedPath = Path.Combine(baseDirectory, ExecutableName);
        if (!allowDevelopmentOverrides)
        {
            return deployedPath;
        }

        if (!string.IsNullOrWhiteSpace(configuredPath))
        {
            return Path.GetFullPath(configuredPath);
        }

        if (File.Exists(deployedPath))
        {
            return deployedPath;
        }

        foreach (var startPath in new[] { baseDirectory, currentDirectory })
        {
            var repositoryPath = FindRepositoryRoot(startPath);
            if (repositoryPath is not null)
            {
                var developmentPath = Path.Combine(repositoryPath, "target", "debug", ExecutableName);
                if (File.Exists(developmentPath))
                {
                    return developmentPath;
                }
            }
        }

        return deployedPath;
    }

    private static string? FindRepositoryRoot(string startPath)
    {
        for (var directory = new DirectoryInfo(startPath); directory is not null; directory = directory.Parent)
        {
            if (File.Exists(Path.Combine(directory.FullName, "Cargo.toml")) &&
                Directory.Exists(Path.Combine(directory.FullName, "crates", "super-duper-worker")))
            {
                return directory.FullName;
            }
        }

        return null;
    }
}
