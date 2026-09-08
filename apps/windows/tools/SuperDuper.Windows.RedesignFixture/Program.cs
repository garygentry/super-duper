using System.Windows;
using System.Windows.Controls;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Fixtures;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        // Never construct the shipping App: its OnStartup creates the production worker.
        var application = new Application { ThemeMode = ThemeMode.System };
        application.Resources.MergedDictionaries.Add(new ResourceDictionary
        {
            Source = new Uri("/SuperDuper.Windows;component/Resources/ShellResources.xaml", UriKind.Relative),
        });
        application.Resources.Add("BooleanToVisibilityConverter", new BooleanToVisibilityConverter());
        using var fixture = new ShellFixtureData();
        var model = fixture.Model;
        model.History.SelectedRun = model.History.Runs.Single(run => run.Id == fixture.OldRun.Id);
        model.OpenScanCommand.Execute(null);
        var window = new MainWindow(model, fixture.Client) { Title = "FICTIONAL FIXTURE — Super Duper — no disk actions" };
        var content = (UIElement)window.Content;
        window.Content = null;
        var dock = new DockPanel();
        var toolbar = new WrapPanel { Margin = new Thickness(8) };
        toolbar.Children.Add(new TextBlock
        {
            Text = "In-memory fixture • no worker or disk actions", VerticalAlignment = VerticalAlignment.Center,
        });
        TaskCompletionSource<WorkerDuplicateFolderGroupPage>? pending = null;
        AddButton("Delay folders", () =>
        {
            pending ??= new();
            fixture.Client.FolderGroupPageHandler = (_, _) => pending.Task;
            model.SelectedDestination = WorkspaceDestination.FolderResults;
        });
        AddButton("Fail pending folders", () =>
            pending?.TrySetException(new InvalidOperationException("Fictional delayed folder query failure")));
        AddButton("900 × 600", () => { window.Width = 900; window.Height = 600; });
        AddButton("1180 × 760", () => { window.Width = 1180; window.Height = 760; });
        DockPanel.SetDock(toolbar, Dock.Top);
        dock.Children.Add(toolbar);
        dock.Children.Add(content);
        window.Content = dock;
        if (args.Contains("--verify", StringComparer.Ordinal))
        {
            window.ShowActivated = false;
            window.ShowInTaskbar = false;
            window.WindowStartupLocation = WindowStartupLocation.Manual;
            window.Left = -10000;
            window.Top = -10000;
            window.ContentRendered += (_, _) => window.Close();
        }
        application.Run(window);

        void AddButton(string text, Action action)
        {
            var button = new Button { Content = text, Margin = new Thickness(8, 0, 0, 0) };
            button.Click += (_, _) => action();
            toolbar.Children.Add(button);
        }
    }
}
