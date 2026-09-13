using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Views;

namespace SuperDuper.Windows.Smoke.Tests;

internal static class FileQueryLayoutFixture
{
    internal static void Verify()
    {
        using var data = new SuperDuper.Windows.Fixtures.ShellFixtureData();
        var shell = data.Model;
        var model = shell.DuplicateFiles;
        var window = new MainWindow(shell, data.Client, ownsWorkerLifetime: false)
        { ShowActivated = false, ShowInTaskbar = false, Left = -10000, Top = -10000 };
        window.SetResourceReference(Window.BackgroundProperty, "ApplicationBackgroundBrush");
        try
        {
            window.Show(); Drain();
            shell.SelectedDestination = WorkspaceDestination.History;
            shell.History.SelectedRun = shell.History.Runs.Single(r => r.Id == data.OldRun.Id);
            shell.OpenScanCommand.Execute(null); Drain();
            var view = (DuplicateFilesView)window.FindName("DuplicateFilesWorkspace");
            var search = (TextBox)view.FindName("FileSearch");
            var filters = (Expander)view.FindName("FileFiltersExpander");
            var scroll = Find<ScrollViewer>(view, "FileWorkspaceScrollViewer");
            var longPath = @"\\?\UNC\fictional\archive\" + string.Concat(Enumerable.Repeat("long-discriminating-folder\\", 30)) + "copy.jpg";
            var handler = data.Client.GroupPageHandler!;
            foreach (var size in new[] { new Size(900, 600), new Size(1180, 760) })
            {
                window.Width = size.Width; window.Height = size.Height; Drain();
                filters.IsExpanded = false;
                Assert.IsTrue(search.Focus());
                search.Text = longPath; model.ExactPathMatch = true;
                search.Select(10, 20);
                search.ScrollToHorizontalOffset(100); Drain();
                var selection = search.SelectionStart;
                var offset = search.HorizontalOffset;
                var oldRows = model.Groups;
                var oldChips = model.AppliedFilters;
                var delayed = new TaskCompletionSource<WorkerDuplicateFileGroupPage>();
                data.Client.GroupPageHandler = (_, _) => delayed.Task;
                // Exercise the actual routed Enter handler, without a physical-keyboard claim.
                var enter = new KeyEventArgs(Keyboard.PrimaryDevice, PresentationSource.FromVisual(search), 0, Key.Enter)
                { RoutedEvent = Keyboard.PreviewKeyDownEvent };
                search.RaiseEvent(enter); Drain();
                Assert.IsTrue(enter.Handled);
                Assert.IsTrue(model.IsLoading);
                Assert.AreSame(oldRows, model.Groups);
                Assert.AreSame(oldChips, model.AppliedFilters);
                Assert.IsTrue(search.IsKeyboardFocused);
                Assert.AreEqual(selection, search.SelectionStart);
                Assert.AreEqual(offset, search.HorizontalOffset, 1d);
                AssertHeaderFits(window, view);
                Capture(window, $"query-pending-{size.Width}");
                var page = handler(new DuplicateFileGroupQuery(data.OldRun.Id, 200,
                    DuplicateFileGroupSortField.RecoverableBytes, WorkerSortDirection.Descending, new("", "0")), default).GetAwaiter().GetResult();
                delayed.SetResult(page);
                WaitUntil(() => !model.ApplyFiltersCommand.IsRunning);
                Assert.IsTrue(search.IsKeyboardFocused);
                Assert.AreEqual(selection, search.SelectionStart);
                Assert.AreEqual(offset, search.HorizontalOffset, 1d);
                Assert.AreEqual(longPath, model.SearchText);
                Assert.AreEqual(data.OldRun.Id, model.Run!.Id);
                Assert.AreEqual(data.ActiveRun.Id, shell.Progress.Run!.Id);
                Assert.AreEqual("25", Find<TextBlock>(view, "FileSummaryMatchingSets").Text);
                Assert.AreEqual("50", Find<TextBlock>(view, "FileSummaryMatchingCopies").Text);
                Assert.IsTrue(model.AppliedFilters.Single().Text.EndsWith(longPath, StringComparison.Ordinal));
                Assert.AreEqual(0, scroll.ScrollableWidth, 0.5,
                    "The bounded advanced-controls region must never introduce page-level horizontal scrolling.");
                AssertHeaderFits(window, view);
                Capture(window, $"query-applied-header-{size.Width}");
                filters.IsExpanded = true;
                Settle(window);
                var unit = (ComboBox)view.FindName("FileSizeUnit");
                unit.BringIntoView(); Drain();
                Assert.IsTrue(unit.Focus());
                unit.SelectedItem = "GiB";
                ((TextBox)view.FindName("FileMinimumSize")).Text = "1.5";
                Drain();
                Assert.AreEqual("GiB", model.MinimumSizeUnit);
                Assert.AreEqual("1.5", model.MinimumSizeText);
                Assert.AreEqual(1, model.AppliedFilters.Count, "Advanced edits remain drafts.");
                scroll.ScrollToBottom(); Drain();
                Assert.IsTrue(scroll.VerticalOffset > 0,
                    "Expanded advanced controls remain reachable through their bounded vertical scroller.");
                Capture(window, $"query-draft-units-{size.Width}");
                unit.RaiseEvent(new KeyEventArgs(Keyboard.PrimaryDevice, PresentationSource.FromVisual(unit), 0, Key.Escape)
                { RoutedEvent = Keyboard.PreviewKeyDownEvent });
                Drain();
                Assert.IsFalse(filters.IsExpanded);
                Assert.IsTrue(((FrameworkElement)view.FindName("FileFiltersToggle")).IsKeyboardFocused);
                data.Client.GroupPageHandler = handler;
                var chip = Descendants<Button>(view).Single(b => b.Tag is FileFilterChip);
                Assert.IsTrue(chip.Focus());
                chip.RaiseEvent(new RoutedEventArgs(System.Windows.Controls.Primitives.ButtonBase.ClickEvent));
                Drain();
                Assert.IsTrue(search.IsKeyboardFocused, "Removing a focused chip returns focus before the async update.");
                Assert.AreEqual("1.5", model.MinimumSizeText, "Removing path must not erase the unrelated size draft.");
                Assert.AreEqual(0, model.AppliedFilters.Count);
                var sort = (ComboBox)view.FindName("FileSetSort");
                sort.SelectedIndex = 5;
                WaitUntil(() => model.SortField == DuplicateFileGroupSortField.CopyCount
                    && model.SortDirection == WorkerSortDirection.Ascending && !model.IsLoading);
                model.ClearFiltersCommand.ExecuteAsync(null).GetAwaiter().GetResult(); Drain();
                Assert.AreEqual(System.ComponentModel.ListSortDirection.Descending,
                    Find<DataGrid>(view, "FileGroupsGrid").Columns.Single(c => c.SortMemberPath == "RecoverableBytes").SortDirection);
                Assert.AreEqual("Potential savings, largest first", ((ComboBoxItem)sort.SelectedItem).Content);
                Settle(window);
                AssertHeaderFits(window, view);
                Capture(window, $"query-default-{size.Width}");
            }
        }
        finally { window.Close(); Drain(); }
    }

