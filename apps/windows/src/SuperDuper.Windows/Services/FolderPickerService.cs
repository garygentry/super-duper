using Microsoft.Win32;
using SuperDuper.Windows.Core.Services;

namespace SuperDuper.Windows.Services;

public sealed class FolderPickerService : IFolderPickerService
{
    public Task<string?> PickFolderAsync(CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var dialog = new OpenFolderDialog
        {
            Title = "Choose a scan root",
            Multiselect = false,
        };
        return Task.FromResult(dialog.ShowDialog() == true ? dialog.FolderName : null);
    }
}
