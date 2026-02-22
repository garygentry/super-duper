using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using SuperDuper.Controls;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class ExplorerPage : Page
{
    public ExplorerViewModel ViewModel { get; }

    public ExplorerPage()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        ViewModel = App.Services.GetRequiredService<ExplorerViewModel>();
        this.DataContext = this;

        // Wire events (XAML compiler pass 2 doesn't generate IComponentConnector)
        DirectoryTree.SelectedDirectoryChanged += DirectoryTree_SelectionChanged;
        FileList.SelectedFileChanged += FileList_SelectionChanged;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (e.Parameter is string dirPath && !string.IsNullOrWhiteSpace(dirPath))
        {
            ViewModel.SelectedDirectory = dirPath;
            FileList.DirectoryPath = dirPath;
        }
    }

    private void DirectoryTree_SelectionChanged(object? sender, string? dirPath)
    {
        ViewModel.SelectedDirectory = dirPath;
        FileList.DirectoryPath = dirPath;
    }

    private void FileList_SelectionChanged(object? sender, Models.DbFileInfo? file)
    {
        ViewModel.SelectedFile = file;
        ComparisonPaneControl.SelectedFile = file;
    }
}
