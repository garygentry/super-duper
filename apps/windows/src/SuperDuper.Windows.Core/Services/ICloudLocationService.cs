using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Services;

public sealed record CloudLocationDetectionResult(
    string Status,
    IReadOnlyList<WorkerRegisteredCloudLocation> Locations,
    string? ErrorMessage = null);

public interface ICloudLocationService
{
    Task<CloudLocationDetectionResult> DetectAsync(CancellationToken cancellationToken = default);
}

public sealed class UnavailableCloudLocationService : ICloudLocationService
{
    private readonly string _message;

    public UnavailableCloudLocationService(
        string message = "Registered cloud location detection is unavailable.")
    {
        _message = message;
    }

    public Task<CloudLocationDetectionResult> DetectAsync(CancellationToken cancellationToken = default) =>
        Task.FromResult(new CloudLocationDetectionResult(
            CloudDetectionStatusNames.Unavailable,
            [],
            _message));
}
