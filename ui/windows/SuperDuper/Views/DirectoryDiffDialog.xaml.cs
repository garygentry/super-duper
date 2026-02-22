using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using SuperDuper.NativeMethods;

namespace SuperDuper.Views;

/// <summary>
/// Side-by-side file diff dialog for two similar directories.
/// Files are categorized as: matched (in both), left-only, right-only.
/// </summary>
public sealed partial class DirectoryDiffDialog : ContentDialog
{
    public DirectoryDiffDialog()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        this.DataContext = this;
    }

    public async Task LoadAsync(EngineWrapper engine, DirectorySimilarityInfo pair)
    {
        LeftHeader.Text = pair.DirADisplayPath;
        RightHeader.Text = pair.DirBDisplayPath;

        await Task.Run(() =>
        {
            // Use a simplified comparison — list directory children from DB or FFI
            // In practice, diff is computed from directory_fingerprint hash sets
        });

        // For now, show placeholder summary
        SummaryText.Text = $"{pair.FormattedSharedBytes} shared · {pair.FormattedScore} match";

        // TODO: populate LeftRepeater and RightRepeater with file lists
        // Full implementation requires a DatabaseService query returning per-file match state
    }
}
