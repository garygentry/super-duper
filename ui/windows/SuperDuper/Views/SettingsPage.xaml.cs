using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using SuperDuper.NativeMethods;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class SettingsPage : Page
{
    public SessionsViewModel ViewModel { get; } = new();

    public SettingsPage()
    {
        this.InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (e.Parameter is (EngineWrapper engine, MainViewModel mainVm))
            ViewModel.Initialize(engine, mainVm);
        else if (e.Parameter is EngineWrapper eng)   // fallback compatibility
            ViewModel.Initialize(eng, null);
    }

    private async void ClearHistoryButton_Click(object sender, RoutedEventArgs e)
    {
        var dialog = new ContentDialog
        {
            Title = "Clear Scan History?",
            Content = "This will permanently delete all sessions, file records, and duplicate groups.\n\n" +
                      "The hash cache is preserved — the next scan will be fast.\n\n" +
                      "This cannot be undone.",
            PrimaryButtonText = "Clear History",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = this.XamlRoot,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
            await ViewModel.ClearScanHistoryAsync();
    }

    private async void FullResetButton_Click(object sender, RoutedEventArgs e)
    {
        var dialog = new ContentDialog
        {
            Title = "Full Reset?",
            Content = "This will permanently delete ALL database records AND the hash cache.\n\n" +
                      "The next scan will re-hash every file from scratch.\n\n" +
                      "This cannot be undone.",
            PrimaryButtonText = "Reset Everything",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = this.XamlRoot,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
            await ViewModel.FullResetAsync();
    }
}
