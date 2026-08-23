using System.Diagnostics;
using SuperDuper.Windows.Core.Services;

namespace SuperDuper.Windows.Infrastructure;

public sealed class WindowsRecycleBinService : IRecycleBinService
{
    private readonly Action<ProcessStartInfo> _start;

    public WindowsRecycleBinService()
        : this(startInfo => Process.Start(startInfo))
    {
    }

    internal WindowsRecycleBinService(Action<ProcessStartInfo> start)
    {
        _start = start;
    }

    public Task OpenAsync(CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        try
        {
            _start(new ProcessStartInfo
            {
                FileName = "explorer.exe",
                Arguments = "shell:RecycleBinFolder",
                UseShellExecute = true,
            });
            return Task.CompletedTask;
        }
        catch (Exception exception)
        {
            throw new InvalidOperationException(
                $"Windows could not open the Recycle Bin. {exception.Message}",
                exception);
        }
    }
}
