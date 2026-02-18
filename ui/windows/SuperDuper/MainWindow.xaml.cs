using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace SuperDuper;

public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        this.InitializeComponent();
        SystemBackdrop = new MicaBackdrop { Kind = MicaKind.Base };
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        ContentFrame.Navigate(typeof(Views.MainPage));

        var iconPath = System.IO.Path.Combine(
            System.IO.Path.GetDirectoryName(
                System.Reflection.Assembly.GetExecutingAssembly().Location)!,
            "Assets", "AppIcon.ico");

        if (System.IO.File.Exists(iconPath))
            AppWindow.SetIcon(iconPath);
    }
}
