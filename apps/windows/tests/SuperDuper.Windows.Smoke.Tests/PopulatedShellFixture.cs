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
                VerifyScanScrollClearance(window, model, size);
                VerifyViewportAccess(window, model, size);
            }
            // The standalone fixture adds a wrapping toolbar above the shipping content.
            // Reserve 80 DIPs at minimum size to cover that host's extra vertical overhead.
            ((FrameworkElement)window.Content).Margin = new Thickness(0, 80, 0, 0);
            Drain();
            VerifyScanScrollClearance(window, model, new Size(900, 600), "-toolbar");
            VerifyViewportAccess(window, model, new Size(900, 600), "-toolbar");
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

    private static void VerifyScanScrollClearance(MainWindow window, ShellViewModel model, Size size, string host = "")
    {
        foreach (var destination in new[] { WorkspaceDestination.ScanProgress, WorkspaceDestination.ScanSummary })
        {
            model.SelectedDestination = destination;
            Drain();
            var scroll = Find<ScrollViewer>(window, "ScanProgressScrollViewer");
            var content = (FrameworkElement)scroll.Content;
            var bar = Descendants<System.Windows.Controls.Primitives.ScrollBar>(scroll)
                .Single(candidate => candidate.Orientation == Orientation.Vertical
                    && ReferenceEquals(candidate.TemplatedParent, scroll));
            Assert.IsTrue(bar.IsVisible && bar.ActualWidth > 0 && scroll.ScrollableHeight > 0,
                $"{destination}: populated scan content must exercise the outer vertical scrollbar.");
            foreach (var bottom in new[] { false, true })
            {
                if (bottom) scroll.ScrollToBottom(); else scroll.ScrollToTop();
                Drain();
                var contentBounds = content.TransformToAncestor(window).TransformBounds(new Rect(content.RenderSize));
                var barBounds = bar.TransformToAncestor(window).TransformBounds(new Rect(bar.RenderSize));
                Capture(window, $"scan-clearance-{destination}-{size.Width}x{size.Height}{host}-{(bottom ? "bottom" : "top")}");
                Assert.IsTrue(contentBounds.Right <= barBounds.Left,
                    $"{destination} {size.Width}x{size.Height}{host}: outer scrollbar overlaps body: content={contentBounds}, scrollbar={barBounds}.");
            }
        }
    }

    private static void VerifyViewportAccess(MainWindow window, ShellViewModel model, Size size, string host = "")
    {
        var suffix = $"{size.Width}x{size.Height}{host}";
        model.SelectedDestination = WorkspaceDestination.FileResults;
        Drain();
        var page = Find<ScrollViewer>(window, "FileWorkspaceScrollViewer");
        var groups = Find<DataGrid>(window, "FileGroupsGrid");
        var members = Find<DataGrid>(window, "FileMembersGrid");
        Assert.IsTrue(model.DuplicateFiles.Members.All(member =>
            !string.IsNullOrWhiteSpace(member.SelectedRoot) && !string.IsNullOrWhiteSpace(member.RelativePath)),
            "The populated viewport fixture must display meaningful locations and paths.");
        foreach (var expanded in new[] { false, true })
        {
            Find<Expander>(window, "FileFiltersExpander").IsExpanded = expanded;
            Find<Expander>(window, "FileTotalsExpander").IsExpanded = expanded;
            Drain();
            foreach (var grid in new[] { groups, members })
            {
                Assert.IsTrue(grid.ActualHeight >= 180 && grid.ActualHeight <= 301,
                    $"{suffix}: bounded {grid.Name} must have a usable viewport even with disclosures open.");
                grid.ScrollIntoView(grid.Items[0], grid.Columns[0]);
                Drain();
                var row = (DataGridRow)grid.ItemContainerGenerator.ContainerFromIndex(0);
                Reach(row, window);
            }
            Assert.IsTrue(Descendants<DataGridRow>(groups).Count() < groups.Items.Count,
                "The outer page scroll must not realize the whole bound group page.");
            foreach (var id in new[] { "FilePreviousGroupPage", "FileNextGroupPage", "FileClearFilters",
                "FilePreviousSet", "FileNextSet", "FileValidateVisiblePage", "FileCancelValidation",
                "FilePreviousMemberPage", "FileNextMemberPage" }) Reach(Find<Button>(window, id), window);
        }
        Find<Expander>(window, "FileFiltersExpander").IsExpanded = false;
        Find<Expander>(window, "FileTotalsExpander").IsExpanded = false;
        SettleLayout(window);

        // Existing technical columns still scroll horizontally until UIR-05. Each complete
        // decision/path action must fit its cell and be reachable, not merely exist in the tree.
        foreach (var header in new[] { "Review decision", "Actions" })
        {
            var column = members.Columns.Single(column => Equals(column.Header, header));
            members.ScrollIntoView(members.Items[0], column);
            Drain();
            var cellContent = column.GetCellContent(members.Items[0]);
            foreach (var button in Descendants<Button>(cellContent)) Reach(button, window);
            Capture(window, $"populated-File-{header.Replace(' ', '-')}-{suffix}");
        }

        // Invoke the actual Click handler. Its focus restoration must also reveal the selected row.
        Find<Button>(window, "FileNextSet").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        Drain();
        Assert.IsTrue(groups.IsKeyboardFocusWithin);
        var selected = model.DuplicateFiles.SelectedGroup;
        var selectedRow = (DataGridRow)groups.ItemContainerGenerator.ContainerFromItem(groups.SelectedItem);
        AssertVisible(selectedRow, window);
        Reach(Find<Button>(window, "FileValidateVisiblePage"), window);
        Find<Button>(window, "FileValidateVisiblePage").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        Drain();
        Assert.IsTrue(members.IsKeyboardFocusWithin, "Validation returns through the actual member-focus handler.");
        AssertVisible(members, window, minimumHeight: 36);

        // Retain both the page's position and the grid's own nonzero offset in this same run.
        var groupScroll = Descendants<ScrollViewer>(groups).First();
        groupScroll.ScrollToVerticalOffset(10);
        page.ScrollToBottom();
        Drain();
        var pageOffset = page.VerticalOffset;
        var gridOffset = groupScroll.VerticalOffset;
        Console.WriteLine($"{suffix} before retention: page={pageOffset}/{page.ExtentHeight}/{page.ViewportHeight}, grids={groups.ActualHeight}/{members.ActualHeight}");
        Assert.IsTrue(pageOffset > 0 && gridOffset > 0);
        model.SelectedDestination = WorkspaceDestination.Review;
        Drain();
        model.SelectedDestination = WorkspaceDestination.FileResults;
        Drain();
        Assert.AreSame(selected, model.DuplicateFiles.SelectedGroup);
        Console.WriteLine($"{suffix} after retention: page={page.VerticalOffset}/{page.ExtentHeight}/{page.ViewportHeight}, grids={groups.ActualHeight}/{members.ActualHeight}");
        Assert.AreEqual(pageOffset, page.VerticalOffset, 1d);
        Assert.AreEqual(gridOffset, groupScroll.VerticalOffset, 1d);
        members.ScrollIntoView(members.Items[0], members.Columns[0]);
        Reach((DataGridRow)members.ItemContainerGenerator.ContainerFromIndex(0), window);
        Capture(window, $"populated-File-comparison-{suffix}");

        model.SelectedDestination = WorkspaceDestination.History;
        Drain();
        var history = Find<DataGrid>(window, "RunHistoryGrid");
        Reach(Find<Button>(window, "OpenScan"), window);
        Reach(Find<Button>(window, "OpenRunWarnings"), window);
        model.History.OpenWarningsCommand.ExecuteAsync(null).GetAwaiter().GetResult();
        Drain();
        var warnings = Find<DataGrid>(window, "RunWarningGrid");
        Assert.IsTrue(warnings.IsKeyboardFocusWithin);
        AssertVisible(warnings, window, minimumHeight: 36);
        Assert.IsTrue(history.ActualHeight >= 144 && warnings.ActualHeight >= 144);
        var warningActionColumn = warnings.Columns.Single(column => Equals(column.Header, "Action"));
        warnings.ScrollIntoView(warnings.Items[0], warningActionColumn);
        Drain();
        Reach(Find<Button>(window, "RunWarningHashResults-1"), window);
        Capture(window, $"populated-History-warning-action-{suffix}");
        Reach(Find<Button>(window, "CloseRunWarnings"), window);
        Capture(window, $"populated-History-warnings-{suffix}");
        Reach(Find<Button>(window, "NextRunWarningPage"), window);
        Reach(Find<Button>(window, "CancelRunWarningLoad"), window);
        model.History.CloseWarningsCommand.Execute(null);
        Drain();
        Assert.IsTrue(history.IsKeyboardFocusWithin);
        AssertVisible(history, window, minimumHeight: 36);
    }

    private static void Reach(FrameworkElement element, Window window)
    {
        element.BringIntoView();
        Drain();
        AssertVisible(element, window);
    }

    private static void SettleLayout(Window window)
    {
        // Native disclosure/scroll layout can span render frames. Retention is measured after
        // the disclosure transition, rather than comparing a transient expanded extent with
        // the final collapsed extent after tab navigation. This does not relax offset assertions.
        var frame = new DispatcherFrame();
        var started = System.Diagnostics.Stopwatch.StartNew();
        var stableSince = TimeSpan.Zero;
        string? previous = null;
        var timer = new DispatcherTimer(DispatcherPriority.ContextIdle) { Interval = TimeSpan.FromMilliseconds(50) };
        timer.Tick += (_, _) =>
        {
            window.UpdateLayout();
            var dimensions = string.Join(";", Descendants<ScrollViewer>(window)
                .Select(scroll => $"{scroll.ExtentHeight:F2}/{scroll.ViewportHeight:F2}"));
            if (dimensions != previous) { previous = dimensions; stableSince = started.Elapsed; }
            if (started.Elapsed - stableSince < TimeSpan.FromMilliseconds(250)
                && started.Elapsed < TimeSpan.FromSeconds(3)) return;
            timer.Stop();
            frame.Continue = false;
        };
        timer.Start();
        Dispatcher.PushFrame(frame);
        Assert.IsTrue(started.Elapsed < TimeSpan.FromSeconds(3), "Disclosure layout did not settle.");
    }

    private static void AssertVisible(FrameworkElement element, Window window, double? minimumHeight = null)
    {
        var bounds = element.TransformToAncestor(window).TransformBounds(new Rect(element.RenderSize));
        var visible = Rect.Intersect(bounds, new Rect(window.RenderSize));
        for (DependencyObject? parent = VisualTreeHelper.GetParent(element); parent is not null && parent != window;
             parent = VisualTreeHelper.GetParent(parent))
        {
            if (parent is UIElement { ClipToBounds: true } clip)
                visible.Intersect(clip.TransformToAncestor(window).TransformBounds(new Rect(clip.RenderSize)));
        }
        // Rows may extend across technical columns; individual actions must fit completely.
        var width = element is DataGridRow or DataGrid ? Math.Min(100, bounds.Width) : bounds.Width;
        Assert.IsTrue(!visible.IsEmpty && visible.Width >= width - 1
            && visible.Height >= (minimumHeight ?? bounds.Height) - 1,
            $"{element.GetType().Name} {element.Name} is clipped: bounds={bounds}, visible={visible}.");
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
