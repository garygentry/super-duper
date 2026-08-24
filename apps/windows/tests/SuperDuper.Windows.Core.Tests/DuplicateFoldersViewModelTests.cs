using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class DuplicateFoldersViewModelTests
{
    [TestMethod]
    public async Task CompletedRunLoadsDistinctMasterDetailAndPathActions()
    {
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage([Group(1, query.RunId, @"C:\One")], 1, null, null)),
            FolderMemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderMemberPage([new(1, query.GroupId, @"C:\One")], 1, null, null)),
        };
        var clipboard = new TestClipboard();
        var explorer = new TestExplorer();
        using var viewModel = new DuplicateFoldersViewModel(client, clipboard, explorer);

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(7, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.AreEqual(1, viewModel.Groups.Count);
        Assert.AreEqual(1, viewModel.Members.Count);
        Assert.AreEqual(
            "Duplicate folder query complete. 1 matching exact duplicate folder group.",
            viewModel.GroupStatusAnnouncement);
        Assert.AreEqual(1, viewModel.GroupStatusAnnouncementVersion);
        Assert.AreEqual(
            "Selected exact duplicate folder group loaded. Showing 1 of 1 folder copies on this server-owned page. "
            + "Use the side-by-side location cards; highlighted path segments differ among this page.",
            viewModel.MemberStatusAnnouncement);
        Assert.AreEqual(1, viewModel.MemberStatusAnnouncementVersion);
        viewModel.CopyPathCommand.Execute(viewModel.Members[0]);
        await viewModel.RevealInExplorerCommand.ExecuteAsync(viewModel.Members[0]);
        Assert.AreEqual(@"C:\One", clipboard.Text);
        Assert.AreEqual(@"C:\One", explorer.RevealedPath);
        Assert.AreEqual(
            "File Explorer opened and selected One at C:.",
            viewModel.ExplorerStatusMessage);
        Assert.AreEqual(1, viewModel.ExplorerStatusAnnouncementVersion);
        Assert.IsFalse(viewModel.IsExplorerCommandRunning);
    }

    [TestMethod]
    public async Task FilterGenerationRejectsLateResponseAndCacheRemainsBounded()
    {
        var oldResponse = new TaskCompletionSource<WorkerDuplicateFolderGroupPage>(TaskCreationOptions.RunContinuationsAsynchronously);
        var observed = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) =>
            {
                if (query.Filter.Search.Length == 0 && query.Cursor is null)
                {
                    observed.TrySetResult();
                    return oldResponse.Task;
                }
                var page = query.Cursor is null ? 0 : int.Parse(query.Cursor);
                return Task.FromResult(new WorkerDuplicateFolderGroupPage(
                    [Group(page + 10, query.RunId, $@"C:\new-{page}")],
                    10,
                    page < 9 ? (page + 1).ToString() : null,
                    page > 0 ? (page - 1).ToString() : null));
            },
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), new TestExplorer());
        var initial = viewModel.ShowRunAsync(TestWorkerClient.CreateRun(8, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        await observed.Task;
        viewModel.SearchText = "new";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        var currentAnnouncementVersion = viewModel.GroupStatusAnnouncementVersion;
        oldResponse.SetResult(new WorkerDuplicateFolderGroupPage([Group(1, 8, @"C:\stale")], 1, null, null));
        await initial;
        Assert.AreEqual(@"C:\new-0", viewModel.Groups[0].RepresentativePath);
        Assert.AreEqual(currentAnnouncementVersion, viewModel.GroupStatusAnnouncementVersion);

        for (var page = 1; page < 9; page++)
        {
            await viewModel.NextPageCommand.ExecuteAsync(null);
            Assert.IsTrue(viewModel.CachedGroupPageCount <= DuplicateFoldersViewModel.CacheCapacity);
        }
    }

    [TestMethod]
    public async Task GroupQueryAnnouncementsRepeatAndReportValidationAndWorkerFailures()
    {
        var failWorkerQuery = false;
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (_, _) => failWorkerQuery
                ? Task.FromException<WorkerDuplicateFolderGroupPage>(new InvalidOperationException("Worker query failed."))
                : Task.FromResult(new WorkerDuplicateFolderGroupPage([], 0, null, null)),
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), new TestExplorer());

        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(13, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        Assert.AreEqual(
            "Duplicate folder query complete. No matching exact duplicate folder groups.",
            viewModel.GroupStatusAnnouncement);
        Assert.AreEqual(1, viewModel.GroupStatusAnnouncementVersion);

        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.AreEqual(2, viewModel.GroupStatusAnnouncementVersion);

        viewModel.MinimumSizeText = "invalid";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        StringAssert.Contains(viewModel.GroupErrorAnnouncement, "filters could not be applied");
        StringAssert.Contains(viewModel.GroupErrorAnnouncement, "non-negative whole number");
        Assert.AreEqual(1, viewModel.GroupErrorAnnouncementVersion);

        viewModel.MinimumSizeText = string.Empty;
        failWorkerQuery = true;
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        StringAssert.Contains(viewModel.GroupErrorAnnouncement, "results could not be loaded");
        StringAssert.Contains(viewModel.GroupErrorAnnouncement, "Worker query failed");
        Assert.AreEqual(2, viewModel.GroupErrorAnnouncementVersion);
    }

    [TestMethod]
    public async Task ResortKeepsDisplayedResultsUntilReplacementPageArrives()
    {
        var replacement = new TaskCompletionSource<WorkerDuplicateFolderGroupPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var resortObserved = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) =>
            {
                if (query.SortDirection == WorkerSortDirection.Ascending)
                {
                    resortObserved.TrySetResult();
                    return replacement.Task;
                }
                return Task.FromResult(new WorkerDuplicateFolderGroupPage(
                    [Group(1, query.RunId, @"C:\before-sort")], 1, null, null));
            },
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(12, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        var resort = viewModel.ApplySortAsync(
            DuplicateFolderGroupSortField.TotalBytes,
            WorkerSortDirection.Ascending);
        await resortObserved.Task;

        Assert.IsTrue(viewModel.IsLoading);
        Assert.IsFalse(viewModel.IsEmpty);
        Assert.IsFalse(viewModel.IsLoadingOverlayVisible);
        Assert.AreEqual(@"C:\before-sort", viewModel.Groups.Single().RepresentativePath);

        replacement.SetResult(new WorkerDuplicateFolderGroupPage(
            [Group(2, 12, @"C:\after-sort")], 1, null, null));
        await resort;

        Assert.IsFalse(viewModel.IsLoading);
        Assert.AreEqual(@"C:\after-sort", viewModel.Groups.Single().RepresentativePath);
    }

    [TestMethod]
    public async Task MemberQueryAnnouncementsRepeatForCachedPagesAndCoverEmptyAndWorkerFailure()
    {
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage(
                    [
                        Group(1, query.RunId, @"C:\first"),
                        Group(2, query.RunId, @"C:\empty"),
                        Group(3, query.RunId, @"C:\failed"),
                    ],
                    3,
                    null,
                    null)),
            FolderMemberPageHandler = (query, _) => query.GroupId switch
            {
                1 when query.Cursor is null => Task.FromResult(
                    new WorkerDuplicateFolderMemberPage(
                        [new(1, query.GroupId, @"C:\first")],
                        2,
                        "next-members",
                        null)),
                1 => Task.FromResult(
                    new WorkerDuplicateFolderMemberPage(
                        [new(2, query.GroupId, @"D:\first-copy")],
                        2,
                        null,
                        "previous-members")),
                2 => Task.FromResult(new WorkerDuplicateFolderMemberPage([], 0, null, null)),
                _ => Task.FromException<WorkerDuplicateFolderMemberPage>(
                    new IOException("Worker folder-member query failed.")),
            },
        };
        var explorer = new TestExplorer { Error = new IOException("Explorer action failed.") };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), explorer);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(14, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.AreEqual(1, viewModel.MemberStatusAnnouncementVersion);
        var repeatedAnnouncement = viewModel.MemberStatusAnnouncement;
        await viewModel.RevealInExplorerCommand.ExecuteAsync(viewModel.Members[0]);
        Assert.IsTrue(viewModel.HasExplorerError);
        StringAssert.Contains(viewModel.ExplorerErrorMessage, "Verify that the location is available");
        Assert.AreEqual(1, viewModel.ExplorerErrorAnnouncementVersion);
        Assert.AreEqual(0, viewModel.MemberErrorAnnouncementVersion);
        await viewModel.NextMemberPageCommand.ExecuteAsync(null);
        Assert.AreEqual(2, viewModel.MemberStatusAnnouncementVersion);
        Assert.AreEqual(repeatedAnnouncement, viewModel.MemberStatusAnnouncement);
        Assert.IsFalse(viewModel.HasDetailError);

        viewModel.SelectedGroup = viewModel.Groups[1];
        Assert.AreEqual(3, viewModel.MemberStatusAnnouncementVersion);
        Assert.AreEqual(
            "Selected exact duplicate folder group loaded. No folder copies to display.",
            viewModel.MemberStatusAnnouncement);

        viewModel.SelectedGroup = viewModel.Groups[2];
        Assert.AreEqual(1, viewModel.MemberErrorAnnouncementVersion);
        Assert.IsTrue(viewModel.HasDetailError);
        StringAssert.Contains(viewModel.DetailErrorMessage, "Worker folder-member query failed");
        Assert.AreEqual(3, viewModel.MemberStatusAnnouncementVersion);
    }

    [TestMethod]
    public async Task ExplorerRevealPublishesBusyAndActionableFailureWithoutBlockingTheCaller()
    {
        var nativeCompletion = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var nativeStarted = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var explorer = new TestExplorer
        {
            Handler = async (_, _) =>
            {
                nativeStarted.SetResult();
                await nativeCompletion.Task;
                throw new IOException("The folder is offline.");
            },
        };
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage([Group(1, query.RunId, @"C:\One")], 1, null, null)),
            FolderMemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderMemberPage([new(1, query.GroupId, @"C:\One")], 1, null, null)),
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), explorer);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(32, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        var reveal = viewModel.RevealInExplorerCommand.ExecuteAsync(viewModel.SelectedMember);
        await nativeStarted.Task;

        Assert.IsFalse(reveal.IsCompleted);
        Assert.IsTrue(viewModel.IsExplorerCommandRunning);
        StringAssert.Contains(viewModel.ExplorerStatusMessage, "Opening One at C:");

        nativeCompletion.SetResult();
        await reveal;

        Assert.IsFalse(viewModel.IsExplorerCommandRunning);
        Assert.IsFalse(viewModel.HasExplorerStatus);
        StringAssert.Contains(viewModel.ExplorerErrorMessage, "Could not show One at C: in File Explorer");
        StringAssert.Contains(viewModel.ExplorerErrorMessage, "Verify that the location is available, then try again");
        StringAssert.Contains(viewModel.ExplorerErrorMessage, "The folder is offline");
        Assert.AreEqual(1, viewModel.ExplorerErrorAnnouncementVersion);
    }

    [TestMethod]
    public async Task ExplorerRevealLateFailureCannotReplaceNewerSelectedMemberContext()
    {
        var staleCompletion = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var staleStarted = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var explorer = new TestExplorer
        {
            Handler = async (_, _) =>
            {
                staleStarted.SetResult();
                await staleCompletion.Task;
                throw new IOException("Late stale failure.");
            },
        };
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage([Group(1, query.RunId, @"C:\One")], 1, null, null)),
            FolderMemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderMemberPage(
                    [new(1, query.GroupId, @"C:\One"), new(2, query.GroupId, @"D:\Two")],
                    2,
                    null,
                    null)),
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), explorer);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(33, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        var reveal = viewModel.RevealInExplorerCommand.ExecuteAsync(viewModel.Members[0]);
        await staleStarted.Task;
        viewModel.SelectedMember = viewModel.Members[1];

        Assert.IsFalse(viewModel.IsExplorerCommandRunning);
        Assert.IsFalse(viewModel.HasExplorerStatus);
        Assert.IsFalse(viewModel.HasExplorerError);

        staleCompletion.SetResult();
        await reveal;

        Assert.AreEqual(2, viewModel.SelectedMember.Id);
        Assert.IsFalse(viewModel.HasExplorerStatus);
        Assert.IsFalse(viewModel.HasExplorerError);
        Assert.AreEqual(0, viewModel.ExplorerStatusAnnouncementVersion);
        Assert.AreEqual(0, viewModel.ExplorerErrorAnnouncementVersion);
    }

    [TestMethod]
    public async Task ExplorerPageSelectionUsesOnlyTheBoundedCurrentMemberPage()
    {
        var memberQueryCount = 0;
        var explorer = new TestExplorer();
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage([Group(1, query.RunId, @"C:\Shared\One")], 1, null, null)),
            FolderMemberPageHandler = (query, _) =>
            {
                memberQueryCount++;
                return Task.FromResult(new WorkerDuplicateFolderMemberPage(
                    [
                        new(1, query.GroupId, @"C:\Shared\One"),
                        new(2, query.GroupId, @"C:\Shared\Two"),
                        new(3, query.GroupId, @"D:\Other\Three"),
                    ],
                    999,
                    null,
                    null));
            },
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), explorer);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(34, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.IsTrue(viewModel.CanSelectPageInExplorer);
        await viewModel.SelectPageInExplorerCommand.ExecuteAsync(null);

        Assert.AreEqual(1, memberQueryCount);
        Assert.AreEqual(1, explorer.SelectionCallCount);
        CollectionAssert.AreEqual(
            new[] { @"C:\Shared\One", @"C:\Shared\Two", @"D:\Other\Three" },
            explorer.SelectedPaths!.ToArray());
        Assert.AreEqual(
            "File Explorer selected 3 folder copies in 2 parent locations from this page.",
            viewModel.ExplorerStatusMessage);
        Assert.AreEqual(1, viewModel.ExplorerStatusAnnouncementVersion);
        Assert.IsFalse(viewModel.HasExplorerError);
        Assert.IsFalse(viewModel.IsExplorerCommandRunning);
    }

    [TestMethod]
    public async Task ExplorerPageSelectionPublishesActionableAggregatePartialFailure()
    {
        var explorer = new TestExplorer
        {
            SelectionHandler = (_, _) => Task.FromResult(new ExplorerSelectionResult(
                3,
                2,
                2,
                [new ExplorerParentSelectionFailure(@"D:\Offline", 1, "The parent is offline.")])),
        };
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage([Group(1, query.RunId, @"C:\Shared\One")], 1, null, null)),
            FolderMemberPageHandler = (query, _) => Task.FromResult(new WorkerDuplicateFolderMemberPage(
                [
                    new(1, query.GroupId, @"C:\Shared\One"),
                    new(2, query.GroupId, @"C:\Shared\Two"),
                    new(3, query.GroupId, @"D:\Offline\Three"),
                ],
                3,
                null,
                null)),
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), explorer);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(35, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        await viewModel.SelectPageInExplorerCommand.ExecuteAsync(null);

        StringAssert.Contains(viewModel.ExplorerStatusMessage, "selected 2 of 3 folder copies");
        StringAssert.Contains(viewModel.ExplorerStatusMessage, "1 of 2 parent locations");
        StringAssert.Contains(viewModel.ExplorerErrorMessage, "could not select 1 folder copy");
        StringAssert.Contains(viewModel.ExplorerErrorMessage, @"D:\Offline");
        StringAssert.Contains(viewModel.ExplorerErrorMessage, "try this current page again");
        Assert.AreEqual(1, viewModel.ExplorerStatusAnnouncementVersion);
        Assert.AreEqual(1, viewModel.ExplorerErrorAnnouncementVersion);
    }

    [TestMethod]
    public async Task ExplorerPageSelectionLateResultCannotReplaceNewerSelectionContext()
    {
        var selectionStarted = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var selectionCompletion = new TaskCompletionSource<ExplorerSelectionResult>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var explorer = new TestExplorer
        {
            SelectionHandler = async (_, _) =>
            {
                selectionStarted.SetResult();
                return await selectionCompletion.Task;
            },
        };
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage([Group(1, query.RunId, @"C:\Shared\One")], 1, null, null)),
            FolderMemberPageHandler = (query, _) => Task.FromResult(new WorkerDuplicateFolderMemberPage(
                [new(1, query.GroupId, @"C:\Shared\One"), new(2, query.GroupId, @"C:\Shared\Two")],
                2,
                null,
                null)),
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), explorer);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(36, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        var selection = viewModel.SelectPageInExplorerCommand.ExecuteAsync(null);
        await selectionStarted.Task;
        Assert.IsTrue(viewModel.IsExplorerCommandRunning);
        viewModel.SelectedMember = viewModel.Members[1];

        Assert.IsFalse(viewModel.IsExplorerCommandRunning);
        Assert.IsFalse(viewModel.HasExplorerStatus);
        Assert.IsFalse(viewModel.HasExplorerError);

        selectionCompletion.SetResult(new ExplorerSelectionResult(2, 1, 2, []));
        await selection;

        Assert.AreEqual(2, viewModel.SelectedMember.Id);
        Assert.IsFalse(viewModel.HasExplorerStatus);
        Assert.IsFalse(viewModel.HasExplorerError);
        Assert.AreEqual(0, viewModel.ExplorerStatusAnnouncementVersion);
        Assert.AreEqual(0, viewModel.ExplorerErrorAnnouncementVersion);
    }

    [TestMethod]
    public async Task MemberQueryGenerationRejectsLateResponseWithoutAnnouncement()
    {
        var staleResponse = new TaskCompletionSource<WorkerDuplicateFolderMemberPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var staleRequestObserved = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage(
                    [Group(1, query.RunId, @"C:\first"), Group(2, query.RunId, @"C:\second")],
                    2,
                    null,
                    null)),
            FolderMemberPageHandler = (query, _) =>
            {
                if (query.GroupId == 1)
                {
                    staleRequestObserved.TrySetResult();
                    return staleResponse.Task;
                }
                return Task.FromResult(new WorkerDuplicateFolderMemberPage(
                    [new(2, query.GroupId, @"C:\second")],
                    1,
                    null,
                    null));
            },
        };
        using var viewModel = new DuplicateFoldersViewModel(
            client,
            new TestClipboard(),
            new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(15, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        await staleRequestObserved.Task;

        viewModel.SelectedGroup = viewModel.Groups[1];
        Assert.AreEqual(1, viewModel.MemberStatusAnnouncementVersion);
        StringAssert.Contains(viewModel.MemberStatusAnnouncement, "side-by-side location cards");

        staleResponse.SetResult(new WorkerDuplicateFolderMemberPage(
            [new(1, 1, @"C:\stale")],
            1,
            null,
            null));
        await Task.Yield();
        await Task.Yield();

        Assert.AreEqual(@"C:\second", viewModel.Members.Single().Path);
        Assert.AreEqual(1, viewModel.MemberStatusAnnouncementVersion);
        Assert.AreEqual(0, viewModel.MemberErrorAnnouncementVersion);
    }

    [TestMethod]
    public async Task FolderLocationCardsDifferentiatePagedPathsAndCapTheBoundCollection()
    {
        var returnedMembers = Enumerable.Range(0, DuplicateFoldersViewModel.PageSize + 5)
            .Select(index => new WorkerDuplicateFolderMember(
                index + 10,
                1,
                index == 0
                    ? @"C:\Users\Gary\Primary\Photos\2026"
                    : $@"D:\Archive\Location-{index}\Photos\2026"))
            .ToArray();
        var returnedGroups = Enumerable.Range(0, DuplicateFoldersViewModel.PageSize + 5)
            .Select(index => new WorkerDuplicateFolderGroup(
                index + 1,
                41,
                "2048",
                12,
                205,
                returnedMembers[0].Path))
            .ToArray();
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) =>
            {
                Assert.AreEqual(DuplicateFoldersViewModel.PageSize, query.PageSize);
                return Task.FromResult(new WorkerDuplicateFolderGroupPage(
                    returnedGroups,
                    returnedGroups.Length,
                    null,
                    null));
            },
            FolderMemberPageHandler = (query, _) =>
            {
                Assert.AreEqual(DuplicateFoldersViewModel.PageSize, query.PageSize);
                return Task.FromResult(new WorkerDuplicateFolderMemberPage(
                    returnedMembers,
                    returnedMembers.Length,
                    "next",
                    null));
            },
        };
        using var viewModel = new DuplicateFoldersViewModel(
            client,
            new TestClipboard(),
            new TestExplorer());

        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(41, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.AreEqual(DuplicateFoldersViewModel.PageSize, viewModel.Groups.Count);
        Assert.AreEqual(DuplicateFoldersViewModel.PageSize, viewModel.Members.Count);
        Assert.AreEqual(viewModel.Members[0], viewModel.SelectedMember);
        Assert.AreEqual(
            "Showing 200 of 205 folder copies on this server-owned page",
            viewModel.MemberPageStatusText);
        Assert.AreEqual(
            "205 folder copies · 12 files per copy · 2 KB per copy · 408 KB recoverable",
            viewModel.SelectedRelationshipSummaryText);
        Assert.AreEqual("Photos › 2026", viewModel.Members[0].SharedPathContext);
        Assert.AreEqual("C: › Users › Gary › Primary", viewModel.Members[0].DifferingPathSegments);
        Assert.AreEqual("FolderLocationCard-10", viewModel.Members[0].AutomationId);
        StringAssert.Contains(viewModel.Members[0].AutomationName, "different path segments");
        Assert.AreEqual(returnedMembers[0].Path, viewModel.Members[0].Path);
        Assert.IsTrue(viewModel.CanMoveMembersNext);
    }

    [TestMethod]
    public async Task NonCompletedAndEmptyRunsExposeExplicitStates()
    {
        using var viewModel = new DuplicateFoldersViewModel(new TestWorkerClient(), new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(1, 1, "running", "hashing", DateTimeOffset.UtcNow));
        Assert.IsTrue(viewModel.IsUnavailable);
        StringAssert.Contains(viewModel.StateMessage, "after this scan completes");

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(2, 1, "completed", "finalizing", DateTimeOffset.UtcNow));
        Assert.IsTrue(viewModel.IsEmpty);
    }

    [TestMethod]
    public async Task FolderDecisionRefreshesCombinedAndSelectedSummariesAndAnnouncement()
    {
        var revision = 0L;
        var decision = "undecided";
        var client = new TestWorkerClient
        {
            ReviewPlanHandler = (runId, _) => Task.FromResult(new WorkerReviewPlanView(
                new WorkerReviewPlan(revision == 0 ? null : 4, runId, revision == 0 ? "notCreated" : "active", revision, null, null),
                new WorkerReviewPlanSummary(0, 0, 0, 0, revision == 0 ? "0" : "2048", 2)
                {
                    FolderRemoveCount = revision,
                    FolderUndecidedCount = 2 - revision,
                    EffectiveRemovalFileCount = revision,
                    PlannedRemovalPhysicalItemCount = revision,
                    IntactFolderCopyCount = 2 - revision,
                })),
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage([Group(1, query.RunId, @"C:\One")], 1, null, null)),
            FolderMemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderMemberPage(
                    [new WorkerDuplicateFolderMember(10, query.GroupId, @"C:\One") { Decision = decision }],
                    2,
                    null,
                    null)
                {
                    ReviewPlanId = revision == 0 ? null : 4,
                    ReviewRevision = revision,
                    ReviewSummary = new WorkerReviewFolderGroupSummary(query.GroupId, 0, revision, 2 - revision, 2 - revision),
                }),
            ReviewFolderDecisionHandler = (_, runId, groupId, memberId, requested, expected, _) =>
            {
                Assert.AreEqual(21, runId);
                Assert.AreEqual(1, groupId);
                Assert.AreEqual(10, memberId);
                Assert.AreEqual(0, expected);
                revision = 1;
                decision = requested;
                return Task.FromResult(new WorkerReviewFolderDecisionMutation(4, revision, false, requested));
            },
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), new TestExplorer());
        (long RunId, long Revision)? publishedRevision = null;
        viewModel.ReviewRevisionChanged += (runId, appliedRevision) =>
            publishedRevision = (runId, appliedRevision);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(21, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        await viewModel.RemoveFolderCommand.ExecuteAsync(viewModel.Members.Single());

        Assert.AreEqual("Remove", viewModel.Members.Single().Decision);
        Assert.AreEqual(1, viewModel.ReviewPlan.Plan.Revision);
        StringAssert.Contains(viewModel.ReviewPlanSummaryText, "1 folders marked Remove");
        StringAssert.Contains(viewModel.SelectedReviewSummaryText, "1 intact copy remains");
        StringAssert.Contains(viewModel.MemberStatusAnnouncement, @"Folder review decision saved: Remove for C:\One");
        Assert.AreEqual((21L, 1L), publishedRevision);
    }

    [TestMethod]
    public async Task ExternalFileRevisionRefreshesVisibleFolderReviewState()
    {
        var revision = 0L;
        var planQueries = 0;
        var memberQueries = 0;
        var client = new TestWorkerClient
        {
            ReviewPlanHandler = (runId, _) =>
            {
                planQueries++;
                return Task.FromResult(new WorkerReviewPlanView(
                    new WorkerReviewPlan(revision == 0 ? null : 4, runId, revision == 0 ? "notCreated" : "active", revision, null, null),
                    new WorkerReviewPlanSummary(0, 0, revision, 2 - revision, revision == 0 ? "0" : "1024", 2 - revision)
                    {
                        IntactFolderCopyCount = 2 - revision,
                    }));
            },
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage([Group(1, query.RunId, @"C:\One")], 1, null, null)),
            FolderMemberPageHandler = (query, _) =>
            {
                memberQueries++;
                return Task.FromResult(new WorkerDuplicateFolderMemberPage(
                    [new WorkerDuplicateFolderMember(10, query.GroupId, @"C:\One")],
                    2,
                    null,
                    null)
                {
                    ReviewPlanId = revision == 0 ? null : 4,
                    ReviewRevision = revision,
                    ReviewSummary = new WorkerReviewFolderGroupSummary(query.GroupId, 0, 0, 2, 2 - revision),
                });
            },
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(31, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        revision = 1;
        await viewModel.RefreshReviewRevisionAsync(31, revision);

        Assert.AreEqual(1, viewModel.ReviewPlan.Plan.Revision);
        Assert.AreEqual(2, planQueries);
        Assert.AreEqual(2, memberQueries);
        Assert.IsTrue(viewModel.CachedMemberPageCount <= DuplicateFoldersViewModel.CacheCapacity);
    }

    [TestMethod]
    public async Task FolderDecisionOverlapIsActionableAndDoesNotReplaceDurableState()
    {
        var client = new TestWorkerClient
        {
            FolderGroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderGroupPage([Group(1, query.RunId, @"C:\One")], 1, null, null)),
            FolderMemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFolderMemberPage(
                    [new WorkerDuplicateFolderMember(10, query.GroupId, @"C:\One") { Decision = "keep" }],
                    1,
                    null,
                    null)
                {
                    ReviewSummary = new WorkerReviewFolderGroupSummary(query.GroupId, 1, 0, 0, 1),
                }),
            ReviewFolderDecisionHandler = (_, _, _, _, _, _, _) =>
                Task.FromException<WorkerReviewFolderDecisionMutation>(
                    new InvalidOperationException("review_overlap_conflict: conflicting file decision")),
        };
        using var viewModel = new DuplicateFoldersViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(22, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        await viewModel.RemoveFolderCommand.ExecuteAsync(viewModel.Members.Single());

        Assert.AreEqual("Keep", viewModel.Members.Single().Decision);
        StringAssert.Contains(viewModel.DetailErrorMessage, "Clear the contained file or folder decision first");
        Assert.AreEqual(1, viewModel.MemberErrorAnnouncementVersion);
    }

    private static WorkerDuplicateFolderGroup Group(long id, long runId, string path) =>
        new(id, runId, "2048", 2, 2, path);
}
