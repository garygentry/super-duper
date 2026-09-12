using System.Windows;
using System.Windows.Automation;
using System.Windows.Automation.Peers;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using SuperDuper.Windows.Accessibility;
using SuperDuper.Windows.Core.Tests;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Views;

namespace SuperDuper.Windows.Smoke.Tests;

internal static class LongScanMonitoringFixture
{
    internal static void Verify()
    {
        var clock = new ManualProgressClock();
        using var model = new ScanProgressViewModel(new TestWorkerClient(), new ImmediateDispatcher(), clock: clock);
        var view = new ScanProgressView { DataContext = model };
        var window = new Window { Content = view, Width = 900, Height = 600,
            ShowActivated = false, ShowInTaskbar = false, Left = -10000, Top = -10000 };
        window.SetResourceReference(Window.BackgroundProperty, "ApplicationBackgroundBrush");
        var notifications = new List<string>();
        void Notification(FrameworkElement element, string text, AutomationNotificationKind kind,
            AutomationNotificationProcessing processing, string activity)
        {
            if (element == Find<TextBlock>(view, "ScanProgressAnnouncement"))
            {
                Assert.AreEqual(AutomationNotificationProcessing.MostRecent, processing);
                notifications.Add(text);
            }
        }
        try
        {
            window.Show(); Drain();
            Assert.AreSame(DependencyProperty.UnsetValue,
                Find<ProgressBar>(view, "ScanProgressBar").ReadLocalValue(FrameworkElement.StyleProperty),
                "Terminal visibility must retain the native Fluent progress template.");
            AutomationNotificationBehavior.NotificationRaised += Notification;
            VerifyCompactSummary(window, view, model, clock);
            foreach (var size in new[] { new Size(900, 600), new Size(1180, 760) })
            {
                window.Width = size.Width; window.Height = size.Height;
                foreach (var status in new[] { "completed", "cancelled", "failed", "interrupted" })
                {
                    var run = TestWorkerClient.CreateRun(1, 1, "running", "discovering", clock.GetUtcNow());
                    model.ShowRun(run); Drain();
                    Assert.IsTrue(model.ApplyProgress(ProgressTestData.Discovery(discoveredFiles: 10)));
                    Drain();
                    var cancel = Find<Button>(view, "CancelScanButton");
                    cancel.BringIntoView(); Drain();
                    Assert.IsTrue(cancel.Focus());
                    var count = notifications.Count;
                    clock.Advance(TimeSpan.FromDays(2) + TimeSpan.FromHours(7) + TimeSpan.FromMinutes(14));
                    Drain();
                    Assert.AreEqual("2d 7h 14m", Find<TextBlock>(view, "ScanRunElapsed").Text);
                    StringAssert.Contains(Find<TextBlock>(view, "ScanUpdateFreshness").Text, "does not indicate failure");
                    Assert.AreEqual(count, notifications.Count, "Clock-only updates stay silent.");
                    Assert.IsTrue(cancel.IsKeyboardFocused);
                    Assert.IsTrue(Find<ProgressBar>(view, "ScanProgressBar").IsIndeterminate);
                    AssertReachable(window, Find<TextBlock>(view, "ScanUpdateFreshness"));
                    Capture(window, $"stale-{status}-{size.Width}");

                    // A loaded view restored after several updates sees the latest state, without
                    // replaying intermediate announcements or moving focus to the activity path.
                    window.WindowState = WindowState.Minimized;
                    Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(sequence: 2, revision: 2)));
                    Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(sequence: 3, revision: 3)));
                    window.WindowState = WindowState.Normal;
                    Drain();
                    Assert.AreEqual(3UL, model.ProgressSnapshot!.Revision);
                    Assert.AreEqual(6, Find<ItemsControl>(view, "ScanProgressFunnel").Items.Count);
                    StringAssert.Contains(Find<TextBlock>(view, "ScanEstimatedTimeRemaining").Text, "Hash pipeline");
                    Find<Expander>(view, "ScanWorkExpander").IsExpanded = true;
                    Find<Expander>(view, "ScanDiagnosticsExpander").IsExpanded = true;
                    SettleLayout(window);
                    var path = Find<TextBox>(view, "ScanCurrentPath");
                    path.BringIntoView(); Drain();
                    Assert.IsTrue(path.Focus());
                    path.SelectAll();
                    var offset = Find<ScrollViewer>(view, "ScanProgressScrollViewer").VerticalOffset;
                    count = notifications.Count;
                    model.ApplyProgress(ProgressTestData.Hashing(sequence: 4, revision: 4));
                    Drain();
                    Assert.AreEqual(count, notifications.Count);
                    Assert.IsTrue(path.IsKeyboardFocused);
                    Assert.AreEqual(path.Text.Length, path.SelectionLength);
                    Assert.AreEqual(offset, Find<ScrollViewer>(view, "ScanProgressScrollViewer").VerticalOffset);
                    model.ApplyLifecycle(model.Run! with { Status = status, CompletedAt = clock.GetUtcNow(),
                        ErrorMessage = status == "failed" ? "Controlled fixture failure" : null });
                    Drain();
                    Assert.AreEqual(count + 1, notifications.Count);
                    StringAssert.Contains(notifications[^1], model.Status);
                    Assert.IsTrue(path.IsKeyboardFocused, "Completion must not steal focus from the inspected path.");
                    Assert.AreEqual("Last reported scan path", AutomationProperties.GetName(path));
                    Assert.AreEqual("Last reported activity", Find<TextBlock>(view, "ScanActivityHeading").Text);
                    Assert.IsFalse(Find<ProgressBar>(view, "ScanProgressBar").IsIndeterminate);
                    Assert.IsFalse(Find<ProgressBar>(view, "ScanProgressBar").IsVisible);
                    Assert.IsFalse(cancel.IsEnabled);
                    Capture(window, $"terminal-activity-{status}-{size.Width}");
                    if (status == "failed")
                    {
                        var error = Find<Border>(view, "ScanProgressError");
                        AssertReachable(window, error);
                        StringAssert.Contains(AutomationProperties.GetName(error), "Controlled fixture failure");
                        Capture(window, $"terminal-error-{size.Width}");
                    }
                    var elapsed = model.Elapsed;
                    clock.Advance(TimeSpan.FromDays(1)); Drain();
                    Assert.AreEqual(elapsed, model.Elapsed);
                    Find<ScrollViewer>(view, "ScanProgressScrollViewer").ScrollToTop(); Drain();
                    StringAssert.Contains(Find<TextBlock>(view, "ScanMetricsContext").Text, "Historical metrics");
                    AssertReachable(window, Find<TextBlock>(view, "ScanMetricsContext"));
                    Capture(window, $"terminal-summary-{status}-{size.Width}");
                }
            }
        }
        finally
        {
            AutomationNotificationBehavior.NotificationRaised -= Notification;
            window.Close();
        }
    }

    private static void VerifyCompactSummary(Window window, ScanProgressView view,
        ScanProgressViewModel model, ManualProgressClock clock)
    {
        var pathText = @"\\?\UNC\server\share\" + string.Concat(Enumerable.Repeat("long parent location ", 20))
            + @"\" + new string('f', 180) + ".bin";
        foreach (var size in new[] { new Size(900, 600), new Size(1180, 760) })
        {
            window.Width = size.Width; window.Height = size.Height;
            var work = Find<Expander>(view, "ScanWorkExpander");
            var reuse = Find<Expander>(view, "ScanHashReuseExpander");
            var diagnostics = Find<Expander>(view, "ScanDiagnosticsExpander");
            work.IsExpanded = reuse.IsExpanded = diagnostics.IsExpanded = false;
            SettleLayout(window);
            foreach (var phase in new[] { "discovering", "hashing", "zero-hash", "unknown-hash", "unknown-folder",
                "hierarchy", "structural_candidates", "verification", "persistence", "zero-folder", "finalizing" })
            {
                model.ShowRun(TestWorkerClient.CreateRun(1, 1, "running", "discovering", clock.GetUtcNow()));
                WorkerRunProgressEventArgs progress = phase switch
                {
                    "discovering" => ProgressTestData.Discovery(discoveredFiles: 3_547_188),
                    "hashing" => ProgressTestData.Hashing(currentPath: pathText),
                    "zero-hash" => ProgressTestData.Hashing(measuredCounters: new(), measuredLogical: new()),
                    "unknown-hash" => ProgressTestData.Hashing(measuredCounters: new(), measuredLogical: new(), candidateTotalsKnown: false),
                    "unknown-folder" => ProgressTestData.Hashing(legacyPhase: "analyzing_folders", typedPhase: "analyzing_folders", etaUnavailableReason: "not_applicable"),
                    "finalizing" => ProgressTestData.Hashing(legacyPhase: "finalizing", typedPhase: "finalizing", etaUnavailableReason: "not_applicable"),
                    _ => ProgressTestData.Hashing(legacyPhase: "analyzing_folders", typedPhase: "analyzing_folders",
                        etaUnavailableReason: "not_applicable", folderAnalysis: new()
                        {
                            Substage = phase == "zero-folder" ? "persistence" : phase,
                            Completed = phase == "zero-folder" ? 0UL : 1_000_000UL,
                            Total = phase == "zero-folder" ? 0UL : 4_000_000UL,
                        }),
                };
                Assert.IsTrue(model.ApplyProgress(progress));
                Drain();
                var scroll = Find<ScrollViewer>(view, "ScanProgressScrollViewer");
                scroll.ScrollToTop(); Drain();
                var bar = Find<ProgressBar>(view, "ScanProgressBar");
                Assert.AreEqual(model.PhaseWork.BarValue, bar.Value);
                Assert.AreEqual(model.IsIndeterminate, bar.IsIndeterminate);
                Assert.AreEqual(model.PhaseWork.AutomationName, AutomationProperties.GetName(bar));
                Assert.IsTrue(bar.IsVisible);
                Assert.IsFalse(work.IsExpanded || reuse.IsExpanded || diagnostics.IsExpanded);
                foreach (var id in new[] { "ScanActivityFileName", "ScanActivityParent", "ScanPhaseWorkDetail", "ScanMetricsContext" })
                {
                    var element = Find<TextBlock>(view, id);
                    var bounds = element.TransformToAncestor(window).TransformBounds(new Rect(element.RenderSize));
                    Assert.IsTrue(bounds.Left >= 0 && bounds.Right <= window.ActualWidth && bounds.Bottom <= window.ActualHeight,
                        $"Compact summary {id} must fit at {size} during {phase}: {bounds}");
                }
                Capture(window, $"compact-{phase}-{size.Width}");
            }
            model.ApplyLifecycle(model.Run! with { Status = "completed", CompletedAt = clock.GetUtcNow() });
            Drain();
            Assert.IsFalse(Find<ProgressBar>(view, "ScanProgressBar").IsVisible);
            StringAssert.Contains(Find<TextBlock>(view, "ScanPhaseWorkDetail").Text, "Historical phase work");
            Capture(window, $"compact-terminal-unknown-{size.Width}");

            model.ShowRun(TestWorkerClient.CreateRun(1, 1, "running", "hashing", clock.GetUtcNow()));
            Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(currentPath: pathText)));
            work.IsExpanded = reuse.IsExpanded = diagnostics.IsExpanded = true;
            SettleLayout(window);
            Assert.AreEqual(6, Find<ItemsControl>(view, "ScanProgressFunnel").Items.Count);
            StringAssert.Contains(Find<TextBlock>(view, "ScanExactHashWork").Text, "4000 of 8000");
            StringAssert.Contains(Find<TextBlock>(view, "ScanPartialReadBytes").Text, "400 B actually read");
            StringAssert.Contains(Find<TextBlock>(view, "ScanFullCacheOutcomes").Text, "Hits 1");
            var path = Find<TextBox>(view, "ScanCurrentPath");
            path.BringIntoView(); Drain();
            Assert.IsTrue(path.Focus());
            path.Select(10, 30);
            path.ScrollToHorizontalOffset(100);
            Drain();
            var offset = Find<ScrollViewer>(view, "ScanProgressScrollViewer").VerticalOffset;
            var horizontalOffset = path.HorizontalOffset;
            Assert.IsTrue(horizontalOffset > 0, "Exercise the exact long path's horizontal scroll.");
            Assert.IsTrue(model.ApplyProgress(ProgressTestData.Hashing(sequence: 2, revision: 2, currentPath: pathText)));
            clock.Advance(TimeSpan.FromSeconds(3)); Drain();
            Assert.IsTrue(work.IsExpanded && reuse.IsExpanded && diagnostics.IsExpanded);
            Assert.IsTrue(path.IsKeyboardFocused);
            Assert.AreEqual(10, path.SelectionStart);
            Assert.AreEqual(30, path.SelectionLength);
            Assert.AreEqual(offset, Find<ScrollViewer>(view, "ScanProgressScrollViewer").VerticalOffset);
            Assert.AreEqual(horizontalOffset, path.HorizontalOffset);
            Assert.AreEqual(pathText, path.Text);
            AssertReachable(window, path);
            Capture(window, $"compact-exact-details-{size.Width}");
            foreach (var id in new[] { "ScanPartialRecentRate", "ScanFullCumulativeRate", "ScanFullCacheOutcomes" })
            {
                AssertReachable(window, Find<TextBlock>(view, id));
                Capture(window, $"compact-details-{id}-{size.Width}");
            }
        }
    }

    private static void SettleLayout(Window window)
    {
        // Fluent disclosure animation continues after a dispatcher drain. Capture and compare
        // offsets only after its geometry settles; never treat an intermediate frame as layout.
        var frame = new DispatcherFrame();
        var elapsed = System.Diagnostics.Stopwatch.StartNew();
        var stableSince = TimeSpan.Zero;
        string? previous = null;
        var timer = new DispatcherTimer(DispatcherPriority.ContextIdle) { Interval = TimeSpan.FromMilliseconds(50) };
        timer.Tick += (_, _) =>
        {
            window.UpdateLayout();
            var geometry = string.Join(";", Descendants<ScrollViewer>(window)
                .Select(scroll => $"{scroll.ExtentHeight:F2}/{scroll.ViewportHeight:F2}"));
            if (geometry != previous) { previous = geometry; stableSince = elapsed.Elapsed; }
            if (elapsed.Elapsed - stableSince < TimeSpan.FromMilliseconds(250)
                && elapsed.Elapsed < TimeSpan.FromSeconds(3)) return;
            timer.Stop();
            frame.Continue = false;
        };
        timer.Start();
        Dispatcher.PushFrame(frame);
        Assert.IsTrue(elapsed.Elapsed < TimeSpan.FromSeconds(3), "Scan disclosure layout did not settle.");
    }

    private static void AssertReachable(Window window, FrameworkElement control)
    {
        control.BringIntoView(); Drain();
        var bounds = control.TransformToAncestor(window).TransformBounds(new Rect(control.RenderSize));
        Assert.IsTrue(bounds.Left >= 0 && bounds.Right <= window.ActualWidth && bounds.Top >= 0 && bounds.Bottom <= window.ActualHeight,
            $"{AutomationProperties.GetAutomationId(control)} must remain reachable.");
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
        var directory = Environment.GetEnvironmentVariable("SUPER_DUPER_UIR04B_CAPTURES");
        if (string.IsNullOrWhiteSpace(directory)) return;
        Directory.CreateDirectory(directory);
        var bitmap = new RenderTargetBitmap((int)window.ActualWidth, (int)window.ActualHeight, 96, 96, PixelFormats.Pbgra32);
        bitmap.Render(window);
        var encoder = new PngBitmapEncoder(); encoder.Frames.Add(BitmapFrame.Create(bitmap));
        using var output = File.Create(Path.Combine(directory, name + ".png")); encoder.Save(output);
    }
}
