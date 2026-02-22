using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using SuperDuper.Models;
using SuperDuper.Services;
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

        // Wire events (XAML compiler pass 2 doesn't generate IComponentConnector for non-DataTemplate elements)
        StorageTreemapControl.NodeClicked += Treemap_NodeClicked;
        ScanPathInput.KeyDown += ScanPathInput_KeyDown;
        ScanOptionsButton.Click += ScanOptionsButton_Click;
        CancelScanButton.Click += CancelScanButton_Click;

        // Toggle progress panel visibility when IsScanning changes
        ViewModel.ScanService.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(ScanService.IsScanning))
            {
                ScanProgressPanel.Visibility = ViewModel.ScanService.IsScanning
                    ? Visibility.Visible : Visibility.Collapsed;
            }
            // Update progress labels (binding through two-level path can be unreliable)
            if (e.PropertyName == nameof(ScanService.ScanPhaseLabel))
                PhaseLabel.Text = ViewModel.ScanService.ScanPhaseLabel;
            if (e.PropertyName == nameof(ScanService.ScanCountLabel))
                CountLabel.Text = ViewModel.ScanService.ScanCountLabel;
            if (e.PropertyName == nameof(ScanService.CurrentFilePath))
                FilePathLabel.Text = ViewModel.ScanService.CurrentFilePath;
        };

        // Show error dialog on scan failure
        ViewModel.ScanService.ScanError += async (_, msg) =>
        {
            var dlg = new ContentDialog
            {
                Title = "Scan Error",
                Content = msg,
                CloseButtonText = "OK",
                XamlRoot = this.XamlRoot
            };
            await dlg.ShowAsync();
        };
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

    private async void ScanOptionsButton_Click(object sender, RoutedEventArgs e)
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
            // Scan was started via advanced dialog; refresh session picker when done
            await ViewModel.LoadSessionPickerAsync();
        }
    }

    private void CancelScanButton_Click(object sender, RoutedEventArgs e)
    {
        ViewModel.ScanService.CancelScan();
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
