using Microsoft.UI.Xaml.Controls;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class DirectoryComparisonPage : Page
{
    public DirectoryComparisonViewModel ViewModel { get; } = new DirectoryComparisonViewModel();

    public DirectoryComparisonPage()
    {
        this.InitializeComponent();
    }
}
