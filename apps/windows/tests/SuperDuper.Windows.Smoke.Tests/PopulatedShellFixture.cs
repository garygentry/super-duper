using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Tests;

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
        var textScale = new FixtureTextScale();
        var window = new MainWindow(model, client, ownsWorkerLifetime: false, textScale)
        {
            ShowActivated = false, ShowInTaskbar = false,
            WindowStartupLocation = WindowStartupLocation.Manual, Left = -10000, Top = -10000,
        };
        // RenderTargetBitmap does not include the native window background. Use an explicit
        // native Fluent brush in this offscreen host, including live theme changes.
        window.SetResourceReference(Window.BackgroundProperty, "ApplicationBackgroundBrush");
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
            Find<Expander>(window, "ScanWorkExpander").IsExpanded = true;
            Find<Expander>(window, "ScanDiagnosticsExpander").IsExpanded = true;
            Drain();
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
            Assert.IsTrue(Find<Expander>(window, "ScanWorkExpander").IsExpanded);
            Assert.IsTrue(Find<Expander>(window, "ScanDiagnosticsExpander").IsExpanded);

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
            ConfigurePopulatedFolderResults(client, model, old);

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
                    if (destination == WorkspaceDestination.Review)
                    {
                        Find<ScrollViewer>(window, "ReviewWorkspaceScrollViewer").ScrollToTop();
                        Drain();
                        AssertVisible(Find<TextBlock>(window, "ReviewHeading"), window);
                    }
                    Capture(window, $"populated-{destination}-{size.Width}x{size.Height}");
                }
                VerifyScanScrollClearance(window, model, size);
                VerifyViewportAccess(window, model, size);
                VerifyFolderViewportAccess(window, model, size);
                VerifyReviewViewportAccess(window, model, size);
            }
            // The standalone fixture adds a wrapping toolbar above the shipping content.
            // Reserve 80 DIPs at minimum size to cover that host's extra vertical overhead.
            ((FrameworkElement)window.Content).Margin = new Thickness(0, 80, 0, 0);
            Drain();
            VerifyScanScrollClearance(window, model, new Size(900, 600), "-toolbar");
            VerifyViewportAccess(window, model, new Size(900, 600), "-toolbar");
            VerifyFolderViewportAccess(window, model, new Size(900, 600), "-toolbar");
            VerifyReviewViewportAccess(window, model, new Size(900, 600), "-toolbar");
            ((FrameworkElement)window.Content).Margin = new Thickness(0);
            Drain();
            client.FolderGroupPageHandler = (_, _) => Task.FromResult(new WorkerDuplicateFolderGroupPage([], 0, null, null));
            VerifyThemeAndTextScale(window, model, textScale);
        }
        finally { window.Close(); }
        Assert.IsTrue(textScale.Disposed, "Closing the window releases the native settings subscription.");
    }

    private static void VerifyReviewViewportAccess(
        MainWindow window,
        ShellViewModel model,
        Size size,
        string suffix = "")
    {
        model.SelectedDestination = WorkspaceDestination.Review;
        Drain();
        var scroll = Find<ScrollViewer>(window, "ReviewWorkspaceScrollViewer");
        Assert.AreEqual(ScrollBarVisibility.Disabled, scroll.HorizontalScrollBarVisibility);
        var fileGroups = Find<ListView>(window, "ReviewFileGroupsList");
        var folderGroups = Find<ListView>(window, "ReviewFolderGroupsList");
        Assert.AreEqual(25, fileGroups.Items.Count);
        Assert.AreEqual(25, folderGroups.Items.Count);
        Assert.IsTrue(VirtualizingPanel.GetIsVirtualizing(fileGroups));
        Assert.IsTrue(VirtualizingPanel.GetIsVirtualizing(folderGroups));
        Assert.AreEqual(ScrollBarVisibility.Disabled, ScrollViewer.GetHorizontalScrollBarVisibility(fileGroups));
        Assert.AreEqual(ScrollBarVisibility.Disabled, ScrollViewer.GetHorizontalScrollBarVisibility(folderGroups));
        var preferences = Find<Expander>(window, "LocationPreferencesExpander");
        preferences.IsExpanded = true;
        preferences.BringIntoView();
        Drain();
        Assert.IsTrue(preferences.IsVisible);
        var preview = Find<Button>(window, "PreferenceRunPreview");
        var reverse = Find<Button>(window, "PreferenceReverseApplication");
        var preferenceList = Find<ListView>(window, "PreferencePreviewGroups");
        preview.BringIntoView();
        Drain();
        AssertVisible(preview, window);
        Capture(window, $"populated-Review-location-preferences-preview-{size.Width}x{size.Height}{suffix}");
        Assert.AreEqual("Reverse rule application", reverse.Content);
        Assert.IsTrue(VirtualizingPanel.GetIsVirtualizing(preferenceList));
        Assert.AreEqual(ScrollBarVisibility.Disabled, ScrollViewer.GetHorizontalScrollBarVisibility(preferenceList));
        reverse.BringIntoView();
        Drain();
        AssertVisible(reverse, window);
        Capture(window, $"populated-Review-location-preferences-application-{size.Width}x{size.Height}{suffix}");
        fileGroups.BringIntoView();
        Drain();
        AssertVisible(Descendants<Button>(fileGroups).First(button => Equals(button.Content, "Open set")), window);
        AssertVisible(Descendants<Button>(folderGroups).First(button => Equals(button.Content, "Open set")), window);
        var check = Find<Button>(window, "StartPreflightButton");
        check.BringIntoView();
        Drain();
        AssertVisible(check, window);
        var boundary = Find<Border>(window, "ReviewBuildBoundaryNotice");
        boundary.BringIntoView();
        Drain();
        AssertVisible(boundary, window);
        Console.WriteLine($"{size.Width}x{size.Height}{suffix} review: files={fileGroups.ActualWidth:F1}, folders={folderGroups.ActualWidth:F1}, extent={scroll.ExtentHeight:F1}");
    }

    private static void ConfigurePopulatedFolderResults(TestWorkerClient client, ShellViewModel model, WorkerRun run)
    {
        client.FolderGroupPageHandler = (query, _) => Task.FromResult(new WorkerDuplicateFolderGroupPage(
            Enumerable.Range(1, 25).Select(id => new WorkerDuplicateFolderGroup(
                id,
                query.RunId,
                "8192",
                14,
                2,
                $@"C:\fixture\source-{id:00}\family archive\photos\2026")).ToArray(),
            25,
            null,
            null));
        client.FolderMemberPageHandler = (query, _) => Task.FromResult(new WorkerDuplicateFolderMemberPage(
            [
                new WorkerDuplicateFolderMember(
                    query.GroupId * 10 + 1,
                    query.GroupId,
                    $@"C:\fixture\source-{query.GroupId:00}\family archive\photos\2026"),
                new WorkerDuplicateFolderMember(
                    query.GroupId * 10 + 2,
                    query.GroupId,
                    $@"\\fictional-server\archive\retained generations\location-{query.GroupId:00}\family archive\photos\2026"),
            ],
            2,
            null,
            null)
        {
            ReviewSummary = new WorkerReviewFolderGroupSummary(query.GroupId, 0, 0, 2, 2),
        });
        model.DuplicateFolders.ShowRunAsync(run).GetAwaiter().GetResult();
        Drain();
    }

    private sealed class FixtureTextScale : ITextScaleSource
    {
        public double ScaleFactor { get; private set; } = 1;
        public event EventHandler? Changed;
        public bool Disposed { get; private set; }
        internal void Set(double factor)
        {
            ScaleFactor = factor;
            // Windows delivers setting events outside the WPF dispatcher.
            Task.Run(() => Changed?.Invoke(this, EventArgs.Empty)).GetAwaiter().GetResult();
        }
        public void Dispose() => Disposed = true;
    }

    private static void VerifyThemeAndTextScale(MainWindow window, ShellViewModel model, FixtureTextScale scale)
    {
        var originalTheme = Application.Current.ThemeMode;
        try
        {
            foreach (var theme in new[] { ThemeMode.Light, ThemeMode.Dark })
            {
                Application.Current.ThemeMode = theme;
                Drain();
                model.SelectedDestination = WorkspaceDestination.ScanSetup;
                Drain();
                var context = Find<TextBlock>(window, "SelectedScanContext");
                var action = Find<Button>(window, "ViewActiveProgress");
                AssertContrast(context.Foreground, window.Background, $"{theme}: shell text");
                Assert.AreEqual(window.FindResource("ButtonForeground"), action.Foreground);
                Assert.AreEqual(window.FindResource("ButtonBackground"), action.Background);
                Assert.IsTrue(action.Style.BasedOn!.Setters.OfType<Setter>()
                    .Any(setter => setter.Property == Control.TemplateProperty), "Keep the native Fluent button template.");
                model.SelectedDestination = WorkspaceDestination.FileResults;
                Drain();
                var groups = Find<DataGrid>(window, "FileGroupsGrid");
                var search = (TextBox)((SuperDuper.Windows.Views.DuplicateFilesView)
                    window.FindName("DuplicateFilesWorkspace")).FindName("FileSearch");
                Assert.IsTrue(groups.Style.BasedOn!.Setters.OfType<Setter>()
                    .Any(setter => setter.Property == Control.TemplateProperty));
                Assert.IsTrue(search.Style.BasedOn!.Setters.OfType<Setter>()
                    .Any(setter => setter.Property == Control.TemplateProperty));
                groups.ScrollIntoView(groups.Items[0]);
                groups.SelectedIndex = 0;
                groups.Focus();
                Drain();
                var selected = model.DuplicateFiles.SelectedGroup;
                foreach (var factor in new[] { 1.5, 1d })
                {
                    scale.Set(factor);
                    Drain();
                    Assert.AreEqual(14 * factor, context.FontSize);
                    Assert.AreEqual(14 * factor, action.FontSize);
                    Assert.AreEqual(14 * factor, search.FontSize);
                    Assert.AreEqual(14 * factor, groups.FontSize);
                    Assert.AreSame(selected, model.DuplicateFiles.SelectedGroup);
                    Assert.IsTrue(groups.IsKeyboardFocusWithin, "Text enlargement must preserve keyboard focus.");
                    foreach (var size in new[] { new Size(1180, 760), new Size(900, 600) })
                    {
                        window.Width = size.Width;
                        window.Height = size.Height;
                        model.SelectedDestination = WorkspaceDestination.ScanSetup;
                        Drain();
                        Capture(window, $"theme-{theme}-text-{factor}-Setup-{size.Width}x{size.Height}");
                        VerifyScanScrollClearance(window, model, size, $"-{theme}-text-{factor}");
                        model.SelectedDestination = WorkspaceDestination.FolderResults;
                        model.DuplicateFolders.ClearFiltersCommand.ExecuteAsync(null).GetAwaiter().GetResult();
                        Drain();
                        Assert.IsTrue(model.DuplicateFolders.IsEmpty, "The folder regression must exercise the empty state.");
                        {
                            var empty = Find<StackPanel>(window, "FolderEmptyState");
                            Assert.IsFalse(Find<DataGrid>(window, "FolderGroupsGrid").IsVisible,
                                "Empty-folder text must not overlay visible column headers.");
                            Reach(empty, window);
                            foreach (var text in Descendants<TextBlock>(empty)) AssertVisible(text, window);
                            Capture(window, $"theme-{theme}-text-{factor}-Folders-{size.Width}x{size.Height}");
                        }
                        model.SelectedDestination = WorkspaceDestination.FileResults;
                        Drain();
                        foreach (var id in new[] { "FileSearch", "FileApplyFilters", "FileFiltersToggle", "FileClearFilters",
                            "FileSummaryMatchingSets", "FileSummaryMatchingCopies", "FileSummaryRecoverable" })
                            Reach(Find<FrameworkElement>(window, id), window);
                        Assert.IsTrue(search.ActualWidth >= 100, "Enlarged path search must keep a useful input width.");
                        var members = Find<DataGrid>(window, "FileMembersGrid");
                        if (!members.IsVisible)
                        {
                            Find<Button>(window, "FileCompareSelectedSet").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
                            Drain();
                        }
                        members.SelectedIndex = 0;
                        members.ScrollIntoView(members.Items[0]);
                        Drain();
                        var selectedPanel = Find<Border>(window, "FileSelectedCopyPanel");
                        Assert.IsTrue(selectedPanel.IsVisible);
                        var selectedPanelScroll = Descendants<ScrollViewer>(selectedPanel).First();
                        var decisionAndPathActions = Descendants<Button>(selectedPanel).Where(button => button.Content?.ToString() is
                            "Keep" or "Mark for removal" or "Reset decision" or "Copy path" or "Show in Explorer").ToArray();
                        if (size.Width >= 1180 || factor == 1)
                        {
                            foreach (var button in decisionAndPathActions)
                            {
                                ReachWithinVerticalScroll(button, selectedPanelScroll, window);
                                Assert.IsTrue(button.DesiredSize.Width <= button.ActualWidth + button.Margin.Left + button.Margin.Right + 1,
                                    $"Enlarged {button.Content} must remain complete: desired={button.DesiredSize}, actual={button.RenderSize}.");
                            }
                        }
                        Assert.AreEqual(5, decisionAndPathActions.Length);
                        Assert.AreEqual(0, selectedPanelScroll.ScrollableWidth, 0.5);
                        Capture(window, $"theme-{theme}-text-{factor}-Files-selected-copy-{size.Width}x{size.Height}");
                        var backToCopies = Find<Button>(window, "FileBackToCopies");
                        if (backToCopies.IsVisible)
                        {
                            backToCopies.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
                            Drain();
                        }
                        if (Find<Button>(window, "FileBackToSets").IsVisible)
                        {
                            Find<Button>(window, "FileBackToSets").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
                            Drain();
                        }
                        groups.Focus();
                    }
                }
            }
        }
        finally { scale.Set(1); Application.Current.ThemeMode = originalTheme; Drain(); }
    }

    private static void AssertContrast(Brush foreground, Brush background, string description)
    {
        static double Luminance(Color color)
        {
            static double Linear(byte value) => value / 255d <= 0.04045
                ? value / 255d / 12.92 : Math.Pow((value / 255d + 0.055) / 1.055, 2.4);
            return 0.2126 * Linear(color.R) + 0.7152 * Linear(color.G) + 0.0722 * Linear(color.B);
        }
        var front = Luminance(((SolidColorBrush)foreground).Color);
        var back = Luminance(((SolidColorBrush)background).Color);
        Assert.IsTrue((Math.Max(front, back) + 0.05) / (Math.Min(front, back) + 0.05) >= 4.5,
            $"{description}: normal text must retain at least 4.5:1 contrast.");
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
            Find<Expander>(window, "ScanWorkExpander").IsExpanded = true;
            Find<Expander>(window, "ScanDiagnosticsExpander").IsExpanded = true;
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
        var view = (SuperDuper.Windows.Views.DuplicateFilesView)window.FindName("DuplicateFilesWorkspace");
        Find<Expander>(window, "FileFiltersExpander").IsExpanded = false;
        SettleLayout(window);
        var workspace = Find<Grid>(window, "FileComparisonWorkspace");
        var setPane = Find<Grid>(window, "FileSetPane");
        var detailPane = Find<Grid>(window, "FileDetailPane");
        var splitter = Find<GridSplitter>(window, "FileComparisonSplitter");
        var groups = Find<DataGrid>(window, "FileGroupsGrid");
        var members = Find<DataGrid>(window, "FileMembersGrid");
        Assert.IsTrue(model.DuplicateFiles.Members.All(member =>
            !string.IsNullOrWhiteSpace(member.SelectedRoot) && !string.IsNullOrWhiteSpace(member.RelativePath)),
            "The populated viewport fixture must display meaningful locations and paths.");

        Assert.AreEqual(ScrollBarVisibility.Disabled, ScrollViewer.GetHorizontalScrollBarVisibility(groups));
        Assert.AreEqual(ScrollBarVisibility.Disabled, ScrollViewer.GetHorizontalScrollBarVisibility(members));
        if (size.Width >= 1180)
        {
            Assert.IsTrue(setPane.IsVisible && detailPane.IsVisible && splitter.IsVisible);
            var ratio = workspace.ActualHeight / view.ActualHeight;
            Console.WriteLine($"{suffix} comparison height={workspace.ActualHeight:F1}/{view.ActualHeight:F1} ({ratio:P1}), widths={setPane.ActualWidth:F1}/{detailPane.ActualWidth:F1}");
            Assert.IsTrue(ratio >= 0.60,
                $"{suffix}: list/detail must receive at least 60% of usable Files height; measured {ratio:P1}.");
            var setWidthRatio = setPane.ActualWidth / (setPane.ActualWidth + detailPane.ActualWidth);
            Assert.IsTrue(setWidthRatio is >= 0.32 and <= 0.42,
                $"{suffix}: initial list/detail split must stay near 36/64; measured {setWidthRatio:P1}.");
            Assert.IsTrue(splitter.Focusable && KeyboardNavigation.GetIsTabStop(splitter));

            var setColumn = (ColumnDefinition)view.FindName("SetPaneColumn");
            var detailColumn = (ColumnDefinition)view.FindName("DetailPaneColumn");
            var priorSetWidth = setPane.ActualWidth;
            setColumn.Width = new GridLength(45, GridUnitType.Star);
            detailColumn.Width = new GridLength(55, GridUnitType.Star);
            Drain();
            Assert.IsTrue(setPane.ActualWidth > priorSetWidth + 20,
                "Changing the adjustable split must grow the set pane.");
            setColumn.Width = new GridLength(36, GridUnitType.Star);
            detailColumn.Width = new GridLength(64, GridUnitType.Star);
            Drain();
        }
        else
        {
            Assert.IsTrue(setPane.IsVisible && !detailPane.IsVisible && !splitter.IsVisible,
                $"{suffix}: narrow Files starts with the set list only.");
            var compare = Find<Button>(window, "FileCompareSelectedSet");
            Assert.IsTrue(compare.IsVisible && compare.IsEnabled);
            compare.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            Drain();
            Assert.IsTrue(!setPane.IsVisible && detailPane.IsVisible && !splitter.IsVisible,
                $"{suffix}: Compare selected set must replace the list with detail.");
            Assert.IsTrue(Find<Button>(window, "FileBackToSets").IsVisible);
            Capture(window, $"populated-File-copies-{suffix}");
        }

        members.SelectedIndex = 0;
        members.ScrollIntoView(members.Items[0]);
        Drain();
        var memberScroll = Descendants<ScrollViewer>(members).First();
        Assert.AreEqual(0, memberScroll.ScrollableWidth, 0.5,
            $"{suffix}: essential copy comparison must not require horizontal scrolling.");
        var exactPath = Find<TextBox>(window, "FileSelectedCopyPath");
        Assert.AreEqual(model.DuplicateFiles.Members[0].Path, exactPath.Text);
        Assert.AreEqual(TextWrapping.Wrap, exactPath.TextWrapping);
        Assert.AreEqual(ScrollBarVisibility.Disabled, exactPath.HorizontalScrollBarVisibility);
        var selectedPanel = Find<Border>(window, "FileSelectedCopyPanel");
        if (size.Width < 1180 && host.Length == 0)
            AssertVisible(Find<Button>(window, "FileBackToCopies"), window);
        Capture(window, $"populated-File-selected-path-{suffix}");
        var selectedPanelScroll = Descendants<ScrollViewer>(selectedPanel).First();
        if (host.Length == 0)
        {
            foreach (var button in Descendants<Button>(selectedPanel).Where(button => button.Content?.ToString() is
                         "Keep" or "Mark for removal" or "Reset decision" or "Copy path" or "Show in Explorer"))
                ReachWithinVerticalScroll(button, selectedPanelScroll, window);
        }
        Assert.AreEqual(0, selectedPanelScroll.ScrollableWidth, 0.5,
            $"{suffix}: selected-copy actions must not require horizontal scrolling.");
        Capture(window, $"populated-File-comparison-{suffix}");

        if (host.Length == 0)
        {
            Reach(Find<Button>(window, "FileValidateVisiblePage"), window);
            Find<Button>(window, "FileValidateVisiblePage").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            Drain();
            if (size.Width >= 1180)
            {
                Assert.IsTrue(members.IsKeyboardFocusWithin,
                    "Validation returns through the actual member-focus handler.");
            }
            else
            {
                var backToCopies = Find<Button>(window, "FileBackToCopies");
                Assert.IsTrue(backToCopies.IsKeyboardFocused,
                    "Narrow validation returns focus to the visible selected-copy comparison.");
                backToCopies.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
                Drain();
                Assert.IsTrue(members.IsVisible && members.IsKeyboardFocusWithin,
                    "Back to copies restores the copy list and keyboard focus.");
            }
        }
        else
        {
            model.DuplicateFiles.SelectedMember = null;
            Drain();
        }

        // Invoke the actual Click handler. Its focus restoration must also reveal the selected row.
        Find<Button>(window, "FileNextSet").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        Drain();
        var selected = model.DuplicateFiles.SelectedGroup;
        if (size.Width >= 1180)
        {
            Assert.IsTrue(groups.IsKeyboardFocusWithin);
            var selectedRow = (DataGridRow)groups.ItemContainerGenerator.ContainerFromItem(groups.SelectedItem);
            AssertVisible(selectedRow, window);
        }
        else
        {
            Assert.IsTrue(Find<TextBlock>(window, "FileSelectedSetName").IsKeyboardFocused,
                "Narrow set navigation keeps focus in the visible detail pane.");
        }
        Console.WriteLine($"{suffix} detail geometry: pane={detailPane.ActualHeight:F1}, members={members.ActualHeight:F1}, selected={selectedPanel.ActualHeight:F1}, heading={Find<TextBlock>(window, "FileSelectedSetName").ActualHeight:F1}");
        if (host.Length == 0)
            AssertVisible(members, window, minimumHeight: size.Width >= 1180 ? 36 : 16);

        if (size.Width < 1180)
        {
            Find<Button>(window, "FileBackToSets").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            Drain();
            Assert.IsTrue(setPane.IsVisible && !detailPane.IsVisible);
            Assert.IsTrue(groups.IsKeyboardFocusWithin, "Back to sets restores focus to the selected set.");
        }
        var groupScroll = Descendants<ScrollViewer>(groups).First();
        groupScroll.ScrollToVerticalOffset(10);
        Drain();
        var gridOffset = groupScroll.VerticalOffset;
        Assert.IsTrue(gridOffset > 0);
        model.SelectedDestination = WorkspaceDestination.Review;
        Drain();
        model.SelectedDestination = WorkspaceDestination.FileResults;
        Drain();
        Assert.AreSame(selected, model.DuplicateFiles.SelectedGroup);
        Assert.AreEqual(gridOffset, groupScroll.VerticalOffset, 1d);

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

    private static void VerifyFolderViewportAccess(MainWindow window, ShellViewModel model, Size size, string host = "")
    {
        var suffix = $"{size.Width}x{size.Height}{host}";
        model.DuplicateFolders.SelectedMember = null;
        model.SelectedDestination = WorkspaceDestination.FolderResults;
        Drain();
        var workspace = Find<Grid>(window, "FolderComparisonWorkspace");
        var setPane = Find<Grid>(window, "FolderSetPane");
        var detailPane = Find<Grid>(window, "FolderDetailPane");
        var splitter = Find<GridSplitter>(window, "FolderComparisonSplitter");
        var groups = Find<DataGrid>(window, "FolderGroupsGrid");
        var members = Find<DataGrid>(window, "FolderLocationCards");

        Assert.AreEqual(25, groups.Items.Count);
        Assert.AreEqual(2, members.Items.Count);
        Assert.IsNull(model.DuplicateFolders.SelectedMember,
            "Opening a folder-copy page must not imply a review choice.");
        Assert.AreEqual(ScrollBarVisibility.Disabled, ScrollViewer.GetHorizontalScrollBarVisibility(groups));
        Assert.AreEqual(ScrollBarVisibility.Disabled, ScrollViewer.GetHorizontalScrollBarVisibility(members));
        Assert.IsTrue(model.DuplicateFolders.SelectedRelationshipSummaryText.Contains("folder copies", StringComparison.Ordinal));
        Assert.IsTrue(model.DuplicateFolders.Members.All(member =>
            !string.IsNullOrWhiteSpace(member.ParentLocation)
            && !string.IsNullOrWhiteSpace(member.DifferingPathSegments)));

        if (size.Width >= 1180)
        {
            Assert.IsTrue(setPane.IsVisible && detailPane.IsVisible && splitter.IsVisible);
            var setWidthRatio = setPane.ActualWidth / (setPane.ActualWidth + detailPane.ActualWidth);
            Assert.IsTrue(setWidthRatio is >= 0.32 and <= 0.42,
                $"{suffix}: initial folder list/detail split must stay near 36/64; measured {setWidthRatio:P1}.");
            Assert.IsTrue(splitter.Focusable && KeyboardNavigation.GetIsTabStop(splitter));
            var setColumn = workspace.ColumnDefinitions[0];
            var detailColumn = workspace.ColumnDefinitions[2];
            var priorSetWidth = setPane.ActualWidth;
            setColumn.Width = new GridLength(45, GridUnitType.Star);
            detailColumn.Width = new GridLength(55, GridUnitType.Star);
            Drain();
            Assert.IsTrue(setPane.ActualWidth > priorSetWidth + 20,
                "Changing the adjustable folder split must grow the set pane.");
            setColumn.Width = new GridLength(36, GridUnitType.Star);
            detailColumn.Width = new GridLength(64, GridUnitType.Star);
            Drain();
        }
        else
        {
            Assert.IsTrue(setPane.IsVisible && !detailPane.IsVisible && !splitter.IsVisible,
                $"{suffix}: narrow Folders starts with the exact-folder set list only.");
            var compare = Find<Button>(window, "FolderCompareSelectedSet");
            Assert.IsTrue(compare.IsVisible && compare.IsEnabled);
            compare.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            Drain();
            Assert.IsTrue(!setPane.IsVisible && detailPane.IsVisible && !splitter.IsVisible,
                $"{suffix}: Compare selected set must replace the folder list with its copies.");
            Assert.IsTrue(Find<Button>(window, "FolderBackToSets").IsVisible);
            Assert.IsTrue(Find<TextBlock>(window, "FolderSelectedSetHeading").IsKeyboardFocused,
                "Narrow folder comparison must focus its selected-set heading.");
            Capture(window, $"populated-Folder-copies-{suffix}");
        }

        members.SelectedIndex = 0;
        members.ScrollIntoView(members.Items[0]);
        Drain();
        var selectedPanel = Find<Border>(window, "FolderSelectedCopyPanel");
        var selectedPath = Find<TextBox>(window, "FolderSelectedCopyPath");
        Assert.AreEqual(model.DuplicateFolders.Members[0].Path, selectedPath.Text);
        Assert.AreEqual(TextWrapping.Wrap, selectedPath.TextWrapping);
        Assert.AreEqual(ScrollBarVisibility.Disabled, selectedPath.HorizontalScrollBarVisibility);
        Assert.IsTrue(selectedPanel.IsVisible);
        var selectedPanelScroll = Find<ScrollViewer>(window, "FolderSelectedCopyScrollViewer");
        var actions = Descendants<Button>(selectedPanel).Where(button => button.Content?.ToString() is
            "Keep" or "Mark for removal" or "Reset decision" or "Copy path" or "Show in Explorer").ToArray();
        Assert.AreEqual(5, actions.Length);
        if (host.Length == 0)
        {
            foreach (var action in actions)
            {
                ReachWithinVerticalScroll(action, selectedPanelScroll, window);
                StringAssert.Contains(AutomationProperties.GetName(action), model.DuplicateFolders.Members[0].FolderName);
            }
        }
        Assert.AreEqual(0, selectedPanelScroll.ScrollableWidth, 0.5,
            $"{suffix}: selected-folder path and decisions must not require horizontal scrolling.");
        StringAssert.Contains(
            Descendants<TextBlock>(selectedPanel).Single(text => text.Text.StartsWith("A folder decision applies", StringComparison.Ordinal)).Text,
            "all descendants");
        Capture(window, $"populated-Folder-selected-copy-{suffix}");

        if (host.Length == 0)
        {
            Find<Button>(window, "FolderSelectPageInExplorer").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            Drain();
            if (size.Width >= 1180)
            {
                Assert.IsTrue(members.IsKeyboardFocusWithin,
                    "Bounded folder-page reveal returns focus to the visible comparison list.");
            }
            else
            {
                var backToCopies = Find<Button>(window, "FolderBackToCopies");
                Assert.IsTrue(backToCopies.IsKeyboardFocused,
                    "Bounded folder-page reveal returns focus to the visible selected-folder detail.");
                backToCopies.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
                Drain();
                Assert.IsTrue(members.IsVisible && members.IsKeyboardFocusWithin,
                    "Back to folder copies restores the comparison list and keyboard focus.");
            }
        }
        else
        {
            model.DuplicateFolders.SelectedMember = null;
            Drain();
        }

        var selectedGroup = model.DuplicateFolders.SelectedGroup;
        if (size.Width < 1180)
        {
            Find<Button>(window, "FolderBackToSets").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            Drain();
            Assert.IsTrue(setPane.IsVisible && !detailPane.IsVisible);
            Assert.IsTrue(groups.IsKeyboardFocusWithin, "Back to folder sets restores set-list focus.");
        }
        model.SelectedDestination = WorkspaceDestination.Review;
        Drain();
        model.SelectedDestination = WorkspaceDestination.FolderResults;
        Drain();
        Assert.AreSame(selectedGroup, model.DuplicateFolders.SelectedGroup);
    }

    private static void Reach(FrameworkElement element, Window window)
    {
        element.BringIntoView();
        Drain();
        AssertVisible(element, window);
    }

    private static void ReachWithinVerticalScroll(
        FrameworkElement element,
        ScrollViewer scroll,
        Window window)
    {
        var bounds = element.TransformToAncestor(scroll).TransformBounds(new Rect(element.RenderSize));
        if (bounds.Top < 0)
            scroll.ScrollToVerticalOffset(Math.Max(0, scroll.VerticalOffset + bounds.Top));
        else if (bounds.Bottom > scroll.ViewportHeight)
            scroll.ScrollToVerticalOffset(scroll.VerticalOffset + bounds.Bottom - scroll.ViewportHeight);
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
            if (parent is UIElement clip && (clip.ClipToBounds || clip is ScrollContentPresenter))
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
        var directory = Environment.GetEnvironmentVariable("SUPER_DUPER_UIR05C_CAPTURES");
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
