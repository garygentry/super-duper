namespace SuperDuper.Windows.Infrastructure;

/// <summary>
/// Where the app keeps worker state. With no overrides everything lives in
/// %LOCALAPPDATA%\SuperDuper, so state survives upgrades and never lands in the install folder.
/// When <c>SUPER_DUPER_DB_PATH</c> is set, the status database, hash cache and presentation
/// preferences follow that database's folder unless individually overridden, which keeps a
/// disposable run self-contained.
/// </summary>
public sealed record WorkerStateLocations(
    string StateDirectory,
    string DatabasePath,
    string StatusDatabasePath,
    string HashCachePath,
    bool CreateStateDirectory)
{
    internal const string DatabaseVariable = "SUPER_DUPER_DB_PATH";
    internal const string StatusDatabaseVariable = "SUPER_DUPER_STATUS_DB_PATH";
    internal const string HashCacheVariable = "HASH_CACHE_PATH";
    internal const string DatabaseFileName = "super_duper.db";
    internal const string StatusDatabaseFileName = "scan_status.db";
    internal const string HashCacheDirectoryName = "content_hash_cache.db";

    public static WorkerStateLocations FromEnvironment() => Resolve(
        Environment.GetEnvironmentVariable,
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData));

    internal static WorkerStateLocations Resolve(
        Func<string, string?> environment,
        string localApplicationData)
    {
        var configuredDatabase = NonEmpty(environment(DatabaseVariable));
        var databasePath = configuredDatabase is not null
            ? Path.GetFullPath(configuredDatabase)
            : Path.Combine(Path.GetFullPath(localApplicationData), "SuperDuper", DatabaseFileName);
        var stateDirectory = Path.GetDirectoryName(databasePath)
            ?? throw new InvalidOperationException($"Database path has no parent directory: {databasePath}");

        return new WorkerStateLocations(
            stateDirectory,
            databasePath,
            FullPathOrDefault(environment(StatusDatabaseVariable), stateDirectory, StatusDatabaseFileName),
            FullPathOrDefault(environment(HashCacheVariable), stateDirectory, HashCacheDirectoryName),
            CreateStateDirectory: configuredDatabase is null);
    }

    private static string FullPathOrDefault(string? configured, string stateDirectory, string fileName) =>
        NonEmpty(configured) is { } path ? Path.GetFullPath(path) : Path.Combine(stateDirectory, fileName);

    private static string? NonEmpty(string? value) => string.IsNullOrWhiteSpace(value) ? null : value;
}
