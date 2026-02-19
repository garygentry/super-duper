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
            await ViewModel.ResetAllSessionsAsync();
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
            await ViewModel.ResetEverythingAsync();
    }
}
