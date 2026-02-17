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
    }

    private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.SelectedItemContainer is NavigationViewItem item)
        {
            var tag = item.Tag?.ToString();

            if (tag == "dashboard")
            {
                DashboardContent.Visibility = Visibility.Visible;
                SubPageFrame.Visibility = Visibility.Collapsed;
                return;
            }

            // Navigate to a sub-page
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
