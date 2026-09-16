namespace SuperDuper.Windows.Core.Services;

/// <summary>
/// Stores small, local presentation choices that are independent from scan and worker state.
/// </summary>
public interface IPresentationPreferencesStore
{
    Task<PresentationPreferences> LoadAsync(CancellationToken cancellationToken = default);

    Task SaveAsync(
        PresentationPreferences preferences,
        CancellationToken cancellationToken = default);
}
