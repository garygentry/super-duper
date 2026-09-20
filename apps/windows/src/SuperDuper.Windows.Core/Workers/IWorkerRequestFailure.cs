namespace SuperDuper.Windows.Core.Workers;

/// <summary>
/// Exposes a worker request failure's protocol error code to Core. Infrastructure's
/// <c>WorkerProtocolException</c> implements this so a view model can switch on <see cref="Code"/>
/// instead of searching <see cref="Exception.Message"/> for code strings, without Core referencing
/// Infrastructure.
/// </summary>
public interface IWorkerRequestFailure
{
    /// <summary>The worker's protocol error code, or <see langword="null"/> for a failure that
    /// never reached a structured worker response (a connection or protocol-framing failure).</summary>
    string? Code { get; }
}
