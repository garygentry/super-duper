namespace SuperDuper.Windows.Infrastructure;

public sealed class WorkerConnectionException : Exception
{
    public WorkerConnectionException(string executablePath, string message, Exception? innerException = null)
        : base($"Unable to connect to worker at '{executablePath}'. {message}", innerException)
    {
        ExecutablePath = executablePath;
    }

    public string ExecutablePath { get; }
}
