using System.Windows;
using SuperDuper.Windows.Core.Services;

namespace SuperDuper.Windows.Services;

public sealed class WpfClipboardService : IClipboardService
{
    public void CopyText(string text)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(text);
        Clipboard.SetText(text);
    }
}
