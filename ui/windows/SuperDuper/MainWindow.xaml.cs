using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using SuperDuper.ViewModels;
using SuperDuper.Views;

namespace SuperDuper;

public sealed partial class MainWindow : Window
{
    public ShellViewModel ViewModel { get; }

    public MainWindow()
    {
        this.InitializeComponent();

        ViewModel = App.Services.GetRequiredService<ShellViewModel>();

        SystemBackdrop = new MicaBackdrop { Kind = MicaKind.Base };
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        // Set initial page to Dashboard
        NavView.SelectedItem = NavView.MenuItems[0];
        ContentFrame.Navigate(typeof(DashboardPage));

        // Subscribe to deletion dialog requests from ShellViewModel commands
        ShellViewModel.OpenDeletionDialogRequested += OnOpenDeletionDialogRequested;

        // Refresh deletion count on load
        _ = ViewModel.RefreshDeletionCountAsync();

        // Set window icon
        var iconPath = System.IO.Path.Combine(
            System.IO.Path.GetDirectoryName(
                System.Reflection.Assembly.GetExecutingAssembly().Location)!,
            "Assets", "AppIcon.ico");
        if (System.IO.File.Exists(iconPath))
            AppWindow.SetIcon(iconPath);
    }

    private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.IsSettingsSelected)
        {
            ContentFrame.Navigate(typeof(SettingsPage));
            return;
        }

        if (args.SelectedItemContainer is NavigationViewItem item)
        {
            var tag = item.Tag?.ToString();
            switch (tag)
            {
                case "dashboard":
                    ContentFrame.Navigate(typeof(DashboardPage));
                    break;
                case "explorer":
                    ContentFrame.Navigate(typeof(ExplorerPage));
                    break;
                case "groups":
                    ContentFrame.Navigate(typeof(GroupsPage));
                    break;
                case "directories":
                    ContentFrame.Navigate(typeof(DirectoriesPage));
                    break;
            }
        }
    }

    private async void ReviewDeleteButton_Click(object sender, RoutedEventArgs e)
    {
        await OpenDeletionDialogAsync();
    }

    private async void OnOpenDeletionDialogRequested(object? sender, EventArgs e)
    {
        await OpenDeletionDialogAsync();
    }

    private async Task OpenDeletionDialogAsync()
    {
        var dialog = new DeletionConfirmationDialog
        {
            XamlRoot = this.Content.XamlRoot
        };
        await dialog.ShowAsync();
        await ViewModel.RefreshDeletionCountAsync();
    }

    private async void OnUndoAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        await ViewModel.UndoCommand.ExecuteAsync(null);
    }

    private async void OnRedoAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        await ViewModel.RedoCommand.ExecuteAsync(null);
    }

    private void OnOpenDeletionDialogAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        ReviewDeleteButton_Click(sender, new RoutedEventArgs());
    }

    private void OnRefreshAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        // Re-navigate to current page to refresh
        if (ContentFrame.CurrentSourcePageType != null)
            ContentFrame.Navigate(ContentFrame.CurrentSourcePageType);
    }
}
