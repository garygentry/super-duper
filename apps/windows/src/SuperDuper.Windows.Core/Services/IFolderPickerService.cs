namespace SuperDuper.Windows.Core.Services;

public interface IFolderPickerService
{
    Task<string?> PickFolderAsync(CancellationToken cancellationToken = default);
}
