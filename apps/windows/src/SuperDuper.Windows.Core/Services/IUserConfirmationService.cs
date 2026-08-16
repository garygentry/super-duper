namespace SuperDuper.Windows.Core.Services;

public interface IUserConfirmationService
{
    Task<bool> ConfirmAsync(
        string title,
        string message,
        CancellationToken cancellationToken = default);
}
