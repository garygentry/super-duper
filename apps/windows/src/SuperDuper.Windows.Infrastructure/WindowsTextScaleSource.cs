using SuperDuper.Windows.Core.Services;
using Windows.UI.ViewManagement;

namespace SuperDuper.Windows.Infrastructure;

public sealed class WindowsTextScaleSource : ITextScaleSource
{
    private readonly UISettings _settings = new();

    public WindowsTextScaleSource() => _settings.TextScaleFactorChanged += OnChanged;

    public double ScaleFactor => _settings.TextScaleFactor;
    public event EventHandler? Changed;

    private void OnChanged(UISettings sender, object args) => Changed?.Invoke(this, EventArgs.Empty);

    public void Dispose() => _settings.TextScaleFactorChanged -= OnChanged;
}
