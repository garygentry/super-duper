using Microsoft.UI.Xaml;

namespace SuperDuper;

public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        this.InitializeComponent();
        ContentFrame.Navigate(typeof(Views.MainPage));
    }
}
