using System.Text.Json;

namespace SuperDuper.Windows.Infrastructure;

public class WorkerProtocolException : Exception
{
    public WorkerProtocolException(string message)
        : base(message)
    {
    }

    public WorkerProtocolException(string message, Exception innerException)
        : base(message, innerException)
    {
    }

    internal WorkerProtocolException(string code, string message, bool retryable, JsonElement details)
        : base($"{code}: {message}")
    {
        Code = code;
        Retryable = retryable;
        Details = details;
    }

    public string? Code { get; }

    public bool Retryable { get; }

    public JsonElement Details { get; }
}
