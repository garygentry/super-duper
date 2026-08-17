using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;
using Windows.Storage.Provider;

namespace SuperDuper.Windows.Infrastructure;

public sealed class WindowsCloudLocationService : ICloudLocationService
{
    private readonly IStorageProviderSyncRootSource _source;

    public WindowsCloudLocationService()
        : this(new WindowsStorageProviderSyncRootSource())
    {
    }

    internal WindowsCloudLocationService(IStorageProviderSyncRootSource source)
    {
        _source = source;
    }

    public Task<CloudLocationDetectionResult> DetectAsync(CancellationToken cancellationToken = default) =>
        Task.Run(() => Detect(cancellationToken), cancellationToken);

    private CloudLocationDetectionResult Detect(CancellationToken cancellationToken)
    {
        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (!_source.IsSupported())
            {
                return new CloudLocationDetectionResult(CloudDetectionStatusNames.Unsupported, []);
            }

            var locations = new List<WorkerRegisteredCloudLocation>();
            foreach (var registration in _source.GetCurrentSyncRoots())
            {
                cancellationToken.ThrowIfCancellationRequested();
                var path = registration.Path;
                if (string.IsNullOrWhiteSpace(path))
                {
                    continue;
                }

                var location = CreateLocation(path, registration.Id);
                if (!locations.Any(existing =>
                    string.Equals(existing.Path, path, StringComparison.OrdinalIgnoreCase)))
                {
                    locations.Add(location);
                }
            }

            return new CloudLocationDetectionResult(
                CloudDetectionStatusNames.Complete,
                locations.OrderBy(location => location.Path, StringComparer.OrdinalIgnoreCase).ToArray());
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception exception)
        {
            return new CloudLocationDetectionResult(
                CloudDetectionStatusNames.Unavailable,
                [],
                $"Windows could not enumerate registered cloud locations: {exception.Message}");
        }
    }

    internal static WorkerRegisteredCloudLocation CreateLocation(string path, string? registrationId)
    {
        var providerId = registrationId ?? "";
        var separator = providerId.IndexOf('!');
        var displayName = separator > 0 ? providerId[..separator] : providerId;
        if (string.IsNullOrWhiteSpace(displayName))
        {
            displayName = "Cloud provider";
        }
        return new WorkerRegisteredCloudLocation(path, providerId, displayName);
    }
}

internal sealed record StorageProviderSyncRootRegistration(string? Path, string? Id);

internal interface IStorageProviderSyncRootSource
{
    bool IsSupported();

    IReadOnlyList<StorageProviderSyncRootRegistration> GetCurrentSyncRoots();
}

internal sealed class WindowsStorageProviderSyncRootSource : IStorageProviderSyncRootSource
{
    public bool IsSupported() => StorageProviderSyncRootManager.IsSupported();

    public IReadOnlyList<StorageProviderSyncRootRegistration> GetCurrentSyncRoots() =>
        StorageProviderSyncRootManager.GetCurrentSyncRoots()
            .Select(registration => new StorageProviderSyncRootRegistration(
                registration.Path?.Path,
                registration.Id))
            .ToArray();
}
