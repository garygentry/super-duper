using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using SuperDuper.Windows.Core.Tests;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Views;

namespace SuperDuper.Windows.Smoke.Tests;

internal static class SetupWorkflowFixture
{
    internal static void Verify()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Repeat scan fixture", Path.GetTempPath());
        var old = client.AddRun(session.Id, "completed");
        using var model = new ShellViewModel(client, new TestFolderPicker(), new TestConfirmation(),
            new ImmediateDispatcher(), new TestClipboard(), new TestExplorer(), new TestCloudLocationService());
        model.InitializeAsync().GetAwaiter().GetResult();
        var window = new MainWindow(model, client, ownsWorkerLifetime: false)
        { ShowActivated = false, ShowInTaskbar = false, Left = -10000, Top = -10000 };
        window.SetResourceReference(Window.BackgroundProperty, "ApplicationBackgroundBrush");
        try
        {
            window.Show();
            foreach (var size in new[] { new Size(900, 600), new Size(1180, 760) })
            {
                window.Width = size.Width; window.Height = size.Height;
                model.SelectedDestination = WorkspaceDestination.FileResults;
                Drain();
                model.ScanAgainCommand.Execute(null);
                Drain();
                Assert.AreEqual(WorkspaceDestination.ScanSetup, model.SelectedDestination);
                Assert.IsTrue(((TabItem)((TabControl)window.FindName("ScanTabs")).SelectedItem).IsKeyboardFocused);
                Assert.AreEqual(old, model.SelectedRun);
                var view = Descendants<SessionSetupView>(window).Single();
                var advanced = (Expander)view.FindName("SetupAdvanced");
                Assert.IsFalse(advanced.IsExpanded);
                Capture(window, $"setup-{size.Width}");
                advanced.IsExpanded = true;
                Drain();
                var selector = Find<ComboBox>(window, "RepeatCachePolicy");
                selector.SelectedValue = RepeatCachePolicyNames.RevalidateContent;
                selector.BringIntoView(); Drain();
                Assert.AreEqual(RepeatCachePolicyNames.RevalidateContent, model.Setup.RepeatCachePolicy);
                StringAssert.Contains(model.Setup.RepeatCachePolicyDescription, "normal candidate filtering");
                foreach (var id in new[] { "RepeatCachePolicy", "ManualCloudLocationExclusions", "IgnorePatterns", "StartScanButton" })
                {
                    var control = Find<FrameworkElement>(window, id);
                    control.BringIntoView(); Drain();
                    var bounds = control.TransformToAncestor(window).TransformBounds(new Rect(control.RenderSize));
                    Assert.IsTrue(bounds.Left >= 0 && bounds.Right <= window.ActualWidth && bounds.Top >= 0 && bounds.Bottom <= window.ActualHeight,
                        $"Setup control {id} must be reachable at {size}.");
                }
                Capture(window, $"setup-advanced-{size.Width}");
                model.Setup.Name = "Unsaved fixture";
                ((TabControl)window.FindName("MainTabs")).SelectedValue = WorkspaceArea.History;
                Drain();
                Assert.IsTrue(Find<Button>(window, "StayInSetup").IsKeyboardFocused);
                Assert.AreEqual(WorkspaceDestination.ScanSetup, model.SelectedDestination);
                Assert.AreEqual(WorkspaceArea.Scan, ((TabControl)window.FindName("MainTabs")).SelectedValue);
                Capture(window, $"setup-dirty-{size.Width}");
                model.StayInSetupCommand.Execute(null); Drain();
                Assert.AreEqual("Unsaved fixture", model.Setup.Name);
                ((TabControl)window.FindName("ScanTabs")).SelectedValue = WorkspaceDestination.ScanProgress;
                Drain();
                Assert.IsTrue(model.HasSetupDeparture);
                Assert.AreEqual(WorkspaceDestination.ScanSetup, ((TabControl)window.FindName("ScanTabs")).SelectedValue);
                model.StayInSetupCommand.Execute(null); Drain();
                model.SelectedDestination = WorkspaceDestination.History;
                model.DiscardSetupAndContinueCommand.ExecuteAsync(null).GetAwaiter().GetResult(); Drain();
                Assert.AreEqual("Repeat scan fixture", model.Setup.Name);
                Assert.AreEqual(WorkspaceDestination.History, model.SelectedDestination);
                advanced.IsExpanded = false;
            }
        }
        finally { window.Close(); }
    }

    private static T Find<T>(DependencyObject root, string id) where T : DependencyObject =>
        Descendants<T>(root).Single(item => AutomationProperties.GetAutomationId(item) == id);

    private static IEnumerable<T> Descendants<T>(DependencyObject root) where T : DependencyObject
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is T item) yield return item;
            foreach (var descendant in Descendants<T>(child)) yield return descendant;
        }
    }

    private static void Drain()
    {
        var frame = new DispatcherFrame();
        Dispatcher.CurrentDispatcher.BeginInvoke(DispatcherPriority.ContextIdle, () => frame.Continue = false);
        Dispatcher.PushFrame(frame);
    }

    private static void Capture(Window window, string name)
    {
        var directory = Environment.GetEnvironmentVariable("SUPER_DUPER_UIR04_CAPTURES");
        if (string.IsNullOrWhiteSpace(directory)) return;
        Directory.CreateDirectory(directory);
        var bitmap = new RenderTargetBitmap((int)window.ActualWidth, (int)window.ActualHeight, 96, 96, PixelFormats.Pbgra32);
        bitmap.Render(window);
        var encoder = new PngBitmapEncoder(); encoder.Frames.Add(BitmapFrame.Create(bitmap));
        using var output = File.Create(Path.Combine(directory, name + ".png")); encoder.Save(output);
    }
}
