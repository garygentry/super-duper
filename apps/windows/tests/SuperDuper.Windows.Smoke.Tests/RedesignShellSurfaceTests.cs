using System.ComponentModel;
using System.IO;
using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using System.Xml.Linq;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Smoke.Tests;

[TestClass]
public sealed class RedesignShellSurfaceTests
{
    [TestMethod]
    public void LoadedShellKeepsContextVisibleAndRoutesByDestinationAfterTabReordering()
    {
        Exception? failure = null;
        var thread = new Thread(() =>
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
                var fixture = new ShellFixture();
                window.DataContext = fixture;
                window.ShowActivated = false;
                window.ShowInTaskbar = false;
                window.WindowStartupLocation = WindowStartupLocation.Manual;
                window.Left = -10000;
                window.Top = -10000;
                window.Show();
                Drain();

                var tabs = (TabControl)window.FindName("MainTabs");
                var review = tabs.Items.Cast<TabItem>().Single(item => Equals(item.Tag, WorkspaceDestination.Review));
                tabs.Items.Remove(review);
                tabs.Items.Insert(0, review);
                fixture.SelectedDestination = WorkspaceDestination.Review;
                Drain();
                Assert.AreSame(review, tabs.SelectedItem);
                tabs.SelectedItem = tabs.Items.Cast<TabItem>().Single(item => Equals(item.Tag, WorkspaceDestination.FileResults));
                Assert.AreEqual(WorkspaceDestination.FileResults, fixture.SelectedDestination);

                var context = Find<TextBlock>(window, "SelectedScanContext");
                Assert.AreEqual(fixture.SelectedScanContext, context.Text);
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
            catch (Exception exception) { failure = exception; }
            finally { window?.Close(); }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.IsTrue(thread.Join(TimeSpan.FromSeconds(30)), "Shell fixture timed out.");
        if (failure is not null) ExceptionDispatchInfo.Capture(failure).Throw();
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

    private sealed class ShellFixture : INotifyPropertyChanged
    {
        private WorkspaceDestination _destination;
        public event PropertyChangedEventHandler? PropertyChanged;
        public WorkspaceDestination SelectedDestination
        {
            get => _destination;
            set { _destination = value; PropertyChanged?.Invoke(this, new(nameof(SelectedDestination))); }
        }
        public bool IsConnected => true;
        public bool IsWorkspaceVisible => true;
        public bool HasActiveRun => true;
        public bool IsEmptyState => false;
        public bool IsLoadingSession => false;
        public bool IsSetupAvailable => true;
        public bool HasContentError => false;
        public bool IsStarting => false;
        public bool IsRecoveryScreenVisible => false;
        public string DisplaySessionName => "Family archive";
        public string SelectedScanContext => "Scan 12 · 9/7/2026 8:30 AM · Completed · 2 locations";
        public string ActiveScanName => "Backup drives";
        public string ProgressScanContext => "Backup drives · Scan 13 · 9/8/2026 8:30 AM · Scanning";
        public IRelayCommand StartRunCommand { get; } = new RelayCommand(() => { }, () => false);
        public IRelayCommand ViewProgressCommand => new RelayCommand(() => SelectedDestination = WorkspaceDestination.ScanProgress);
        public object Progress => new { Phase = "Hashing", Elapsed = "02:14:07", WarningCount = "4" };
        public string StatusTitle => "Hashing";
        public string StatusDetail => "Backup drives is scanning";
    }
}
