using System.Runtime.InteropServices;
using SuperDuper.Windows.Core.Services;
using Windows.Win32;
using Windows.Win32.UI.Shell.Common;

namespace SuperDuper.Windows.Infrastructure;

public sealed class WindowsExplorerService : IExplorerService
{
    public Task RevealAsync(string path, CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        var fullPath = Path.GetFullPath(WindowsShellPath.ToParsingPath(path));
        return RevealWithContextAsync(path, fullPath, cancellationToken);
    }

    private static async Task RevealWithContextAsync(
        string requestedPath,
        string shellPath,
        CancellationToken cancellationToken)
    {
        try
        {
            await Task.Run(() => Reveal(shellPath), cancellationToken).ConfigureAwait(false);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            throw new InvalidOperationException(
                $"File Explorer could not reveal '{requestedPath}'. {exception.Message}",
                exception);
        }
    }

    private static unsafe void Reveal(string path)
    {
        ITEMIDLIST* item = null;
        ITEMIDLIST* parent = null;
        try
        {
            PInvoke.SHParseDisplayName(path, null, out item, 0, null).ThrowOnFailure();
            var parentPath = Path.GetDirectoryName(path.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar));
            if (string.IsNullOrWhiteSpace(parentPath))
            {
                PInvoke.SHOpenFolderAndSelectItems(item, 0, null, 0).ThrowOnFailure();
                return;
            }

            PInvoke.SHParseDisplayName(parentPath, null, out parent, 0, null).ThrowOnFailure();
            var child = PInvoke.ILFindLastID(item);
            PInvoke.SHOpenFolderAndSelectItems(parent, 1, &child, 0).ThrowOnFailure();
        }
        finally
        {
            if (parent is not null)
            {
                Marshal.FreeCoTaskMem((nint)parent);
            }
            if (item is not null)
            {
                Marshal.FreeCoTaskMem((nint)item);
            }
        }
    }
}
