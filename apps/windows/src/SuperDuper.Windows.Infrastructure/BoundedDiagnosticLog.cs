using System.Text;

namespace SuperDuper.Windows.Infrastructure;

internal sealed class BoundedDiagnosticLog : IAsyncDisposable
{
    internal const long DefaultMaximumBytes = 5 * 1024 * 1024;

    private readonly string _path;
    private readonly long _maximumBytes;
    private StreamWriter? _writer;

    private BoundedDiagnosticLog(string path, long maximumBytes)
    {
        _path = path;
        _maximumBytes = maximumBytes;
        RotateIfNeeded();
        _writer = OpenWriter();
    }

    internal static BoundedDiagnosticLog? TryOpen(
        string path,
        long maximumBytes = DefaultMaximumBytes)
    {
        try
        {
            ArgumentException.ThrowIfNullOrWhiteSpace(path);
            ArgumentOutOfRangeException.ThrowIfNegativeOrZero(maximumBytes);
            var fullPath = Path.GetFullPath(path);
            var directory = Path.GetDirectoryName(fullPath)
                ?? throw new IOException("The diagnostic log has no parent directory.");
            Directory.CreateDirectory(directory);
            return new BoundedDiagnosticLog(fullPath, maximumBytes);
        }
        catch (Exception exception) when (
            exception is ArgumentException
                or IOException
                or UnauthorizedAccessException
                or NotSupportedException)
        {
            return null;
        }
    }

    internal async Task<bool> TryWriteLineAsync(string line, CancellationToken cancellationToken)
    {
        if (_writer is null)
        {
            return false;
        }

        try
        {
            await _writer.WriteLineAsync(line.AsMemory(), cancellationToken).ConfigureAwait(false);
            await _writer.FlushAsync(cancellationToken).ConfigureAwait(false);
            if (_writer.BaseStream.Length >= _maximumBytes)
            {
                await _writer.DisposeAsync().ConfigureAwait(false);
                _writer = null;
                RotateIfNeeded();
                _writer = OpenWriter();
            }
            return true;
        }
        catch (Exception exception) when (
            exception is IOException
                or UnauthorizedAccessException
                or ObjectDisposedException
                or NotSupportedException)
        {
            if (_writer is not null)
            {
                await _writer.DisposeAsync().ConfigureAwait(false);
                _writer = null;
            }
            return false;
        }
    }

    public async ValueTask DisposeAsync()
    {
        if (_writer is not null)
        {
            await _writer.DisposeAsync().ConfigureAwait(false);
            _writer = null;
        }
    }

    private void RotateIfNeeded()
    {
        if (File.Exists(_path) && new FileInfo(_path).Length >= _maximumBytes)
        {
            File.Move(_path, _path + ".previous", overwrite: true);
        }
    }

    private StreamWriter OpenWriter()
    {
        var stream = new FileStream(
            _path,
            FileMode.Append,
            FileAccess.Write,
            FileShare.ReadWrite,
            bufferSize: 4096,
            useAsync: true);
        return new StreamWriter(stream, new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
    }
}
