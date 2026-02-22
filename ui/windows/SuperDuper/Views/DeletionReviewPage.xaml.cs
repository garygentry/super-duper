using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class DeletionReviewPage : Page
{
    public DeletionReviewViewModel ViewModel { get; }

    public DeletionReviewPage()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        ViewModel = App.Services.GetRequiredService<DeletionReviewViewModel>();
        this.DataContext = this;
        ViewModel.ErrorOccurred += OnErrorOccurred;

        // Wire events (XAML compiler pass 2 doesn't generate IComponentConnector)
        ExecuteButton.Click += ExecuteButton_Click;
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
            await ViewModel.ExecuteDeletionAsync();
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
