using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Smoke.Tests;

// Runs on the existing App's STA, without invoking App.OnStartup or owning a real worker.
internal static class PopulatedShellFixture
{
    internal static void Verify()
    {
        using var data = new SuperDuper.Windows.Fixtures.ShellFixtureData();
        var client = data.Client;
        var model = data.Model;
        var old = data.OldRun;
        var active = data.ActiveRun;
        var window = new MainWindow(model, client, ownsWorkerLifetime: false)
        {
            ShowActivated = false, ShowInTaskbar = false,
            WindowStartupLocation = WindowStartupLocation.Manual, Left = -10000, Top = -10000,
        };
        // RenderTargetBitmap does not include the native window background. Use an explicit
        // system brush in this offscreen host; do not override the shipping Fluent background.
        window.SetResourceReference(Window.BackgroundProperty, SystemColors.WindowBrushKey);
        try
        {
            window.Show();
            Drain();
            model.SelectedDestination = WorkspaceDestination.History;
            model.History.SelectedRun = model.History.Runs.Single(run => run.Id == old.Id);
            model.OpenScanCommand.Execute(null);
            Drain();
            var results = (TabControl)window.FindName("ResultsTabs");
            Assert.IsTrue(((TabItem)results.SelectedItem).IsKeyboardFocused,
                "The actual MainWindow handler must deliver keyboard focus after Open scan.");
            Assert.AreEqual(old.Id, model.SelectedRun?.Id);
            Assert.AreEqual(active.Id, model.Progress.Run?.Id);
            var groups = Find<DataGrid>(window, "FileGroupsGrid");

            model.SelectedDestination = WorkspaceDestination.History;
            model.History.SelectedRun = model.History.Runs.Single(run => run.Id == old.Id);
            model.History.OpenWarningsCommand.ExecuteAsync(null).GetAwaiter().GetResult();
            Drain();
            model.History.NavigateWarningCommand.ExecuteAsync(model.History.Warnings.Single()).GetAwaiter().GetResult();
            Drain();
            Assert.IsTrue(groups.IsKeyboardFocusWithin,
                "Warning-result navigation must reach the real file grid through MainWindow.");

            var selected = model.DuplicateFiles.SelectedGroup;
            var groupScroll = Descendants<ScrollViewer>(groups).First();
            groupScroll.ScrollToVerticalOffset(10);
            Drain();
            var groupOffset = groupScroll.VerticalOffset;
            Assert.IsTrue(groupOffset > 0);
            model.SelectedDestination = WorkspaceDestination.Review;
            Drain();
            model.SelectedDestination = WorkspaceDestination.FileResults;
            Drain();
            Assert.AreSame(selected, model.DuplicateFiles.SelectedGroup);
            Assert.AreEqual(groupOffset, groupScroll.VerticalOffset, 1d);
            Assert.AreEqual(25, groups.Items.Count);
            Assert.AreEqual(36d, groups.MinRowHeight);
            var filesView = (SuperDuper.Windows.Views.DuplicateFilesView)window.FindName("DuplicateFilesWorkspace");
            var search = (TextBox)filesView.FindName("FileSearch");
            Assert.AreEqual(14d, search.FontSize);
            Assert.AreEqual(32d, search.MinHeight);
            Assert.IsNotNull(((Button)filesView.FindName("FileApplyFilters")).FocusVisualStyle);

            // The posted focus for Open scan must not steal focus after intervening navigation.
            model.OpenScanCommand.Execute(null);
            model.SelectedDestination = WorkspaceDestination.ScanSetup;
            model.SelectedDestination = WorkspaceDestination.FileResults;
            groups.Focus();
            Drain();
            Assert.IsTrue(groups.IsKeyboardFocusWithin,
                "An away-and-back navigation invalidates the queued Open scan tab-focus request.");

            // Native ScrollViewer state, not merely retained view-model collections.
            model.ViewProgressCommand.Execute(null);
            Drain();
            Assert.IsTrue(((TabItem)((TabControl)window.FindName("ScanTabs")).SelectedItem).IsKeyboardFocused);
            var scroll = Find<ScrollViewer>(window, "ScanProgressScrollViewer");
            scroll.ScrollToVerticalOffset(180);
            Drain();
            var offset = scroll.VerticalOffset;
            Assert.IsTrue(offset > 0, "Populated progress must exercise a nonzero scroll offset.");
            model.SelectedDestination = WorkspaceDestination.Review;
            Drain();
            model.SelectedDestination = WorkspaceDestination.ScanProgress;
            Drain();
            Assert.AreEqual(offset, scroll.VerticalOffset, 1d, "Same-run pane reopening retains scroll.");

            model.Progress.OpenWarningsCommand.ExecuteAsync(null).GetAwaiter().GetResult();
            Drain();
            var warnings = Find<DataGrid>(window, "RunWarningGrid");
            Assert.IsTrue(warnings.IsKeyboardFocusWithin);
            Assert.AreEqual(1, warnings.Items.Count);
            Assert.AreEqual(old.Id, model.SelectedRun?.Id);
            model.History.CloseWarningsCommand.Execute(null);
            Drain();
            Assert.IsTrue(Find<DataGrid>(window, "RunHistoryGrid").IsKeyboardFocusWithin,
                "Close warnings restores focus through RunHistoryView's real handler.");
            model.ViewProgressCommand.Execute(null);
            Drain();
            Assert.AreEqual(active.Id, model.Progress.Run?.Id);

            // A deliberately blocked optional query cannot hold the primary workspace hostage.
            var folderPage = new TaskCompletionSource<WorkerDuplicateFolderGroupPage>();
            client.FolderGroupPageHandler = (_, _) => folderPage.Task;
            model.SelectedDestination = WorkspaceDestination.FolderResults;
            Drain();
            model.SelectedDestination = WorkspaceDestination.FileResults;
            Drain();
            Assert.AreSame(groups.ItemsSource, model.DuplicateFiles.Groups);
            Assert.AreEqual(1, data.FileQueries, "Same-run navigation must not requery files.");
            folderPage.SetException(new InvalidOperationException("Fictional optional-pane failure"));
            Drain();
            Assert.IsFalse(model.DuplicateFiles.HasError);
            Assert.AreEqual(old.Id, model.SelectedRun?.Id);

            foreach (var size in new[] { new Size(1180, 760), new Size(900, 600) })
            {
                window.Width = size.Width;
                window.Height = size.Height;
                foreach (var destination in new[] { WorkspaceDestination.ScanSetup, WorkspaceDestination.ScanProgress,
                    WorkspaceDestination.FileResults, WorkspaceDestination.Review, WorkspaceDestination.History })
                {
                    model.SelectedDestination = destination;
                    Drain();
                    var context = Find<TextBlock>(window, "SelectedScanContext");
                    Assert.AreEqual(model.SelectedScanContext, context.Text);
                    Assert.IsTrue(context.ActualWidth > 100);
                    Assert.IsTrue(Find<Button>(window, "ViewActiveProgress").IsVisible);
                    Capture(window, $"populated-{destination}-{size.Width}x{size.Height}");
                }
            }
        }
        finally { window.Close(); }
    }

