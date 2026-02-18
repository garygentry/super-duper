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
    }
}
