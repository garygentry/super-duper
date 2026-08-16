namespace SuperDuper.Windows.Core.Services;

public interface IExplorerService
{
    Task RevealAsync(string path, CancellationToken cancellationToken = default);
}
