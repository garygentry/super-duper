using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.Controls;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class ExplorerPage : Page
{
    public ExplorerViewModel ViewModel { get; }

    public ExplorerPage()
    {
        this.InitializeComponent();
        ViewModel = App.Services.GetRequiredService<ExplorerViewModel>();
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
