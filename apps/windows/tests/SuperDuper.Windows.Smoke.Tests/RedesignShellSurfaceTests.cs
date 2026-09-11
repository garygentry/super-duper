using System.ComponentModel;
using System.IO;
using System.Reflection;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Markup;
using System.Windows.Input;
using SuperDuper.Windows.Core.Tests;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using System.Xml.Linq;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Smoke.Tests;

internal static class RedesignShellSurfaceTests
{
    // Reuse the existing themed App and STA; WPF permits one Application per process.
    internal static void Verify()
    {
        Window? window = null;
        try
        {
            // Load the shipping shell markup/resources into an isolated presentation host.
            // No App startup, worker, database, filesystem scan or MainWindow shutdown runs.
            // Existing screen contents have their own loaded-STA regressions; use empty slots here.
            using var source = Assembly.GetExecutingAssembly().GetManifestResourceStream("ShellMarkup.xaml")!;
            var document = XDocument.Load(source);
            XNamespace xaml = "http://schemas.microsoft.com/winfx/2006/xaml";
            XNamespace wpf = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
            var root = document.Root!;
            root.Attribute(xaml + "Class")!.Remove();
            root.Attribute("Icon")!.Remove();
            foreach (var view in root.Descendants().Where(element => element.Name.NamespaceName == "clr-namespace:SuperDuper.Windows.Views"))
            {
                view.Name = wpf + "ContentControl";
            }
            root.AddFirst(new XElement(wpf + "Window.Resources",
                new XElement(wpf + "ResourceDictionary",
                    new XElement(wpf + "ResourceDictionary.MergedDictionaries",
                        new XElement(wpf + "ResourceDictionary", new XAttribute("Source",
                            "/SuperDuper.Windows;component/Resources/ShellResources.xaml"))),
                    new XElement(wpf + "BooleanToVisibilityConverter", new XAttribute(xaml + "Key", "BooleanToVisibilityConverter")))));
            window = (Window)XamlReader.Parse(document.ToString());
            var client = new TestWorkerClient();
            var session = client.AddSession("Family archive", @"C:\fixture\archive");
            var old = client.AddRun(session.Id, "completed");
            client.AddRun(session.Id, "running", "hashing");
            using var fixture = new ShellViewModel(client, new TestFolderPicker(), new TestConfirmation(),
                new ImmediateDispatcher(), new TestClipboard(), new TestExplorer(), new TestCloudLocationService());
            fixture.InitializeAsync().GetAwaiter().GetResult();
            fixture.History.SelectedRun = fixture.History.Runs.Single(run => run.Id == old.Id);
            fixture.OpenScanCommand.Execute(null);
            window.DataContext = fixture;
            window.ShowActivated = false;
            window.ShowInTaskbar = false;
            window.WindowStartupLocation = WindowStartupLocation.Manual;
            window.Left = -10000;
            window.Top = -10000;
            window.Show();
            Drain();

            var tabs = (TabControl)window.FindName("MainTabs");
            var review = tabs.Items.Cast<TabItem>().Single(item => Equals(item.Tag, WorkspaceArea.Review));
            tabs.Items.Remove(review);
            tabs.Items.Insert(0, review);
            fixture.SelectedDestination = WorkspaceDestination.Review;
            Drain();
            Assert.AreSame(review, tabs.SelectedItem);
            Assert.AreEqual(4, tabs.Items.Count);
            tabs.SelectedItem = tabs.Items.Cast<TabItem>().Single(item => Equals(item.Tag, WorkspaceArea.Results));
            Assert.AreEqual(WorkspaceDestination.FileResults, fixture.SelectedDestination);

            Drain();
            var results = (TabControl)window.FindName("ResultsTabs");
            results.SelectedValue = WorkspaceDestination.FolderResults;
            Assert.AreEqual(WorkspaceDestination.FolderResults, fixture.SelectedDestination);
            fixture.SelectedArea = WorkspaceArea.History;
            Drain();
            var open = Find<Button>(window, "OpenScan");
            Assert.IsTrue(open.IsEnabled);
            FocusManager.SetFocusedElement(window, open);
            Assert.AreSame(open, FocusManager.GetFocusedElement(window));
            open.Command.Execute(null);
            Drain();
            Assert.AreEqual(WorkspaceArea.Results, fixture.SelectedArea);
            Assert.AreEqual(WorkspaceDestination.FileResults, results.SelectedValue);
            var fileTab = (TabItem)results.SelectedItem;
            FocusManager.SetFocusedElement(window, fileTab);
            Assert.AreSame(fileTab, FocusManager.GetFocusedElement(window));
            Assert.IsTrue(fileTab.IsVisible);
            Assert.IsTrue(fileTab.Focusable);

            var context = Find<TextBlock>(window, "SelectedScanContext");
            Assert.AreEqual(fixture.SelectedScanContext, context.Text);
            Assert.IsFalse(context.Text.Contains("\u00c2"), "Context separators must remain correctly encoded.");
            Assert.IsTrue(context.IsVisible);
            var progress = Find<Button>(window, "ViewActiveProgress");
            Assert.IsTrue(progress.IsVisible);
            Assert.IsTrue(progress.Focusable);
            Assert.AreEqual("View active scan progress", AutomationProperties.GetName(progress));
            Assert.IsNotNull(progress.Style);
            Assert.IsNotNull(progress.FocusVisualStyle);
            progress.Command.Execute(null);
            Assert.AreEqual(WorkspaceDestination.ScanProgress, fixture.SelectedDestination);
            Drain();
            Assert.AreEqual(fixture.ProgressScanContext, Find<TextBlock>(window, "ProgressScanContext").Text);

            tabs.Items.Remove(review);
            tabs.Items.Insert(2, review);
            fixture.SelectedDestination = WorkspaceDestination.ScanSetup;
            Drain();
            foreach (var size in new[] { new Size(1180, 760), new Size(900, 600) })
            {
                window.Width = size.Width;
                window.Height = size.Height;
                window.UpdateLayout();
                Assert.IsTrue(context.ActualWidth > 100);
                Assert.IsTrue(progress.ActualWidth + progress.Margin.Left + progress.Margin.Right >= progress.DesiredSize.Width - 1);
                var captures = Environment.GetEnvironmentVariable("SUPER_DUPER_UIR03_CAPTURES");
                if (!string.IsNullOrWhiteSpace(captures))
                {
                    Directory.CreateDirectory(captures);
                    var bitmap = new RenderTargetBitmap((int)window.ActualWidth, (int)window.ActualHeight, 96, 96, PixelFormats.Pbgra32);
                    bitmap.Render(window);
                    var encoder = new PngBitmapEncoder();
                    encoder.Frames.Add(BitmapFrame.Create(bitmap));
                    using var output = File.Create(Path.Combine(captures, $"shell-{size.Width}x{size.Height}.png"));
                    encoder.Save(output);
                }
            }
        }
        finally { window?.Close(); }
    }

    private static T Find<T>(DependencyObject root, string id) where T : FrameworkElement
    {
        if (root is T match && AutomationProperties.GetAutomationId(match) == id) return match;
        for (var index = 0; index < VisualTreeHelper.GetChildrenCount(root); index++)
        {
            try { return Find<T>(VisualTreeHelper.GetChild(root, index), id); }
            catch (KeyNotFoundException) { }
        }
        throw new KeyNotFoundException(id);
    }

    private static void Drain()
    {
        var frame = new DispatcherFrame();
        Dispatcher.CurrentDispatcher.BeginInvoke(DispatcherPriority.ContextIdle, () => frame.Continue = false);
        Dispatcher.PushFrame(frame);
    }

}
