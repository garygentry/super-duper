using System.Runtime.ExceptionServices;
using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Threading;
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
            Assert.AreEqual("Duplicate file review summary", AutomationProperties.GetName(
                FindByAutomationId<FrameworkElement>(files, "FileReviewSummary")));
            StringAssert.Contains(
                FindByAutomationId<TextBlock>(files, "FileSelectedSetExplanation").Text,
                "does not identify an original");
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
}
