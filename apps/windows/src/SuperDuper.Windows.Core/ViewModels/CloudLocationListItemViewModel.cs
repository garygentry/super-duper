namespace SuperDuper.Windows.Core.ViewModels;

public sealed record CloudLocationListItemViewModel(
    string ProviderName,
    string Path,
    string Behavior)
{
    public string AccessibilityName => $"{ProviderName}: {Path}. {Behavior}";
}
