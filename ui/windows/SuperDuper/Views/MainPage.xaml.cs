using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.ViewModels;
using Windows.Storage.Pickers;

namespace SuperDuper.Views;

public sealed partial class MainPage : Page
{
    public MainViewModel ViewModel { get; } = new MainViewModel();

    public MainPage()
    {
        this.InitializeComponent();
        ViewModel.SetDispatcherQueue(DispatcherQueue.GetForCurrentThread());
        ViewModel.ErrorOccurred += OnErrorOccurred;
    }

    private async void OnErrorOccurred(object? sender, (string Title, string Detail) error)
    {
        var dialog = new ContentDialog
        {
            Title = error.Title,
            Content = error.Detail,
            CloseButtonText = "OK",
            XamlRoot = this.XamlRoot,
        };
        await dialog.ShowAsync();
    }

    private async void BrowseFolderButton_Click(object sender, RoutedEventArgs e)
    {
        var picker = new FolderPicker();
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(App.MainWindow!);
        WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
        picker.SuggestedStartLocation = PickerLocationId.ComputerFolder;
        picker.FileTypeFilter.Add("*");
        var folder = await picker.PickSingleFolderAsync();
        if (folder != null)
            ViewModel.AddScanPathDirect(folder.Path);
    }

    private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.IsSettingsSelected)
        {
            DashboardContent.Visibility = Visibility.Collapsed;
            SubPageFrame.Visibility = Visibility.Visible;
            SubPageFrame.Navigate(typeof(SettingsPage), (ViewModel.Engine, ViewModel));
            return;
        }

        if (args.SelectedItemContainer is NavigationViewItem item)
        {
            var tag = item.Tag?.ToString();

            if (tag == "dashboard")
            {
                DashboardContent.Visibility = Visibility.Visible;
                SubPageFrame.Visibility = Visibility.Collapsed;
                return;
            }

            DashboardContent.Visibility = Visibility.Collapsed;
            SubPageFrame.Visibility = Visibility.Visible;

            var engine = ViewModel.Engine;
            switch (tag)
            {
                case "duplicates":
                    SubPageFrame.Navigate(typeof(DuplicateGroupsPage), engine);
                    break;
                case "directories":
                    SubPageFrame.Navigate(typeof(DirectoryComparisonPage), engine);
                    break;
                case "deletion":
                    SubPageFrame.Navigate(typeof(DeletionReviewPage), engine);
                    break;
            }
        }
    }
}
