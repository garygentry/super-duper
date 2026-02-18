using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using SuperDuper.NativeMethods;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class DeletionReviewPage : Page
{
    public DeletionReviewViewModel ViewModel { get; } = new DeletionReviewViewModel();

    public DeletionReviewPage()
    {
        this.InitializeComponent();
        ViewModel.ErrorOccurred += OnErrorOccurred;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (e.Parameter is (EngineWrapper engine, MainViewModel mainVm))
            ViewModel.Initialize(engine, mainVm);
        else if (e.Parameter is EngineWrapper eng)
            ViewModel.Initialize(eng, null);
    }

    private async void ExecuteButton_Click(object sender, RoutedEventArgs e)
    {
        if (ViewModel.IsExecuting || ViewModel.FileCount == 0) return;

        bool trash = ViewModel.UseTrash;
        var dialog = new ContentDialog
        {
            Title = trash ? "Move to Recycle Bin?" : "Confirm Permanent Deletion",
            Content = trash
                ? $"Move {ViewModel.FileCount} files ({ViewModel.FormattedTotalBytes}) to the Recycle Bin?\n\nYou can restore them from the Recycle Bin if needed."
                : $"Permanently delete {ViewModel.FileCount} files ({ViewModel.FormattedTotalBytes})?\n\nThis cannot be undone.",
            PrimaryButtonText = trash ? "Move to Recycle Bin" : "Delete Files",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = this.XamlRoot,
        };

        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
            ViewModel.ExecuteDeletion();
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
}
