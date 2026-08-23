namespace SuperDuper.Windows.Core.Services;

public interface IRecycleBinService
{
    Task OpenAsync(CancellationToken cancellationToken = default);
}
