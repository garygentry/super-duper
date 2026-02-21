using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using SuperDuper.Models;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class DashboardPage : Page
{
    public DashboardViewModel ViewModel { get; }

    public DashboardPage()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        ViewModel = App.Services.GetRequiredService<DashboardViewModel>();
        this.DataContext = this;
        ViewModel.SetDispatcherQueue(DispatcherQueue.GetForCurrentThread());
        ViewModel.NewScanDialogRequested += OnNewScanDialogRequested;

        // Wire events (XAML compiler pass 2 doesn't generate IComponentConnector)
        StorageTreemapControl.NodeClicked += Treemap_NodeClicked;
        ScanPathInput.KeyDown += ScanPathInput_KeyDown;
    }

    private void ScanPathInput_KeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == Windows.System.VirtualKey.Enter)
        {
            ViewModel.AddScanPathCommand.Execute(null);
            e.Handled = true;
        }
    }

    private void RemoveScanPath_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button btn && btn.DataContext is string path)
            ViewModel.RemoveScanPathCommand.Execute(path);
    }

    private async void OnNewScanDialogRequested(object? sender, EventArgs e)
    {
        var dialog = new ScanDialog
        {
            XamlRoot = this.XamlRoot
        };
        var result = await dialog.ShowAsync();

        // Sync scan paths back (dialog may have changed settings via scan completion)
        ViewModel.ReloadScanPaths();

        if (result == ContentDialogResult.Primary)
        {
            // Scan was started; refresh session picker when done
            await ViewModel.LoadSessionPickerAsync();
        }
    }

    private void Treemap_NodeClicked(object? sender, TreemapNode node)
    {
        // Navigate to ExplorerPage with the selected directory path
        Frame.Navigate(typeof(ExplorerPage), node.Path);
    }

    private void QuickWinAction_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button btn && btn.Tag is QuickWinItem item)
        {
            switch (item.Category)
            {
                case "Identical Directories":
                    Frame.Navigate(typeof(DirectoriesPage), item.Payload);
                    break;
                case "Largest Duplicate Groups":
                case "Single-Drive Cluster":
                    Frame.Navigate(typeof(GroupsPage), item.Payload);
                    break;
                default:
                    Frame.Navigate(typeof(GroupsPage), item.Payload);
                    break;
            }
        }
    }
}
