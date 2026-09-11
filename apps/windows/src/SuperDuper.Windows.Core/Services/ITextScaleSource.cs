namespace SuperDuper.Windows.Core.Services;

/// <summary>The operator's accessibility text enlargement, independent of monitor DPI.</summary>
public interface ITextScaleSource : IDisposable
{
    double ScaleFactor { get; }
    event EventHandler? Changed;
}
