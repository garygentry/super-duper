using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.ViewModels;

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

    private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.IsSettingsSelected)
        {
            DashboardContent.Visibility = Visibility.Collapsed;
            SubPageFrame.Visibility = Visibility.Visible;
            SubPageFrame.Navigate(typeof(SettingsPage));
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
