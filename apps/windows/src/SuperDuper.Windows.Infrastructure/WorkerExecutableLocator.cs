namespace SuperDuper.Windows.Infrastructure;

public static class WorkerExecutableLocator
{
    private const string EnvironmentVariableName = "SUPER_DUPER_WORKER_PATH";
    private const string ExecutableName = "super-duper-worker.exe";

    public static string Resolve()
    {
        var configuredPath = Environment.GetEnvironmentVariable(EnvironmentVariableName);
        if (!string.IsNullOrWhiteSpace(configuredPath))
        {
            return Path.GetFullPath(configuredPath);
        }

        var deployedPath = Path.Combine(AppContext.BaseDirectory, ExecutableName);
        if (File.Exists(deployedPath))
        {
            return deployedPath;
        }

        foreach (var startPath in new[] { AppContext.BaseDirectory, Environment.CurrentDirectory })
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
