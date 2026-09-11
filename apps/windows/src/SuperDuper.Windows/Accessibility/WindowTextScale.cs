using System.Windows;
using SuperDuper.Windows.Core.Services;

namespace SuperDuper.Windows.Accessibility;

/// <summary>Applies text enlargement without scaling hit targets, icons or monitor coordinates.</summary>
internal sealed class WindowTextScale : IDisposable
{
    private readonly Window _window;
    private readonly ITextScaleSource _source;
    private bool _disposed;

    internal WindowTextScale(Window window, ITextScaleSource source)
    {
        _window = window;
        _source = source;
        _source.Changed += OnChanged;
        Apply();
    }

    private void OnChanged(object? sender, EventArgs e)
    {
        if (!_window.Dispatcher.HasShutdownStarted)
            _ = _window.Dispatcher.BeginInvoke(Apply);
    }

    private void Apply()
    {
        if (_disposed) return;
        var factor = _source.ScaleFactor;
        if (!double.IsFinite(factor) || factor <= 0) factor = 1;
        foreach (var (key, size) in new[]
        {
            ("ShellBodyFontSize", 14d), ("ShellCaptionFontSize", 12d),
            ("ShellSectionFontSize", 16d), ("ShellPageTitleFontSize", 20d), ("ShellTitleFontSize", 24d),
        }) _window.Resources[key] = size * factor;
    }

    public void Dispose()
    {
        _disposed = true;
        _source.Changed -= OnChanged;
        _source.Dispose();
    }
}
