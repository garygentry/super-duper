using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.NativeMethods;
using SuperDuper.Services.Platform;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class ScanDialog : ContentDialog
{
    public ScanDialogViewModel ViewModel { get; }
    private readonly INotificationService _notifications;
    private readonly EngineWrapper _engine;

    public ScanDialog()
    {
        this.InitializeComponent();
        ViewModel = App.Services.GetRequiredService<ScanDialogViewModel>();
        _notifications = App.Services.GetRequiredService<INotificationService>();
        _engine = App.Services.GetRequiredService<EngineWrapper>();
        ViewModel.SetDispatcherQueue(DispatcherQueue.GetForCurrentThread());
        ViewModel.LoadAvailableDrives();
        ViewModel.ScanCompleted += OnScanCompleted;
        ViewModel.ErrorOccurred += OnErrorOccurred;

        // Bind the progress overlay so it can read scan phase/progress properties
        Loaded += (_, _) => ProgressOverlay.Bind(ViewModel);

        PrimaryButtonClick += OnPrimaryButtonClick;
    }

    private async void OnPrimaryButtonClick(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        var deferral = args.GetDeferral();
        try
        {
            await ViewModel.StartScanCommand.ExecuteAsync(null);
        }
        finally
        {
            deferral.Complete();
        }
    }

    private void StepPivot_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (StepPivot.SelectedIndex == 2)
        {
            var pathCount = ViewModel.ScanPaths.Count;
            ConfirmSummaryText.Text = $"Ready to scan {pathCount} location(s) with the selected options.";
        }
    }

    private void SaveProfile_Click(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        // Profile save — Phase 6.2 (SettingsService.ScanProfiles)
    }

    private void OnScanCompleted(object? sender, EventArgs e)
    {
        // Show a toast when the window is not in the foreground
        if (App.MainWindow?.AppWindow?.IsVisible == false)
        {
            try
            {
                // Query just 1 item to get the TotalAvailable count efficiently
                var (_, totalGroups) = _engine.QueryDuplicateGroups(offset: 0, limit: 1);
                _notifications.ShowScanComplete(totalGroups, wastedBytes: 0);
            }
            catch { /* toast is optional — swallow any errors */ }
        }
    }

    private async void OnErrorOccurred(object? sender, string error)
    {
        var dialog = new ContentDialog
        {
            Title = "Scan Error",
            Content = error,
            CloseButtonText = "OK",
            XamlRoot = this.XamlRoot,
        };
        await dialog.ShowAsync();
    }
}
