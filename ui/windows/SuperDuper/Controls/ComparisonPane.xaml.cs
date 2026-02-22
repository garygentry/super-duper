using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.Models;
using SuperDuper.Services;

namespace SuperDuper.Controls;

public sealed partial class ComparisonPane : UserControl
{
    private readonly IDatabaseService _db;
    private readonly SuggestionEngine _suggestions;

    public ComparisonPane()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        _db = App.Services.GetRequiredService<IDatabaseService>();
        _suggestions = App.Services.GetRequiredService<SuggestionEngine>();
    }

    private DbFileInfo? _selectedFile;
    public DbFileInfo? SelectedFile
    {
        get => _selectedFile;
        set
        {
            _selectedFile = value;
            EmptyState.Visibility = value == null ? Visibility.Visible : Visibility.Collapsed;
            CardsRepeater.Visibility = value == null ? Visibility.Collapsed : Visibility.Visible;
            if (value != null)
                _ = LoadCopiesAsync(value);
        }
    }

    private async Task LoadCopiesAsync(DbFileInfo file)
    {
        // Phase 3 full implementation — stub loads sibling info
        var sibling = await _db.GetSiblingContextAsync(file.FileId);
        SuggestionBar.Message = $"{sibling.TotalSiblings} siblings, {sibling.DuplicatedSiblings} duplicated";
        SuggestionBar.IsOpen = sibling.DuplicatedSiblings > 0;
    }
}
