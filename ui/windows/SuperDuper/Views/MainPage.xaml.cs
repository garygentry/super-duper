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

            switch (tag)
            {
                case "duplicates":
                    SubPageFrame.Navigate(typeof(DuplicateGroupsPage));
                    break;
                case "directories":
                    SubPageFrame.Navigate(typeof(DirectoryComparisonPage));
                    break;
                case "deletion":
                    SubPageFrame.Navigate(typeof(DeletionReviewPage));
                    break;
            }
        }
    }
}
