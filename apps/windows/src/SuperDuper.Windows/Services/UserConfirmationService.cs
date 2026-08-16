using System.Windows;
using SuperDuper.Windows.Core.Services;

namespace SuperDuper.Windows.Services;

public sealed class UserConfirmationService : IUserConfirmationService
{
    public Task<bool> ConfirmAsync(
        string title,
        string message,
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var result = MessageBox.Show(
            Application.Current.MainWindow,
            message,
            title,
            MessageBoxButton.YesNo,
            MessageBoxImage.Warning,
            MessageBoxResult.No);
        return Task.FromResult(result == MessageBoxResult.Yes);
    }
}
