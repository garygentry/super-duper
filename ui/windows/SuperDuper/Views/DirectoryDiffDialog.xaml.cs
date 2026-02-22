using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using SuperDuper.ViewModels;

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

        var db = App.Services.GetRequiredService<IDatabaseService>();
        var scanService = App.Services.GetRequiredService<ScanService>();
        var sessionId = scanService.ActiveSessionId ?? 0;

        // Query files from both directories
        var leftResult = await db.QueryFilesInDirectoryAsync(pair.DirAPath, sessionId, 0, 500);
        var rightResult = await db.QueryFilesInDirectoryAsync(pair.DirBPath, sessionId, 0, 500);

        // Build hash sets for comparison (exclude 0 = unhashed files)
        var rightHashes = new HashSet<long>(
            rightResult.Items.Where(f => f.ContentHash != 0).Select(f => f.ContentHash));
        var leftHashes = new HashSet<long>(
            leftResult.Items.Where(f => f.ContentHash != 0).Select(f => f.ContentHash));

        int matched = 0, leftOnly = 0, rightOnly = 0;

        // Categorize left files
        var leftItems = new List<ComparisonFileItem>();
        foreach (var f in leftResult.Items)
        {
            var status = f.ContentHash != 0 && rightHashes.Contains(f.ContentHash)
                ? MatchStatus.Matched : MatchStatus.LeftOnly;
            if (status == MatchStatus.Matched) matched++;
            else leftOnly++;
            leftItems.Add(new ComparisonFileItem(f.FileName, f.FileSize, status));
        }

        // Categorize right files
        var rightItems = new List<ComparisonFileItem>();
        foreach (var f in rightResult.Items)
        {
            var status = f.ContentHash != 0 && leftHashes.Contains(f.ContentHash)
                ? MatchStatus.Matched : MatchStatus.RightOnly;
            if (status == MatchStatus.RightOnly) rightOnly++;
            rightItems.Add(new ComparisonFileItem(f.FileName, f.FileSize, status));
        }

        // Populate repeaters
        LeftRepeater.ItemsSource = leftItems;
        RightRepeater.ItemsSource = rightItems;

        // Handle empty state
        LeftEmptyText.Visibility = leftItems.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        RightEmptyText.Visibility = rightItems.Count == 0 ? Visibility.Visible : Visibility.Collapsed;

        // Update summary
        SummaryText.Text = $"{matched} shared · {leftOnly} left-only · {rightOnly} right-only";
    }
}
