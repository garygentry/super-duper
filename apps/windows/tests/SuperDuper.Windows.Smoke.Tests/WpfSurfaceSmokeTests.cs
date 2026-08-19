using System.Runtime.ExceptionServices;
using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using SuperDuper.Windows.Accessibility;
using SuperDuper.Windows.Views;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Smoke.Tests;

[TestClass]
public sealed class WpfSurfaceSmokeTests
{
    [TestMethod]
    public void ShutdownCompletionIsNotQueuedAtStarvableIdlePriority()
    {
        Assert.AreEqual(DispatcherPriority.Normal, MainWindow.ShutdownDispatcherPriority);
        Assert.IsTrue(MainWindow.ShutdownDispatcherPriority > DispatcherPriority.ApplicationIdle);
    }

    [TestMethod]
    public void ServerSortInteraction_AlternatesTwoDirectionsFromAuthoritativeState()
    {
        Assert.AreEqual(
            WorkerSortDirection.Descending,
            ServerSortInteraction.NextDirection(
                DuplicateFileGroupSortField.GroupSize,
                WorkerSortDirection.Ascending,
                DuplicateFileGroupSortField.GroupSize));
        Assert.AreEqual(
            WorkerSortDirection.Ascending,
            ServerSortInteraction.NextDirection(
                DuplicateFileGroupSortField.GroupSize,
                WorkerSortDirection.Descending,
                DuplicateFileGroupSortField.GroupSize));
        Assert.AreEqual(
            WorkerSortDirection.Ascending,
            ServerSortInteraction.NextDirection(
                DuplicateFileGroupSortField.RecoverableBytes,
                WorkerSortDirection.Descending,
                DuplicateFileGroupSortField.GroupSize));
    }