    private static void AssertHeaderFits(Window window, DuplicateFilesView view)
    {
        foreach (var id in new[] { "FileSearch", "FileApplyFilters", "FileFiltersToggle", "FileClearFilters",
            "FileSummaryMatchingSets", "FileSummaryMatchingCopies", "FileSummaryRecoverable" })
        {
            var element = Find<FrameworkElement>(view, id);
            var bounds = element.TransformToAncestor(window).TransformBounds(new Rect(element.RenderSize));
            Assert.IsTrue(element.IsVisible && element.ActualWidth > 0 && bounds.Left >= 0
                && bounds.Right <= window.ActualWidth && bounds.Top >= 0 && bounds.Bottom <= window.ActualHeight,
                $"{id} must be visible without scrolling: {bounds}, window {window.RenderSize}");
        }
        Assert.IsTrue(((TextBox)view.FindName("FileSearch")).ActualWidth >= 100, "Path input retains a useful width.");
    }
    private static void WaitUntil(Func<bool> condition)
    {
        var timeout = System.Diagnostics.Stopwatch.StartNew();
        while (!condition() && timeout.Elapsed < TimeSpan.FromSeconds(3)) Drain();
        Assert.IsTrue(condition(), "Delayed query did not settle.");
        Drain();
    }
    private static void Settle(Window window)
    {
        var frame = new DispatcherFrame();
        var timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(500) };
        timer.Tick += (_, _) => { timer.Stop(); window.UpdateLayout(); frame.Continue = false; };
        timer.Start(); Dispatcher.PushFrame(frame);
    }
    private static void Drain()
    {
        var frame = new DispatcherFrame();
        Dispatcher.CurrentDispatcher.BeginInvoke(DispatcherPriority.ContextIdle, () => frame.Continue = false);
        Dispatcher.PushFrame(frame);
    }
    private static T Find<T>(DependencyObject root, string id) where T : DependencyObject =>
        Descendants<T>(root).Single(x => AutomationProperties.GetAutomationId(x) == id);
    private static IEnumerable<T> Descendants<T>(DependencyObject root) where T : DependencyObject
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is T match) yield return match;
            foreach (var item in Descendants<T>(child)) yield return item;
        }
    }
    private static void Capture(Window window, string name)
    {
        var directory = Environment.GetEnvironmentVariable("SUPER_DUPER_UIR05C_CAPTURES");
        if (string.IsNullOrEmpty(directory)) return;
        Directory.CreateDirectory(directory);
        var bitmap = new RenderTargetBitmap((int)window.ActualWidth, (int)window.ActualHeight, 96, 96, PixelFormats.Pbgra32);
        bitmap.Render(window);
        var encoder = new PngBitmapEncoder(); encoder.Frames.Add(BitmapFrame.Create(bitmap));
        using var file = File.Create(Path.Combine(directory, name + ".png")); encoder.Save(file);
    }
}
