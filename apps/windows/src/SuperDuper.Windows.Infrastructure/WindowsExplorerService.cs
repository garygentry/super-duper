using System.Runtime.InteropServices;
using SuperDuper.Windows.Core.Services;
using Windows.Win32;
using Windows.Win32.UI.Shell.Common;

namespace SuperDuper.Windows.Infrastructure;

public sealed class WindowsExplorerService : IExplorerService
{
    private readonly Action<string> _reveal;
    private readonly Action<string, IReadOnlyList<string>> _selectByParent;

    public WindowsExplorerService()
        : this(Reveal, SelectItems)
    {
    }

    internal WindowsExplorerService(
        Action<string> reveal,
        Action<string, IReadOnlyList<string>>? selectByParent = null)
    {
        _reveal = reveal;
        _selectByParent = selectByParent ?? SelectItems;
    }

    public Task<ExplorerSelectionResult> SelectByParentAsync(
        IReadOnlyList<string> paths,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(paths);
        if (paths.Count is < 1 or > IExplorerService.MaximumSelectionItems)
        {
            throw new ArgumentOutOfRangeException(
                nameof(paths),
                $"Explorer selection requires 1 to {IExplorerService.MaximumSelectionItems} current-page items.");
        }

        var items = paths.Select(CreateSelectionItem).ToArray();
        var groups = items
            .GroupBy(item => item.ShellParentPath, StringComparer.OrdinalIgnoreCase)
            .OrderBy(group => group.Key, StringComparer.OrdinalIgnoreCase)
            .ThenBy(group => group.Key, StringComparer.Ordinal)
            .Select(group => group
                .OrderBy(item => item.ShellPath, StringComparer.OrdinalIgnoreCase)
                .ThenBy(item => item.ShellPath, StringComparer.Ordinal)
                .ToArray())
            .ToArray();
        return SelectGroupsAsync(items.Length, groups, cancellationToken);
    }

    private async Task<ExplorerSelectionResult> SelectGroupsAsync(
        int requestedItemCount,
        IReadOnlyList<SelectionItem[]> groups,
        CancellationToken cancellationToken)
    {
        return await Task.Run(
            () =>
            {
                var selectedItemCount = 0;
                var failures = new List<ExplorerParentSelectionFailure>();
                foreach (var group in groups)
                {
                    cancellationToken.ThrowIfCancellationRequested();
                    try
                    {
                        _selectByParent(
                            group[0].ShellParentPath,
                            group.Select(item => item.ShellPath).ToArray());
                        selectedItemCount += group.Length;
                    }
                    catch (Exception exception)
                    {
                        failures.Add(new ExplorerParentSelectionFailure(
                            group[0].DisplayParentPath,
                            group.Length,
                            exception.Message));
                    }
                }

                cancellationToken.ThrowIfCancellationRequested();
                return new ExplorerSelectionResult(
                    requestedItemCount,
                    groups.Count,
                    selectedItemCount,
                    failures);
            },
            cancellationToken).ConfigureAwait(false);
    }

    private static SelectionItem CreateSelectionItem(string requestedPath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(requestedPath);
        var displayPath = Path.GetFullPath(requestedPath);
        var shellPath = Path.GetFullPath(WindowsShellPath.ToParsingPath(requestedPath));
        var displayParentPath = GetParentPath(displayPath, requestedPath);
        var shellParentPath = GetParentPath(shellPath, requestedPath);
        return new SelectionItem(displayParentPath, shellParentPath, shellPath);
    }

    private static string GetParentPath(string path, string requestedPath)
    {
        var parentPath = Path.GetDirectoryName(
            path.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar));
        if (string.IsNullOrWhiteSpace(parentPath))
        {
            throw new ArgumentException(
                $"Explorer cannot select the root location '{requestedPath}' inside a parent directory.",
                nameof(requestedPath));
        }
        return parentPath;
    }

    public Task RevealAsync(string path, CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path);
        var fullPath = Path.GetFullPath(WindowsShellPath.ToParsingPath(path));
        return RevealWithContextAsync(path, fullPath, cancellationToken);
    }

    private async Task RevealWithContextAsync(
        string requestedPath,
        string shellPath,
        CancellationToken cancellationToken)
    {
        try
        {
            await Task.Run(
                () =>
                {
                    cancellationToken.ThrowIfCancellationRequested();
                    _reveal(shellPath);
                },
                cancellationToken).ConfigureAwait(false);
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

    private static unsafe void SelectItems(string parentPath, IReadOnlyList<string> paths)
    {
        ITEMIDLIST* parent = null;
        var items = stackalloc ITEMIDLIST*[paths.Count];
        var children = stackalloc ITEMIDLIST*[paths.Count];
        for (var index = 0; index < paths.Count; index++)
        {
            items[index] = null;
            children[index] = null;
        }

        try
        {
            PInvoke.SHParseDisplayName(parentPath, null, out parent, 0, null).ThrowOnFailure();
            for (var index = 0; index < paths.Count; index++)
            {
                PInvoke.SHParseDisplayName(paths[index], null, out items[index], 0, null).ThrowOnFailure();
                children[index] = PInvoke.ILFindLastID(items[index]);
            }
            PInvoke.SHOpenFolderAndSelectItems(parent, (uint)paths.Count, children, 0).ThrowOnFailure();
        }
        finally
        {
            if (parent is not null)
            {
                Marshal.FreeCoTaskMem((nint)parent);
            }
            for (var index = 0; index < paths.Count; index++)
            {
                if (items[index] is not null)
                {
                    Marshal.FreeCoTaskMem((nint)items[index]);
                }
            }
        }
    }

    private sealed record SelectionItem(
        string DisplayParentPath,
        string ShellParentPath,
        string ShellPath);
}