    [TestMethod]
    public void ResultsSurfaces_LoadOnStaWithSystemThemeVirtualizationAndAutomationIds()
    {
        RunOnSta(() =>
        {
            var app = new App();
            app.InitializeComponent();
            Assert.AreEqual(ThemeMode.System, app.ThemeMode);

            var files = new DuplicateFilesView();
            var folders = new DuplicateFoldersView();
            var sessions = new SessionListView();
            var setup = new SessionSetupView();
            var history = new RunHistoryView();
            AssertSurface(
                files,
                "FileSearch",
                "FileApplyFilters",
                "FileGroupsGrid",
                "FileMembersGrid");
            var fileGroups = FindByAutomationId<DataGrid>(files, "FileGroupsGrid");
            Assert.IsFalse(fileGroups.Columns.Single(column => Equals(column.Header, "Type")).CanUserSort);
            Assert.IsFalse(fileGroups.Columns.Single(column => Equals(column.Header, "Location span")).CanUserSort);
            Assert.AreEqual("Duplicate file review summary", AutomationProperties.GetName(
                FindByAutomationId<FrameworkElement>(files, "FileReviewSummary")));
            Assert.AreEqual("Duplicate file location coverage", AutomationProperties.GetName(
                FindByAutomationId<FrameworkElement>(files, "FileLocationSummary")));
            _ = FindByAutomationId<TextBlock>(files, "FileLocationSummaryText");
            Assert.AreEqual(
                "Show only duplicate sets whose one-copy size is at least 1 GB, 1,073,741,824 bytes",
                AutomationProperties.GetName(FindByAutomationId<CheckBox>(files, "FileOneGigabyteOrLarger")));
            Assert.AreEqual(
                "Show only duplicate sets with three or more copies",
                AutomationProperties.GetName(FindByAutomationId<CheckBox>(files, "FileThreeOrMoreCopies")));
            Assert.AreEqual(
                "Show only duplicate sets across multiple drives",
                AutomationProperties.GetName(FindByAutomationId<CheckBox>(files, "FileAcrossDrives")));
            StringAssert.Contains(
                AutomationProperties.GetHelpText(FindByAutomationId<TextBox>(files, "FileSearch")),
                "complete immutable member path");
            Assert.AreEqual(
                "Match the complete canonical member path",
                AutomationProperties.GetName(FindByAutomationId<CheckBox>(files, "FileExactPathMatch")));
            StringAssert.Contains(
                AutomationProperties.GetHelpText(FindByAutomationId<CheckBox>(files, "FileExactPathMatch")),
                "Unicode case normalization");
            Assert.AreEqual(
                "Filename extension without the dot",
                AutomationProperties.GetName(FindByAutomationId<TextBox>(files, "FileExtension")));
            StringAssert.Contains(
                AutomationProperties.GetHelpText(FindByAutomationId<TextBox>(files, "FileExtension")),
                "any immutable member");
            Assert.AreEqual(
                "Use no filename extension as the extension filter value",
                AutomationProperties.GetName(FindByAutomationId<CheckBox>(files, "FileWithoutExtension")));
            StringAssert.Contains(
                AutomationProperties.GetHelpText(FindByAutomationId<CheckBox>(files, "FileWithoutExtension")),
                "terminal dot");
            Assert.AreEqual(
                "Require every copy in a duplicate set to match the filename extension filter",
                AutomationProperties.GetName(FindByAutomationId<CheckBox>(files, "FileAllExtensionsMatch")));
            StringAssert.Contains(
                AutomationProperties.GetHelpText(FindByAutomationId<CheckBox>(files, "FileAllExtensionsMatch")),
                "distinct from file type");
            StringAssert.Contains(
                AutomationProperties.GetHelpText(FindByAutomationId<TextBox>(files, "FileMinimumSize")),
                "one-copy file size");
            Assert.AreEqual(
                "Selected root facet; choose All selected roots to remove this filter",
                AutomationProperties.GetName(FindByAutomationId<ComboBox>(files, "FileSelectedRootFacet")));
            Assert.AreEqual(
                "Sort selected roots by most matching sets",
                AutomationProperties.GetName(FindByAutomationId<Button>(files, "FileRootFacetMostSets")));
            Assert.AreEqual(
                "Sort selected roots by name",
                AutomationProperties.GetName(FindByAutomationId<Button>(files, "FileRootFacetNameSort")));
            Assert.AreEqual(
                "Previous selected-root facet page",
                AutomationProperties.GetName(FindByAutomationId<Button>(files, "FilePreviousRootFacets")));
            Assert.AreEqual(
                "Next selected-root facet page",
                AutomationProperties.GetName(FindByAutomationId<Button>(files, "FileNextRootFacets")));
            _ = FindByAutomationId<TextBlock>(files, "FileSelectedRootFilterText");
            Assert.AreEqual(
                "Drive facet; choose All drives to remove this filter",
                AutomationProperties.GetName(FindByAutomationId<ComboBox>(files, "FileDriveFacet")));
            Assert.AreEqual(
                "Sort drives by most matching sets",
                AutomationProperties.GetName(FindByAutomationId<Button>(files, "FileDriveFacetMostSets")));
            Assert.AreEqual(
                "Sort drives by name",
                AutomationProperties.GetName(FindByAutomationId<Button>(files, "FileDriveFacetNameSort")));
            Assert.AreEqual(
                "Previous drive facet page",
                AutomationProperties.GetName(FindByAutomationId<Button>(files, "FilePreviousDriveFacets")));
            Assert.AreEqual(
                "Next drive facet page",
                AutomationProperties.GetName(FindByAutomationId<Button>(files, "FileNextDriveFacets")));
            _ = FindByAutomationId<TextBlock>(files, "FileSelectedDriveFilterText");
            Assert.AreEqual(
                AutomationLiveSetting.Polite,
                AutomationProperties.GetLiveSetting(FindByAutomationId<TextBlock>(files, "FileSummaryMatchingSets")));
            var groupStatus = FindByAutomationId<TextBlock>(files, "FileGroupCount");
            Assert.AreEqual(AutomationNotificationKind.ActionCompleted,
                AutomationNotificationBehavior.GetNotificationKind(groupStatus));
            Assert.AreEqual(AutomationNotificationProcessing.MostRecent,
                AutomationNotificationBehavior.GetNotificationProcessing(groupStatus));
            Assert.AreEqual("DuplicateFileGroupQuery",
                AutomationNotificationBehavior.GetActivityId(groupStatus));
            Assert.AreEqual(
                AutomationLiveSetting.Polite,
                AutomationProperties.GetLiveSetting(FindByAutomationId<TextBlock>(files, "FileLocationSummaryText")));
            var groupError = FindByAutomationId<Border>(files, "FileGroupError");
            Assert.AreEqual(AutomationNotificationKind.ActionAborted,
                AutomationNotificationBehavior.GetNotificationKind(groupError));
            Assert.AreEqual(AutomationNotificationProcessing.ImportantMostRecent,
                AutomationNotificationBehavior.GetNotificationProcessing(groupError));
            Assert.AreEqual(
                AutomationLiveSetting.Assertive,
                AutomationProperties.GetLiveSetting(groupError));
            var memberStatus = FindByAutomationId<TextBlock>(files, "FileMemberCount");
            Assert.AreEqual(AutomationNotificationKind.ActionCompleted,
                AutomationNotificationBehavior.GetNotificationKind(memberStatus));
            Assert.AreEqual(AutomationNotificationProcessing.MostRecent,
                AutomationNotificationBehavior.GetNotificationProcessing(memberStatus));
            Assert.AreEqual("DuplicateFileMemberQuery",
                AutomationNotificationBehavior.GetActivityId(memberStatus));
            var detailError = FindByAutomationId<TextBlock>(files, "FileDetailError");
            Assert.AreEqual(AutomationNotificationKind.ActionAborted,
                AutomationNotificationBehavior.GetNotificationKind(detailError));
            Assert.AreEqual(AutomationNotificationProcessing.ImportantMostRecent,
                AutomationNotificationBehavior.GetNotificationProcessing(detailError));
            Assert.AreEqual("DuplicateFileMemberQuery",
                AutomationNotificationBehavior.GetActivityId(detailError));
            Assert.AreEqual(
                AutomationLiveSetting.Assertive,
                AutomationProperties.GetLiveSetting(detailError));
            Assert.AreEqual(
                SystemColors.ControlTextBrush,
                FindByAutomationId<Border>(files, "FileGroupError").BorderBrush);
            Assert.AreEqual(
                SystemColors.ControlTextBrush,
                FindByAutomationId<TextBlock>(files, "FileRootFacetError").Foreground);
            Assert.AreEqual(
                SystemColors.ControlTextBrush,
                FindByAutomationId<TextBlock>(files, "FileDriveFacetError").Foreground);
            Assert.AreEqual(
                SystemColors.ControlTextBrush,
                FindByAutomationId<TextBlock>(files, "FileDetailError").Foreground);
            StringAssert.Contains(
                FindByAutomationId<TextBlock>(files, "FileSelectedSetExplanation").Text,
                "does not identify an original");
            _ = FindByAutomationId<TextBlock>(files, "FileSelectedSetLocations");
            var previousSet = FindByAutomationId<Button>(files, "FilePreviousSet");
            var nextSet = FindByAutomationId<Button>(files, "FileNextSet");
            Assert.AreEqual(DispatcherPriority.Background, DuplicateFilesView.SetNavigationFocusPriority);
            Assert.AreEqual(TimeSpan.FromMilliseconds(50), DuplicateFilesView.SetNavigationFocusRetryDelay);
            StringAssert.Contains(AutomationProperties.GetName(previousSet), "focus returns");
            StringAssert.Contains(AutomationProperties.GetName(nextSet), "focus returns");
            Assert.AreEqual(
                "Resize duplicate group and selected-set areas",
                AutomationProperties.GetName(FindByAutomationId<GridSplitter>(files, "FileResultsSplitter")));
            var keyboardOrder = new FrameworkElement[]
            {
                FindByAutomationId<TextBox>(files, "FileSearch"),
                FindByAutomationId<CheckBox>(files, "FileExactPathMatch"),
                FindByAutomationId<TextBox>(files, "FileMinimumSize"),
                FindByAutomationId<CheckBox>(files, "FileOneGigabyteOrLarger"),
                FindByAutomationId<CheckBox>(files, "FileThreeOrMoreCopies"),
                FindByAutomationId<CheckBox>(files, "FileAcrossDrives"),
                FindByAutomationId<Button>(files, "FileApplyFilters"),
                FindByAutomationId<TextBox>(files, "FileExtension"),
                FindByAutomationId<CheckBox>(files, "FileWithoutExtension"),
                FindByAutomationId<CheckBox>(files, "FileAllExtensionsMatch"),
                FindByAutomationId<ComboBox>(files, "FileSelectedRootFacet"),
                FindByAutomationId<Button>(files, "FileRootFacetMostSets"),
                FindByAutomationId<Button>(files, "FileRootFacetNameSort"),
                FindByAutomationId<Button>(files, "FilePreviousRootFacets"),
                FindByAutomationId<Button>(files, "FileNextRootFacets"),
                FindByAutomationId<ComboBox>(files, "FileDriveFacet"),
                FindByAutomationId<Button>(files, "FileDriveFacetMostSets"),
                FindByAutomationId<Button>(files, "FileDriveFacetNameSort"),
                FindByAutomationId<Button>(files, "FilePreviousDriveFacets"),
                FindByAutomationId<Button>(files, "FileNextDriveFacets"),
                FindByAutomationId<DataGrid>(files, "FileGroupsGrid"),
                FindByAutomationId<Button>(files, "FilePreviousGroupPage"),
                FindByAutomationId<Button>(files, "FileNextGroupPage"),
                FindByAutomationId<Button>(files, "FileClearFilters"),
                FindByAutomationId<GridSplitter>(files, "FileResultsSplitter"),
                previousSet,
                nextSet,
                FindByAutomationId<DataGrid>(files, "FileMembersGrid"),
                FindByAutomationId<Button>(files, "FilePreviousMemberPage"),
                FindByAutomationId<Button>(files, "FileNextMemberPage"),
            };
            CollectionAssert.AreEqual(
                Enumerable.Range(0, keyboardOrder.Length).ToArray(),
                keyboardOrder.Select(KeyboardNavigation.GetTabIndex).ToArray());
            AssertPrimaryFileFiltersReflow(files);
            var fileMemberHeaders = FindByAutomationId<DataGrid>(files, "FileMembersGrid")
                .Columns.Select(column => column.Header?.ToString()).ToArray();
            CollectionAssert.IsSubsetOf(
                new[] { "Selected root", "Relative path", "Drive" },
                fileMemberHeaders);
            AssertSurface(
                folders,
                "FolderSearch",
                "FolderApplyFilters",
                "FolderGroupsGrid",
                "FolderMembersGrid");

            Assert.AreEqual("Scan sessions", AutomationProperties.GetName(
                FindByAutomationId<ListBox>(sessions, "SessionsList")));
            Assert.AreEqual("Session setup", AutomationProperties.GetName(setup));
            Assert.AreEqual("Session name", AutomationProperties.GetName(
                FindByAutomationId<TextBox>(setup, "SessionName")));
            Assert.AreEqual("Cloud scan policy", AutomationProperties.GetName(
                FindByAutomationId<TextBlock>(setup, "CloudPolicyName")));
            Assert.IsFalse(string.IsNullOrWhiteSpace(AutomationProperties.GetName(
                FindByAutomationId<TextBlock>(setup, "CloudPolicyDescription"))));
            Assert.IsFalse(string.IsNullOrWhiteSpace(AutomationProperties.GetName(
                FindByAutomationId<TextBlock>(setup, "CloudDetectionStatus"))));
            Assert.AreEqual("Refresh registered cloud locations", AutomationProperties.GetName(
                FindByAutomationId<Button>(setup, "RefreshCloudLocations")));
            Assert.AreEqual("Detected excluded cloud locations", AutomationProperties.GetName(
                FindByAutomationId<ItemsControl>(setup, "DetectedCloudLocations")));
            Assert.AreEqual("Manual cloud location exclusions", AutomationProperties.GetName(
                FindByAutomationId<TextBox>(setup, "ManualCloudLocationExclusions")));
            Assert.AreEqual("Ignore patterns", AutomationProperties.GetName(
                FindByAutomationId<TextBox>(setup, "IgnorePatterns")));
            Assert.AreEqual("Run history", AutomationProperties.GetName(
                FindByAutomationId<DataGrid>(history, "RunHistoryGrid")));

            var focusHost = new Window { Width = 1200, Height = 800, Content = files };
            focusHost.Show();
            focusHost.Activate();
            DrainDispatcher();
            FrameworkElement? announcedElement = null;
            string? announcedText = null;
            AutomationNotificationKind? announcedKind = null;
            string? announcedActivityId = null;
            void CaptureNotification(
                FrameworkElement element,
                string announcement,
                AutomationNotificationKind kind,
                AutomationNotificationProcessing _,
                string activityId)
            {
                announcedElement = element;
                announcedText = announcement;
                announcedKind = kind;
                announcedActivityId = activityId;
            }
            AutomationNotificationBehavior.NotificationRaised += CaptureNotification;
            try
            {
                const string announcement = "Duplicate file query complete. 2 matching sets.";
                AutomationProperties.SetName(groupStatus, announcement);
                AutomationNotificationBehavior.SetAnnouncementVersion(groupStatus, 1);
                DrainDispatcher();
                Assert.AreSame(groupStatus, announcedElement);
                Assert.AreEqual(announcement, announcedText);
                Assert.AreEqual(AutomationNotificationKind.ActionCompleted, announcedKind);
                Assert.AreEqual("DuplicateFileGroupQuery", announcedActivityId);

                const string selectedSetAnnouncement =
                    "Selected duplicate set loaded: photo.jpg. 2 copies. 1 selected root · on 1 drive.";
                AutomationProperties.SetName(memberStatus, selectedSetAnnouncement);
                AutomationNotificationBehavior.SetAnnouncementVersion(memberStatus, 1);
                DrainDispatcher();
                Assert.AreSame(memberStatus, announcedElement);
                Assert.AreEqual(selectedSetAnnouncement, announcedText);
                Assert.AreEqual(AutomationNotificationKind.ActionCompleted, announcedKind);
                Assert.AreEqual("DuplicateFileMemberQuery", announcedActivityId);

                const string detailErrorAnnouncement =
                    "Duplicate-file detail error: Worker member query failed.";
                AutomationProperties.SetName(detailError, detailErrorAnnouncement);
                AutomationNotificationBehavior.SetAnnouncementVersion(detailError, 1);
                DrainDispatcher();
                Assert.AreSame(detailError, announcedElement);
                Assert.AreEqual(detailErrorAnnouncement, announcedText);
                Assert.AreEqual(AutomationNotificationKind.ActionAborted, announcedKind);
                Assert.AreEqual("DuplicateFileMemberQuery", announcedActivityId);
            }
            finally
            {
                AutomationNotificationBehavior.NotificationRaised -= CaptureNotification;
            }
            fileGroups.ItemsSource = new[] { new object(), new object() };
            fileGroups.SelectedIndex = 1;
            fileGroups.UpdateLayout();
            Assert.IsTrue(nextSet.Focus());
            Assert.IsTrue(files.RestoreGroupGridFocus());
            DrainDispatcher();
            Assert.IsTrue(fileGroups.IsKeyboardFocusWithin);
            Assert.IsInstanceOfType<DataGridCell>(Keyboard.FocusedElement);
            focusHost.Close();

            app.Shutdown();
        });
    }

