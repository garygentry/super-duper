using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using SuperDuper.Services;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class DirectoriesPage : Page
{
    public DirectoriesViewModel ViewModel { get; }

    public DirectoriesPage()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        ViewModel = App.Services.GetRequiredService<DirectoriesViewModel>();
        this.DataContext = this;
        _ = ViewModel.LoadAsync();

        // React to session changes while this page is alive
        var scanService = App.Services.GetRequiredService<ScanService>();
        scanService.ActiveSessionChanged += (_, _) =>
        {
            ViewModel.SelectedPair = null;
            _ = ViewModel.LoadAsync();
        };
    }

    private void PairCard_PointerPressed(object sender, PointerRoutedEventArgs e)
    {
        if (sender is FrameworkElement el && el.Tag is DirectoryPairViewModel pair)
        {
            _ = ViewModel.LoadComparisonAsync(pair);
        }
    }
}
