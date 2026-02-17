using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.NativeMethods;

namespace SuperDuper;

public partial class App : Application
{
    private Window? _window;

    public App()
    {
        this.InitializeComponent();
    }

    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        _window = new MainWindow();
        _window.Activate();

        var error = EngineWrapper.ValidateNativeLibrary();
        if (error != null)
        {
            var dialog = new ContentDialog
            {
                Title = "Failed to Load Native Library",
                Content = error,
                CloseButtonText = "Exit",
                XamlRoot = _window.Content.XamlRoot,
            };
            await dialog.ShowAsync();
            _window.Close();
        }
    }
}