    private static void AssertSurface(
        FrameworkElement view,
        string searchId,
        string applyId,
        string groupsId,
        string membersId)
    {
        var search = FindByAutomationId<TextBox>(view, searchId);
        var apply = FindByAutomationId<Button>(view, applyId);
        var groups = FindByAutomationId<DataGrid>(view, groupsId);
        var members = FindByAutomationId<DataGrid>(view, membersId);

        Assert.IsFalse(string.IsNullOrWhiteSpace(AutomationProperties.GetName(search)));
        Assert.IsFalse(string.IsNullOrWhiteSpace(AutomationProperties.GetName(groups)));
        Assert.IsFalse(string.IsNullOrWhiteSpace(AutomationProperties.GetName(members)));
        Assert.AreEqual("Apply filters", apply.Content);
        Assert.IsTrue(VirtualizingPanel.GetIsVirtualizing(groups));
        Assert.AreEqual(VirtualizationMode.Recycling, VirtualizingPanel.GetVirtualizationMode(groups));
        Assert.IsTrue(VirtualizingPanel.GetIsVirtualizing(members));
        Assert.AreEqual(VirtualizationMode.Recycling, VirtualizingPanel.GetVirtualizationMode(members));
    }

    private static void AssertPrimaryFileFiltersReflow(DuplicateFilesView files)
    {
        const double narrowWorkspaceWidth = 620;
        var host = new Window
        {
            Width = narrowWorkspaceWidth,
            Height = 900,
            Content = files,
            SizeToContent = SizeToContent.Manual,
        };
        host.Show();
        host.UpdateLayout();
        DrainDispatcher();

        var firstFilter = FindByAutomationId<TextBox>(files, "FileSearch");
        var lastFilter = FindByAutomationId<Button>(files, "FileApplyFilters");
        var firstTop = firstFilter.TranslatePoint(new Point(0, 0), files).Y;
        var lastTop = lastFilter.TranslatePoint(new Point(0, 0), files).Y;
        var lastRight = lastFilter.TranslatePoint(
            new Point(lastFilter.ActualWidth, 0),
            files).X;

        Assert.IsTrue(
            lastTop > firstTop,
            "The primary filter controls should wrap to another row in a narrow workspace.");
        Assert.IsTrue(
            lastRight <= files.ActualWidth,
            "A wrapped primary filter control extends beyond the duplicate-file workspace.");
        host.Content = null;
        host.Close();
    }

