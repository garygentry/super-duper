using System.Runtime.ExceptionServices;
using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using SuperDuper.Windows.Views;

namespace SuperDuper.Windows.Smoke.Tests;

[TestClass]
public sealed class WpfSurfaceSmokeTests
{
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
            AssertSurface(
                files,
                "FileSearch",
                "FileApplyFilters",
                "FileGroupsGrid",
                "FileMembersGrid");
            AssertSurface(
                folders,
                "FolderSearch",
                "FolderApplyFilters",
                "FolderGroupsGrid",
                "FolderMembersGrid");

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
