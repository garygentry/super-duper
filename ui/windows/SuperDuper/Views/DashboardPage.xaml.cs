using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class DashboardPage : Page
{
    public DashboardViewModel ViewModel { get; }

    public DashboardPage()
    {
        this.InitializeComponent();
        ViewModel = App.Services.GetRequiredService<DashboardViewModel>();
        ViewModel.SetDispatcherQueue(DispatcherQueue.GetForCurrentThread());
        ViewModel.NewScanDialogRequested += OnNewScanDialogRequested;
    }

    private async void OnNewScanDialogRequested(object? sender, EventArgs e)
    {
        var dialog = new ScanDialog
        {
            XamlRoot = this.XamlRoot
        };
        var result = await dialog.ShowAsync();
        if (result == ContentDialogResult.Primary)
        {
            // Scan was started; refresh session picker when done
            await ViewModel.LoadSessionPickerAsync();
        }
    }
}
