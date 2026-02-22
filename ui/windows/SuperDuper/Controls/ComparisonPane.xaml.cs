using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using SuperDuper.Models;
using SuperDuper.Services;
using SuperDuper.Services.Platform;
using SuperDuper.ViewModels;

namespace SuperDuper.Controls;

public sealed partial class ComparisonPane : UserControl
{
    private readonly IDatabaseService _db;
    private readonly SuggestionEngine _suggestions;
    private readonly DriveColorService _driveColors;
    private readonly IUndoService _undo;
    private readonly IShellIntegrationService _shell;

    private List<DuplicateCardViewModel>? _viewModels;

    public ComparisonPane()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        _db = App.Services.GetRequiredService<IDatabaseService>();
        _suggestions = App.Services.GetRequiredService<SuggestionEngine>();
        _driveColors = App.Services.GetRequiredService<DriveColorService>();
        _undo = App.Services.GetRequiredService<IUndoService>();
        _shell = App.Services.GetRequiredService<IShellIntegrationService>();

        CardsRepeater.ElementPrepared += CardsRepeater_ElementPrepared;
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
                LoadCopiesAsync(value).FireAndForget("ComparisonPane.LoadCopiesAsync");
            else
            {
                _viewModels = null;
                CardsRepeater.ItemsSource = null;
                SuggestionBar.IsOpen = false;
            }
        }
    }

    private async Task LoadCopiesAsync(DbFileInfo file)
    {
        if (file.GroupId == 0)
        {
            _viewModels = null;
            CardsRepeater.ItemsSource = null;
            SuggestionBar.Message = "This file has no duplicates.";
            SuggestionBar.Severity = InfoBarSeverity.Informational;
            SuggestionBar.IsOpen = true;
            return;
        }

        var copies = await _db.QueryFilesInGroupAsync(file.GroupId);
        if (copies.Count == 0)
        {
            _viewModels = null;
            CardsRepeater.ItemsSource = null;
            SuggestionBar.IsOpen = false;
            return;
        }

        // Determine newest/oldest by modified date
        long newestId = -1, oldestId = -1;
        DateTime? newestDate = null, oldestDate = null;
        foreach (var copy in copies)
        {
            var dt = ParseDate(copy.LastModified);
            if (dt == null) continue;
            if (newestDate == null || dt > newestDate)
            {
                newestDate = dt;
                newestId = copy.FileId;
            }
            if (oldestDate == null || dt < oldestDate)
            {
                oldestDate = dt;
                oldestId = copy.FileId;
            }
        }
        // Don't show both badges on the same file
        if (newestId == oldestId) oldestId = -1;

        // Suggestion engine
        var suggestion = _suggestions.Suggest(copies);
        if (suggestion != null)
        {
            SuggestionBar.Message = suggestion.Reason;
            SuggestionBar.Severity = InfoBarSeverity.Informational;
            SuggestionBar.IsOpen = true;
        }
        else
        {
            SuggestionBar.IsOpen = false;
        }

        // Build ViewModels
        var vms = new List<DuplicateCardViewModel>(copies.Count);
        foreach (var copy in copies)
        {
            var decision = await _db.GetDecisionAsync(copy.FileId);
            var brush = new SolidColorBrush(_driveColors.GetColor(copy.DriveLetter));
            var heuristic = _suggestions.GetHeuristicLabel(copy.CanonicalPath);

            vms.Add(new DuplicateCardViewModel(
                copy.FileId, copy.GroupId, copy.CanonicalPath,
                copy.LastModified, "", "", // Created/Accessed not stored in DB
                copy.FileId == newestId, copy.FileId == oldestId,
                decision,
                brush,
                string.IsNullOrEmpty(heuristic) ? null : heuristic,
                null, // SiblingsText
                _db, _undo, _shell
            ));
        }

        _viewModels = vms;
        CardsRepeater.ItemsSource = vms;
    }

    private void CardsRepeater_ElementPrepared(ItemsRepeater sender, ItemsRepeaterElementPreparedEventArgs args)
    {
        if (args.Element is DuplicateCard card && _viewModels != null && args.Index < _viewModels.Count)
        {
            card.Bind(_viewModels[args.Index]);
        }
    }

    private static DateTime? ParseDate(string? dateStr)
    {
        if (string.IsNullOrEmpty(dateStr)) return null;
        if (DateTime.TryParse(dateStr, out var dt)) return dt;
        if (long.TryParse(dateStr, out var unix))
            return DateTimeOffset.FromUnixTimeSeconds(unix).DateTime;
        return null;
    }
}
