using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using SuperDuper.NativeMethods;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class DirectoryComparisonPage : Page
{
    public DirectoryComparisonViewModel ViewModel { get; } = new DirectoryComparisonViewModel();

    public DirectoryComparisonPage()
    {
        this.InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (e.Parameter is EngineWrapper engine)
        {
            ViewModel.Initialize(engine);
        }
    }
}