    private static T FindByAutomationId<T>(DependencyObject root, string automationId)
        where T : FrameworkElement
    {
        if (root is T match && AutomationProperties.GetAutomationId(match) == automationId)
        {
            return match;
        }
        foreach (var child in LogicalTreeHelper.GetChildren(root).OfType<DependencyObject>())
        {
            try
            {
                return FindByAutomationId<T>(child, automationId);
            }
            catch (AssertFailedException)
            {
            }
        }
        Assert.Fail($"Could not find {typeof(T).Name} with automation ID {automationId}.");
        return null!;
    }

    private static void RunOnSta(Action action)
    {
        Exception? failure = null;
        var thread = new Thread(() =>
        {
            try
            {
                action();
            }
            catch (Exception exception)
            {
                failure = exception;
            }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.IsTrue(thread.Join(TimeSpan.FromSeconds(15)), "The WPF smoke thread timed out.");
        if (failure is not null)
        {
            ExceptionDispatchInfo.Capture(failure).Throw();
        }
    }

    private static void DrainDispatcher()
    {
        var frame = new DispatcherFrame();
        Dispatcher.CurrentDispatcher.BeginInvoke(
            DispatcherPriority.ContextIdle,
            new Action(() => frame.Continue = false));
        Dispatcher.PushFrame(frame);
    }
}
