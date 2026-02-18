using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.NativeMethods;

namespace SuperDuper;

public partial class App : Application
{
    public static Window? MainWindow { get; private set; }

    public App()
    {
        this.InitializeComponent();
    }

    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        MainWindow = new MainWindow();
        MainWindow.Activate();

        var error = EngineWrapper.ValidateNativeLibrary();
        if (error != null)
        {
            var dialog = new ContentDialog
            {
                Title = "Failed to Load Native Library",
                Content = error,
                CloseButtonText = "Exit",
                XamlRoot = MainWindow.Content.XamlRoot,
            };
            await dialog.ShowAsync();
            MainWindow.Close();
        }
    }
}
