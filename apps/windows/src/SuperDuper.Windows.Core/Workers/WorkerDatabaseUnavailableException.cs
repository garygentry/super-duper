namespace SuperDuper.Windows.Core.Workers;

/// <summary>
/// The worker started but could not open (or does not own) its database. The reason codes match
/// the worker's <c>database_unavailable</c> error details.
/// </summary>
public sealed class WorkerDatabaseUnavailableException(
    string reason,
    string databasePath,
    string workerMessage,
    Exception? innerException = null)
    : Exception($"The worker could not open its database ({reason}): {workerMessage}", innerException)
{
    public const string InUse = "in_use";
    public const string NewerVersion = "newer_version";
    public const string UnsupportedVersion = "unsupported_version";
    public const string Damaged = "damaged";
    public const string ReadOnly = "read_only";
    public const string Unavailable = "unavailable";
    public const string DiskFull = "disk_full";

    public string Reason { get; } = reason;

    public string DatabasePath { get; } = databasePath;

    public string WorkerMessage { get; } = workerMessage;

    /// <summary>A title and the next step for a person, naming the file involved.</summary>
    public (string Title, string Detail) Describe()
    {
        var (title, guidance) = Reason switch
        {
            InUse => ("Super Duper is already open",
                "Another Super Duper window is using this data. Switch to that window, or close it and choose Reconnect."),
            NewerVersion => ("Saved data is from a newer version",
                "This data was created by a newer version of Super Duper, so this version can't open it. Use the newer version. Nothing was changed."),
            UnsupportedVersion => ("Saved data can't be upgraded",
                "This data is from an early development version that can't be upgraded. Nothing was changed. To start fresh, close Super Duper and move the file aside."),
            Damaged => ("Saved data can't be read",
                "The database appears to be damaged. Nothing was changed. To start with an empty history, close Super Duper, move this file and any -wal and -shm files beside it to a safe place, then reopen."),
            ReadOnly => ("Saved data can't be written",
                "Super Duper can't write to its data. Check that the file and its folder aren't read-only and that you have permission, then choose Reconnect."),
            Unavailable => ("Saved data location is unavailable",
                "Super Duper couldn't open its data. Check that the folder exists and is reachable, then choose Reconnect."),
            DiskFull => ("Disk is full",
                "There isn't enough free space to open Super Duper's data. Free some space, then choose Reconnect."),
            _ => ("Saved data couldn't be opened", WorkerMessage),
        };
        return (title, $"{guidance} Data: {DatabasePath}");
    }
}
