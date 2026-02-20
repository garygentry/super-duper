using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using SuperDuper.Services.Platform;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class SettingsPage : Page
{
    public SessionsViewModel ViewModel { get; }
    private readonly SettingsService _settings;
    private readonly IShellIntegrationService _shell;

    public SettingsPage()
    {
        this.InitializeComponent();
        ViewModel = new SessionsViewModel();
        _settings = App.Services.GetRequiredService<SettingsService>();
        _shell = App.Services.GetRequiredService<IShellIntegrationService>();
        var engine = App.Services.GetRequiredService<EngineWrapper>();
        ViewModel.Initialize(engine, null);

        LoadSettingsToUI();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        _ = ViewModel.LoadSessionsAsync();
    }

    private void LoadSettingsToUI()
    {
        ThemeComboBox.SelectedIndex = (int)_settings.Theme;
        SmartSuggestionsToggle.IsOn = _settings.SmartSuggestionsEnabled;
        DeletionLogPathBox.Text = _settings.DeletionLogPath ?? "";
        DateFormatComboBox.SelectedIndex = _settings.DateFormat switch
        {
            "relative" => 1,
            "iso" => 2,
            _ => 0
        };
        SizeDisplayComboBox.SelectedIndex = (int)_settings.SizeDisplayMode;
        DensityBadgesToggle.IsOn = _settings.ShowDensityBadges;
        ReviewRingsToggle.IsOn = _settings.ShowReviewRings;
        DriveStripesToggle.IsOn = _settings.ShowDriveStripes;
        ContextMenuToggle.IsOn = _settings.ContextMenuRegistered;
    }

    private void ThemeComboBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        _settings.Theme = (Models.AppTheme)ThemeComboBox.SelectedIndex;
        var requested = _settings.Theme switch
        {
            Models.AppTheme.Light => Microsoft.UI.Xaml.ElementTheme.Light,
            Models.AppTheme.Dark => Microsoft.UI.Xaml.ElementTheme.Dark,
            _ => Microsoft.UI.Xaml.ElementTheme.Default
        };
        if (App.MainWindow?.Content is FrameworkElement root)
            root.RequestedTheme = requested;
    }

    private void SmartSuggestionsToggle_Toggled(object sender, RoutedEventArgs e)
    {
        _settings.SmartSuggestionsEnabled = SmartSuggestionsToggle.IsOn;
        // SettingsService auto-saves on each property set
    }

    private void DeletionLogBrowse_Click(object sender, RoutedEventArgs e)
    {
        // Full browse implementation uses IFilePickerService (Phase 7)
    }

    private void DateFormatComboBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        _settings.DateFormat = DateFormatComboBox.SelectedIndex switch
        {
            1 => "relative",
            2 => "iso",
            _ => "short"
        };
    }

    private void SizeDisplayComboBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        _settings.SizeDisplayMode = (Models.SizeDisplayMode)SizeDisplayComboBox.SelectedIndex;
    }

    private void DensityBadgesToggle_Toggled(object sender, RoutedEventArgs e)
    {
        _settings.ShowDensityBadges = DensityBadgesToggle.IsOn;
    }

    private void ReviewRingsToggle_Toggled(object sender, RoutedEventArgs e)
    {
        _settings.ShowReviewRings = ReviewRingsToggle.IsOn;
    }

    private void DriveStripesToggle_Toggled(object sender, RoutedEventArgs e)
    {
        _settings.ShowDriveStripes = DriveStripesToggle.IsOn;
    }

    private void ContextMenuToggle_Toggled(object sender, RoutedEventArgs e)
    {
        if (ContextMenuToggle.IsOn)
            _settings.ContextMenuRegistered = _shell.RegisterContextMenu();
        else
        {
            _shell.UnregisterContextMenu();
            _settings.ContextMenuRegistered = false;
        }
    }

    private async void DeleteAllSessionsButton_Click(object sender, RoutedEventArgs e)
    {
        var dialog = new ContentDialog
        {
            Title = "Delete All Sessions?",
            Content = "This will permanently delete all sessions, duplicate groups, and directory analysis.\n\n" +
                      "The file index and hash cache are preserved — the next scan will be fast.\n\n" +
                      "This cannot be undone.",
            PrimaryButtonText = "Delete All",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = this.XamlRoot,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
        {
            try { await ViewModel.ResetAllSessionsAsync(); }
            catch (Exception ex) { await ShowErrorDialog("Delete All Sessions failed", ex.Message); }
        }
    }

    private async void ResetAllSessionsButton_Click(object sender, RoutedEventArgs e)
    {
        var dialog = new ContentDialog
        {
            Title = "Reset All Sessions?",
            Content = "This will permanently delete all sessions, duplicate groups, and directory analysis.\n\n" +
                      "The file index and hash cache are preserved — the next scan will be fast.\n\n" +
                      "This cannot be undone.",
            PrimaryButtonText = "Reset All Sessions",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = this.XamlRoot,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
        {
            try { await ViewModel.ResetAllSessionsAsync(); }
            catch (Exception ex) { await ShowErrorDialog("Reset All Sessions failed", ex.Message); }
        }
    }

    private async void ResetEverythingButton_Click(object sender, RoutedEventArgs e)
    {
        var dialog = new ContentDialog
        {
            Title = "Reset Everything?",
            Content = "This will permanently delete ALL database records AND the hash cache.\n\n" +
                      "Every file will be re-hashed from scratch on the next scan.\n\n" +
                      "This cannot be undone.",
            PrimaryButtonText = "Reset Everything",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = this.XamlRoot,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
        {
            try { await ViewModel.ResetEverythingAsync(); }
            catch (Exception ex) { await ShowErrorDialog("Reset Everything failed", ex.Message); }
        }
    }

    private async Task ShowErrorDialog(string title, string message)
    {
        var dialog = new ContentDialog
        {
            Title = title,
            Content = message,
            CloseButtonText = "OK",
            XamlRoot = this.XamlRoot,
        };
        await dialog.ShowAsync();
    }
}
