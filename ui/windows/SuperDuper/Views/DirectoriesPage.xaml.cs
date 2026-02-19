using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class DirectoriesPage : Page
{
    public DirectoriesViewModel ViewModel { get; }

    public DirectoriesPage()
    {
        this.InitializeComponent();
        ViewModel = App.Services.GetRequiredService<DirectoriesViewModel>();
        _ = ViewModel.LoadAsync();
    }
}