    private static T Find<T>(DependencyObject root, string id) where T : FrameworkElement
    {
        if (root is T match && AutomationProperties.GetAutomationId(match) == id) return match;
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            try { return Find<T>(VisualTreeHelper.GetChild(root, i), id); }
            catch (KeyNotFoundException) { }
        }
        throw new KeyNotFoundException(id);
    }

    private static IEnumerable<T> Descendants<T>(DependencyObject root) where T : DependencyObject
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is T match) yield return match;
            foreach (var descendant in Descendants<T>(child)) yield return descendant;
        }
    }

    private static void Capture(Window window, string name)
    {
        var directory = Environment.GetEnvironmentVariable("SUPER_DUPER_UIR03_CAPTURES");
        if (string.IsNullOrWhiteSpace(directory)) return;
        Directory.CreateDirectory(directory);
        var bitmap = new RenderTargetBitmap((int)window.ActualWidth, (int)window.ActualHeight, 96, 96, PixelFormats.Pbgra32);
        bitmap.Render(window);
        var encoder = new PngBitmapEncoder();
        encoder.Frames.Add(BitmapFrame.Create(bitmap));
        using var output = File.Create(Path.Combine(directory, name + ".png"));
        encoder.Save(output);
    }

    private static void Drain()
    {
        var frame = new DispatcherFrame();
        Dispatcher.CurrentDispatcher.BeginInvoke(DispatcherPriority.ContextIdle, () => frame.Continue = false);
        Dispatcher.PushFrame(frame);
    }
}
