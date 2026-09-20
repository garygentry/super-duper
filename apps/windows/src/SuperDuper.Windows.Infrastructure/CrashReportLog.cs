namespace SuperDuper.Windows.Infrastructure;

/// <summary>
/// Best-effort, synchronous, one-shot logging for a crash report, for when no
/// <see cref="BoundedDiagnosticLog"/> is open to own the write — <see cref="WorkerClient"/> falls
/// back to this only when it has no live connection (so no other writer can be using the file at
/// the same time; see <see cref="WorkerClient.LogDiagnosticAsync"/>, the preferred path whenever a
/// connection exists). Two independent writers open at once WOULD corrupt each other:
/// <see cref="FileMode.Append"/> seeks to end once, at open time, per handle, not atomically per
/// write, so this must never run concurrently with another writer on the same path.
/// </summary>
public static class CrashReportLog
{
    public static bool TryAppend(string path, string source, Exception exception)
    {
        try
        {
            var fullPath = Path.GetFullPath(path);
            var directory = Path.GetDirectoryName(fullPath)
                ?? throw new IOException("The diagnostic log has no parent directory.");
            Directory.CreateDirectory(directory);
            if (File.Exists(fullPath)
                && new FileInfo(fullPath).Length >= BoundedDiagnosticLog.DefaultMaximumBytes)
            {
                File.Move(fullPath, fullPath + ".previous", overwrite: true);
            }

            // FileShare.ReadWrite only so a concurrent reader (for example a person tailing the
            // log) is never blocked; it does not make a concurrent second writer safe.
            using var stream = new FileStream(
                fullPath, FileMode.Append, FileAccess.Write, FileShare.ReadWrite);
            using var writer = new StreamWriter(stream);
            writer.WriteLine($"{DateTimeOffset.UtcNow:O} [{source}] {exception}");
            return true;
        }
        catch (Exception ex) when (
            ex is IOException
                or UnauthorizedAccessException
                or NotSupportedException
                or ArgumentException)
        {
            return false;
        }
    }
}
