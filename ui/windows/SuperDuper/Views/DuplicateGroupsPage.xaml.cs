using Microsoft.UI.Xaml.Controls;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class DuplicateGroupsPage : Page
{
    public DuplicateGroupsViewModel ViewModel { get; } = new DuplicateGroupsViewModel();

    public DuplicateGroupsPage()
    {
        this.InitializeComponent();
    }
}
