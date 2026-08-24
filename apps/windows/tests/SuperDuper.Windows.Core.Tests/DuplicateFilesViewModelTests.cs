using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class DuplicateFilesViewModelTests
{
    [TestMethod]
    public async Task CompletedRunLoadsMasterDetailAndExecutesPathActions()
    {
        DuplicateFileGroupQuery? lastGroupQuery = null;
        DuplicateFileSelectedRootFacetQuery? lastRootFacetQuery = null;
        DuplicateFileDriveFacetQuery? lastDriveFacetQuery = null;
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                lastGroupQuery = query;
                return Task.FromResult(new WorkerDuplicateFileGroupPage(
                    [Group(1, query.RunId, "photo.jpg")],
                    1,
                    null,
                    null)
                {
                    Summary = new WorkerDuplicateFileReviewSummary(1, 2, "4096", "2048")
                    {
                        DistinctSelectedRootCount = 2,
                        DistinctDriveCount = 2,
                        AcrossDriveGroupCount = 1,
                    },
                });
            },
            RootFacetPageHandler = (query, _) =>
            {
                lastRootFacetQuery = query;
                return Task.FromResult(new WorkerDuplicateFileSelectedRootFacetPage(
                    [new WorkerDuplicateFileSelectedRootFacet(@"C:\Photos", 1)],
                    1,
                    null,
                    null));
            },
            DriveFacetPageHandler = (query, _) =>
            {
                lastDriveFacetQuery = query;
                return Task.FromResult(new WorkerDuplicateFileDriveFacetPage(
                    [new WorkerDuplicateFileDriveFacet("C:", 1)],
                    1,
                    null,
                    null));
            },
            MemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileMemberPage(
                    [Member(1, query.GroupId, @"C:\Photos\photo.jpg")],
                    1,
                    null,
                    null)),
        };
        var clipboard = new TestClipboard();
        var explorer = new TestExplorer();
        using var viewModel = new DuplicateFilesViewModel(client, clipboard, explorer);

        await viewModel.ShowRunAsync(TestWorkerClient.CreateRun(7, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.AreEqual(1, viewModel.Groups.Count);
        Assert.AreEqual(1, viewModel.Members.Count);
        Assert.AreEqual("photo.jpg", viewModel.Groups[0].RepresentativeName);
        Assert.AreEqual("1", viewModel.MatchingSetCountText);
        Assert.AreEqual("2", viewModel.MatchingCopyCountText);
        Assert.AreEqual("4 KB", viewModel.PotentialRecoverableText);
        Assert.AreEqual("2 KB", viewModel.LargestOpportunityText);
        Assert.AreEqual(
            "2 selected roots represented · 2 drives represented · 1 set spans multiple drives",
            viewModel.LocationCoverageText);
        Assert.AreEqual(1, viewModel.GroupStatusAnnouncementVersion);
        Assert.AreEqual(
            "Duplicate file query complete. 1 matching set, 2 copies, 4 KB potentially recoverable. "
                + "2 selected roots represented · 2 drives represented · 1 set spans multiple drives.",
            viewModel.GroupStatusAnnouncement);
        Assert.AreEqual(1, viewModel.SelectedSetStatusAnnouncementVersion);
        Assert.AreEqual(
            "Selected duplicate set loaded: photo.jpg. 1 copy. 2 selected roots · across 2 drives. "
                + "Exact content was verified at scan time; the representative label does not identify an original.",
            viewModel.SelectedSetStatusAnnouncement);
        Assert.AreEqual("2 selected roots · across 2 drives", viewModel.Groups[0].LocationSpan);
        Assert.AreEqual(@"C:\Photos", viewModel.Members[0].SelectedRoot);
        Assert.AreEqual("photo.jpg", viewModel.Members[0].RelativePath);
        Assert.AreEqual("C:", viewModel.Members[0].Drive);
        Assert.AreEqual(2, viewModel.SelectedRootFacetOptions.Count);
        Assert.AreEqual("1 selected root", viewModel.RootFacetCountText);
        Assert.AreEqual("C:\\Photos · 1 set", viewModel.SelectedRootFacetOptions[1].DisplayText);
        Assert.AreEqual(2, viewModel.DriveFacetOptions.Count);
        Assert.AreEqual("1 drive", viewModel.DriveFacetCountText);
        Assert.AreEqual("C: · 1 set", viewModel.DriveFacetOptions[1].DisplayText);
        viewModel.CopyPathCommand.Execute(viewModel.Members[0]);
        await viewModel.RevealInExplorerCommand.ExecuteAsync(viewModel.Members[0]);
        Assert.AreEqual(@"C:\Photos\photo.jpg", clipboard.Text);
        Assert.AreEqual(@"C:\Photos\photo.jpg", explorer.RevealedPath);

        viewModel.AcrossDrives = true;
        viewModel.ThreeOrMoreCopies = true;
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.IsTrue(lastGroupQuery!.Filter.AcrossDrives);
        Assert.IsTrue(lastRootFacetQuery!.Filter.AcrossDrives);
        Assert.IsTrue(lastDriveFacetQuery!.Filter.AcrossDrives);
        Assert.AreEqual(3, lastGroupQuery.Filter.MinimumCopyCount);
        Assert.AreEqual(3, lastRootFacetQuery.Filter.MinimumCopyCount);
        Assert.AreEqual(3, lastDriveFacetQuery.Filter.MinimumCopyCount);

        viewModel.SelectedRootFacet = viewModel.SelectedRootFacetOptions[1];
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.AreEqual(@"C:\Photos", lastGroupQuery.Filter.SelectedRoot);
        Assert.AreEqual(@"C:\Photos", lastDriveFacetQuery.Filter.SelectedRoot);
        Assert.AreEqual("Filtering sets represented under C:\\Photos", viewModel.SelectedRootFilterText);

        viewModel.SelectedDriveFacet = viewModel.DriveFacetOptions[1];
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.AreEqual("C:", lastGroupQuery.Filter.SelectedDrive);
        Assert.AreEqual("C:", lastRootFacetQuery.Filter.SelectedDrive);
        Assert.AreEqual("Filtering sets represented on C:", viewModel.SelectedDriveFilterText);

        await viewModel.SortRootFacetsByNameCommand.ExecuteAsync(null);
        Assert.AreEqual(DuplicateFileSelectedRootFacetSortField.Value, lastRootFacetQuery.SortField);
        Assert.AreEqual(WorkerSortDirection.Ascending, lastRootFacetQuery.SortDirection);

        await viewModel.SortDriveFacetsByNameCommand.ExecuteAsync(null);
        Assert.AreEqual(DuplicateFileDriveFacetSortField.Value, lastDriveFacetQuery.SortField);
        Assert.AreEqual(WorkerSortDirection.Ascending, lastDriveFacetQuery.SortDirection);

        await viewModel.ClearFiltersCommand.ExecuteAsync(null);
        Assert.IsFalse(viewModel.AcrossDrives);
        Assert.IsFalse(viewModel.ThreeOrMoreCopies);
        Assert.IsFalse(lastGroupQuery.Filter.AcrossDrives);
        Assert.AreEqual(2, lastGroupQuery.Filter.MinimumCopyCount);
        Assert.AreEqual(2, lastRootFacetQuery.Filter.MinimumCopyCount);
        Assert.AreEqual(2, lastDriveFacetQuery.Filter.MinimumCopyCount);
        Assert.IsNull(lastGroupQuery.Filter.SelectedRoot);
        Assert.IsNull(lastGroupQuery.Filter.SelectedDrive);
    }

    [TestMethod]
    public async Task NewFilterGenerationRejectsLateOldResponse()
    {
        var oldResponse = new TaskCompletionSource<WorkerDuplicateFileGroupPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var firstRequestObserved = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                if (query.Filter.Search.Length == 0)
                {
                    firstRequestObserved.TrySetResult();
                    return oldResponse.Task;
                }
                return Task.FromResult(new WorkerDuplicateFileGroupPage(
                    [Group(2, query.RunId, "new-result.bin")],
                    1,
                    null,
                    null)
                {
                    Summary = new WorkerDuplicateFileReviewSummary(1, 2, "222", "111")
                    {
                        DistinctSelectedRootCount = 1,
                        DistinctDriveCount = 1,
                        AcrossDriveGroupCount = 0,
                    },
                });
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        var initialLoad = viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(8, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        await firstRequestObserved.Task;

        viewModel.SearchText = "new";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        oldResponse.SetResult(new WorkerDuplicateFileGroupPage(
            [Group(1, 8, "stale-result.bin")],
            9,
            null,
            null)
        {
            Summary = new WorkerDuplicateFileReviewSummary(9, 18, "999", "999")
            {
                DistinctSelectedRootCount = 9,
                DistinctDriveCount = 9,
                AcrossDriveGroupCount = 9,
            },
        });
        await initialLoad;

        Assert.AreEqual(1, viewModel.Groups.Count);
        Assert.AreEqual("new-result.bin", viewModel.Groups[0].RepresentativeName);
        Assert.AreEqual("222 B", viewModel.PotentialRecoverableText);
        Assert.AreEqual("111 B", viewModel.LargestOpportunityText);
        Assert.AreEqual(
            "1 selected root represented · 1 drive represented · no sets span multiple drives",
            viewModel.LocationCoverageText);
    }

    [TestMethod]
    public async Task OneGigabytePresetNormalizesMinimumSizeAcrossGroupsAndIndependentFacets()
    {
        DuplicateFileGroupQuery? groupQuery = null;
        DuplicateFileSelectedRootFacetQuery? rootFacetQuery = null;
        DuplicateFileDriveFacetQuery? driveFacetQuery = null;
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                groupQuery = query;
                return Task.FromResult(new WorkerDuplicateFileGroupPage([], 0, null, null));
            },
            RootFacetPageHandler = (query, _) =>
            {
                rootFacetQuery = query;
                return Task.FromResult(new WorkerDuplicateFileSelectedRootFacetPage([], 0, null, null));
            },
            DriveFacetPageHandler = (query, _) =>
            {
                driveFacetQuery = query;
                return Task.FromResult(new WorkerDuplicateFileDriveFacetPage([], 0, null, null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(14, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        viewModel.MinimumSizeText = "4096";
        viewModel.OneGigabyteOrLarger = true;
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);

        const string oneGigabyte = "1073741824";
        Assert.AreEqual(oneGigabyte, groupQuery!.Filter.MinimumSize);
        Assert.AreEqual(oneGigabyte, rootFacetQuery!.Filter.MinimumSize);
        Assert.AreEqual(oneGigabyte, driveFacetQuery!.Filter.MinimumSize);

        viewModel.MinimumSizeText = "2147483648";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.AreEqual("2147483648", groupQuery.Filter.MinimumSize);
        Assert.AreEqual("2147483648", rootFacetQuery.Filter.MinimumSize);
        Assert.AreEqual("2147483648", driveFacetQuery.Filter.MinimumSize);

        await viewModel.ClearFiltersCommand.ExecuteAsync(null);
        Assert.IsFalse(viewModel.OneGigabyteOrLarger);
        Assert.AreEqual("0", groupQuery.Filter.MinimumSize);
        Assert.AreEqual("0", rootFacetQuery.Filter.MinimumSize);
        Assert.AreEqual("0", driveFacetQuery.Filter.MinimumSize);
    }

    [TestMethod]
    public async Task ExactPathModeFlowsThroughGroupsAndIndependentFacetsAndClearsAtomically()
    {
        DuplicateFileGroupQuery? groupQuery = null;
        DuplicateFileSelectedRootFacetQuery? rootFacetQuery = null;
        DuplicateFileDriveFacetQuery? driveFacetQuery = null;
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                groupQuery = query;
                return Task.FromResult(new WorkerDuplicateFileGroupPage([], 0, null, null));
            },
            RootFacetPageHandler = (query, _) =>
            {
                rootFacetQuery = query;
                return Task.FromResult(new WorkerDuplicateFileSelectedRootFacetPage([], 0, null, null));
            },
            DriveFacetPageHandler = (query, _) =>
            {
                driveFacetQuery = query;
                return Task.FromResult(new WorkerDuplicateFileDriveFacetPage([], 0, null, null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(18, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        const string exactPath = @"C:\Data\Überraschung.TXT ";
        viewModel.SearchText = exactPath;
        viewModel.ExactPathMatch = true;
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);

        Assert.AreEqual(exactPath, groupQuery!.Filter.Search);
        Assert.AreEqual(exactPath, rootFacetQuery!.Filter.Search);
        Assert.AreEqual(exactPath, driveFacetQuery!.Filter.Search);
        Assert.AreEqual(DuplicateFilePathMatchMode.Exact, groupQuery.Filter.PathMatch);
        Assert.AreEqual(DuplicateFilePathMatchMode.Exact, rootFacetQuery.Filter.PathMatch);
        Assert.AreEqual(DuplicateFilePathMatchMode.Exact, driveFacetQuery.Filter.PathMatch);

        await viewModel.ClearFiltersCommand.ExecuteAsync(null);

        Assert.IsFalse(viewModel.ExactPathMatch);
        Assert.AreEqual(string.Empty, viewModel.SearchText);
        Assert.AreEqual(DuplicateFilePathMatchMode.Substring, groupQuery.Filter.PathMatch);
        Assert.AreEqual(DuplicateFilePathMatchMode.Substring, rootFacetQuery.Filter.PathMatch);
        Assert.AreEqual(DuplicateFilePathMatchMode.Substring, driveFacetQuery.Filter.PathMatch);
    }

    [TestMethod]
    public async Task ExactPathLimitCountsUnicodeScalarsRatherThanUtf16CodeUnits()
    {
        DuplicateFileGroupQuery? groupQuery = null;
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                groupQuery = query;
                return Task.FromResult(new WorkerDuplicateFileGroupPage([], 0, null, null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(19, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        var maximumScalarPath = string.Concat(Enumerable.Repeat("\U0001F600", DuplicateFilesViewModel.MaximumExactPathCharacters));
        viewModel.SearchText = maximumScalarPath;
        viewModel.ExactPathMatch = true;
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);

        Assert.AreEqual(maximumScalarPath, groupQuery!.Filter.Search);
        Assert.IsFalse(viewModel.HasError);

        groupQuery = null;
        viewModel.SearchText += "x";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);

        Assert.IsNull(groupQuery);
        StringAssert.Contains(viewModel.ErrorMessage, "32,767");
        Assert.AreEqual(1, viewModel.GroupErrorAnnouncementVersion);
        StringAssert.StartsWith(viewModel.GroupErrorAnnouncement, "Duplicate file filters could not be applied.");
    }

    [TestMethod]
    public async Task ExtensionMatchModesAndNoExtensionFlowThroughGroupsAndIndependentFacets()
    {
        DuplicateFileGroupQuery? groupQuery = null;
        DuplicateFileSelectedRootFacetQuery? rootFacetQuery = null;
        DuplicateFileDriveFacetQuery? driveFacetQuery = null;
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                groupQuery = query;
                return Task.FromResult(new WorkerDuplicateFileGroupPage([], 0, null, null));
            },
            RootFacetPageHandler = (query, _) =>
            {
                rootFacetQuery = query;
                return Task.FromResult(new WorkerDuplicateFileSelectedRootFacetPage([], 0, null, null));
            },
            DriveFacetPageHandler = (query, _) =>
            {
                driveFacetQuery = query;
                return Task.FromResult(new WorkerDuplicateFileDriveFacetPage([], 0, null, null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(20, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        viewModel.ExtensionText = "JPG";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);

        Assert.AreEqual("JPG", groupQuery!.Filter.Extension);
        Assert.AreEqual("JPG", rootFacetQuery!.Filter.Extension);
        Assert.AreEqual("JPG", driveFacetQuery!.Filter.Extension);
        Assert.AreEqual(DuplicateFileExtensionMatchMode.AnyMember, groupQuery.Filter.ExtensionMatch);
        Assert.AreEqual(DuplicateFileExtensionMatchMode.AnyMember, rootFacetQuery.Filter.ExtensionMatch);
        Assert.AreEqual(DuplicateFileExtensionMatchMode.AnyMember, driveFacetQuery.Filter.ExtensionMatch);

        viewModel.AllMembersMustMatchExtension = true;
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);

        Assert.AreEqual(DuplicateFileExtensionMatchMode.AllMembers, groupQuery.Filter.ExtensionMatch);
        Assert.AreEqual(DuplicateFileExtensionMatchMode.AllMembers, rootFacetQuery.Filter.ExtensionMatch);
        Assert.AreEqual(DuplicateFileExtensionMatchMode.AllMembers, driveFacetQuery.Filter.ExtensionMatch);

        viewModel.WithoutExtension = true;
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);

        Assert.AreEqual(string.Empty, groupQuery.Filter.Extension);
        Assert.AreEqual(string.Empty, rootFacetQuery.Filter.Extension);
        Assert.AreEqual(string.Empty, driveFacetQuery.Filter.Extension);
        Assert.AreEqual(DuplicateFileExtensionMatchMode.AllMembers, groupQuery.Filter.ExtensionMatch);
        Assert.AreEqual(DuplicateFileExtensionMatchMode.AllMembers, rootFacetQuery.Filter.ExtensionMatch);
        Assert.AreEqual(DuplicateFileExtensionMatchMode.AllMembers, driveFacetQuery.Filter.ExtensionMatch);

        groupQuery = null;
        viewModel.WithoutExtension = false;
        viewModel.ExtensionText = "tar.gz";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);

        Assert.IsNull(groupQuery);
        StringAssert.Contains(viewModel.ErrorMessage, "without a dot");

        await viewModel.ClearFiltersCommand.ExecuteAsync(null);
        Assert.AreEqual(string.Empty, viewModel.ExtensionText);
        Assert.IsFalse(viewModel.WithoutExtension);
        Assert.IsFalse(viewModel.AllMembersMustMatchExtension);
        Assert.IsNull(groupQuery!.Filter.Extension);
        Assert.IsNull(rootFacetQuery.Filter.Extension);
        Assert.IsNull(driveFacetQuery.Filter.Extension);
        Assert.AreEqual(DuplicateFileExtensionMatchMode.AnyMember, groupQuery.Filter.ExtensionMatch);
    }

    [TestMethod]
    public async Task SetNavigationMovesWithinAndAcrossExistingBoundedGroupPages()
    {
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(query.Cursor switch
            {
                "next-page" => new WorkerDuplicateFileGroupPage(
                    [Group(3, query.RunId, "third.bin"), Group(4, query.RunId, "fourth.bin")],
                    4,
                    null,
                    "previous-page"),
                "previous-page" => new WorkerDuplicateFileGroupPage(
                    [Group(1, query.RunId, "first.bin"), Group(2, query.RunId, "second.bin")],
                    4,
                    "next-page",
                    null),
                _ => new WorkerDuplicateFileGroupPage(
                    [Group(1, query.RunId, "first.bin"), Group(2, query.RunId, "second.bin")],
                    4,
                    "next-page",
                    null),
            }),
            MemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileMemberPage(
                    [Member(query.GroupId, query.GroupId, $@"C:\Data\{query.GroupId}.bin")],
                    1,
                    null,
                    null)),
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(15, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.AreEqual(1, viewModel.SelectedGroup!.Id);
        Assert.IsFalse(viewModel.CanMoveToPreviousSet);
        Assert.IsTrue(viewModel.CanMoveToNextSet);

        await viewModel.NextSetCommand.ExecuteAsync(null);
        Assert.AreEqual(2, viewModel.SelectedGroup!.Id);

        await viewModel.NextSetCommand.ExecuteAsync(null);
        Assert.AreEqual(3, viewModel.SelectedGroup!.Id);
        Assert.AreEqual("third.bin", viewModel.SelectedGroup.RepresentativeName);

        await viewModel.PreviousSetCommand.ExecuteAsync(null);
        Assert.AreEqual(2, viewModel.SelectedGroup!.Id);
        Assert.AreEqual("second.bin", viewModel.SelectedGroup.RepresentativeName);
        Assert.IsTrue(viewModel.CachedGroupPageCount <= DuplicateFilesViewModel.CacheCapacity);
    }

    [TestMethod]
    public async Task SetNavigationRejectsLateMembersFromPreviouslySelectedGroup()
    {
        var oldResponse = new TaskCompletionSource<WorkerDuplicateFileMemberPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var oldRequestObserved = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage(
                    [Group(1, query.RunId, "first.bin"), Group(2, query.RunId, "second.bin")],
                    2,
                    null,
                    null)),
            MemberPageHandler = (query, _) =>
            {
                if (query.GroupId == 1)
                {
                    oldRequestObserved.TrySetResult();
                    return oldResponse.Task;
                }
                return Task.FromResult(new WorkerDuplicateFileMemberPage(
                    [Member(2, query.GroupId, @"C:\Data\second.bin")],
                    1,
                    null,
                    null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(16, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        await oldRequestObserved.Task;

        await viewModel.NextSetCommand.ExecuteAsync(null);
        Assert.AreEqual(2, viewModel.SelectedGroup!.Id);
        Assert.AreEqual(@"C:\Data\second.bin", viewModel.Members.Single().Path);
        Assert.AreEqual(1, viewModel.SelectedSetStatusAnnouncementVersion);
        StringAssert.Contains(viewModel.SelectedSetStatusAnnouncement, "second.bin");

        oldResponse.SetResult(new WorkerDuplicateFileMemberPage(
            [Member(1, 1, @"C:\Data\stale-first.bin")],
            1,
            null,
            null));
        await Task.Yield();
        await Task.Yield();

        Assert.AreEqual(2, viewModel.SelectedGroup!.Id);
        Assert.AreEqual(@"C:\Data\second.bin", viewModel.Members.Single().Path);
        Assert.AreEqual(1, viewModel.SelectedSetStatusAnnouncementVersion);
    }

    [TestMethod]
    public async Task SelectedSetAnnouncementsRepeatForCachedPagesAndCoverEmptyAndWorkerFailure()
    {
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage(
                    [
                        Group(1, query.RunId, "first.bin"),
                        Group(2, query.RunId, "empty.bin"),
                        Group(3, query.RunId, "failed.bin"),
                    ],
                    3,
                    null,
                    null)),
            MemberPageHandler = (query, _) => query.GroupId switch
            {
                1 when query.Cursor is null => Task.FromResult(
                    new WorkerDuplicateFileMemberPage(
                        [Member(1, query.GroupId, @"C:\Data\first.bin")],
                        2,
                        "next-members",
                        null)),
                1 => Task.FromResult(
                    new WorkerDuplicateFileMemberPage(
                        [Member(2, query.GroupId, @"D:\Backup\first.bin")],
                        2,
                        null,
                        "previous-members")),
                2 => Task.FromResult(new WorkerDuplicateFileMemberPage([], 0, null, null)),
                _ => Task.FromException<WorkerDuplicateFileMemberPage>(
                    new IOException("Worker member query failed.")),
            },
        };
        var explorer = new TestExplorer { Error = new IOException("Explorer action failed.") };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), explorer);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(17, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.AreEqual(1, viewModel.SelectedSetStatusAnnouncementVersion);
        var repeatedAnnouncement = viewModel.SelectedSetStatusAnnouncement;
        await viewModel.RevealInExplorerCommand.ExecuteAsync(viewModel.Members[0]);
        Assert.IsTrue(viewModel.HasDetailError);
        await viewModel.NextMemberPageCommand.ExecuteAsync(null);
        Assert.AreEqual(2, viewModel.SelectedSetStatusAnnouncementVersion);
        Assert.AreEqual(repeatedAnnouncement, viewModel.SelectedSetStatusAnnouncement);
        Assert.IsFalse(viewModel.HasDetailError);

        viewModel.SelectedGroup = viewModel.Groups[1];
        Assert.AreEqual(3, viewModel.SelectedSetStatusAnnouncementVersion);
        Assert.AreEqual(
            "Selected duplicate set loaded: empty.bin. No copies to display.",
            viewModel.SelectedSetStatusAnnouncement);

        viewModel.SelectedGroup = viewModel.Groups[2];
        Assert.AreEqual(1, viewModel.SelectedSetErrorAnnouncementVersion);
        Assert.IsTrue(viewModel.HasDetailError);
        StringAssert.Contains(viewModel.DetailErrorMessage, "Worker member query failed");
        Assert.AreEqual(3, viewModel.SelectedSetStatusAnnouncementVersion);
    }

    [TestMethod]
    public async Task FacetPagingAndSortAnnouncementsCoverCachedPagesEmptyAndWorkerFailure()
    {
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage(
                    [Group(1, query.RunId, "item.bin")],
                    1,
                    null,
                    null)),
            RootFacetPageHandler = (query, _) =>
            {
                if (query.SortField == DuplicateFileSelectedRootFacetSortField.Value)
                {
                    return Task.FromResult(new WorkerDuplicateFileSelectedRootFacetPage([], 0, null, null));
                }
                var page = query.Cursor is null ? 0 : int.Parse(query.Cursor);
                return Task.FromResult(new WorkerDuplicateFileSelectedRootFacetPage(
                    [new WorkerDuplicateFileSelectedRootFacet($"root-{page}", 2 - page)],
                    2,
                    page == 0 ? "1" : null,
                    page == 1 ? "0" : null));
            },
            DriveFacetPageHandler = (query, _) =>
            {
                if (query.SortField == DuplicateFileDriveFacetSortField.Value)
                {
                    return Task.FromException<WorkerDuplicateFileDriveFacetPage>(
                        new InvalidOperationException("Worker drive facet query failed."));
                }
                var page = query.Cursor is null ? 0 : int.Parse(query.Cursor);
                return Task.FromResult(new WorkerDuplicateFileDriveFacetPage(
                    [new WorkerDuplicateFileDriveFacet($"drive-{page}", 2 - page)],
                    2,
                    page == 0 ? "1" : null,
                    page == 1 ? "0" : null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());

        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(14, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.AreEqual(0, viewModel.RootFacetStatusAnnouncementVersion);
        Assert.AreEqual(0, viewModel.DriveFacetStatusAnnouncementVersion);

        await viewModel.NextRootFacetPageCommand.ExecuteAsync(null);
        Assert.AreEqual(1, viewModel.RootFacetStatusAnnouncementVersion);
        Assert.AreEqual(
            "Selected-root facet page loaded. 1 selected root shown of 2 selected roots, sorted by most matching sets.",
            viewModel.RootFacetStatusAnnouncement);
        var repeatedRootAnnouncement = viewModel.RootFacetStatusAnnouncement;
        await viewModel.PreviousRootFacetPageCommand.ExecuteAsync(null);
        Assert.AreEqual(2, viewModel.RootFacetStatusAnnouncementVersion);
        Assert.AreEqual(repeatedRootAnnouncement, viewModel.RootFacetStatusAnnouncement);

        await viewModel.NextDriveFacetPageCommand.ExecuteAsync(null);
        Assert.AreEqual(1, viewModel.DriveFacetStatusAnnouncementVersion);
        Assert.AreEqual(
            "Drive facet page loaded. 1 drive shown of 2 drives, sorted by most matching sets.",
            viewModel.DriveFacetStatusAnnouncement);
        var repeatedDriveAnnouncement = viewModel.DriveFacetStatusAnnouncement;
        await viewModel.PreviousDriveFacetPageCommand.ExecuteAsync(null);
        Assert.AreEqual(2, viewModel.DriveFacetStatusAnnouncementVersion);
        Assert.AreEqual(repeatedDriveAnnouncement, viewModel.DriveFacetStatusAnnouncement);

        await viewModel.SortRootFacetsByNameCommand.ExecuteAsync(null);
        Assert.AreEqual(3, viewModel.RootFacetStatusAnnouncementVersion);
        Assert.AreEqual(
            "Selected-root facet page loaded. No selected roots are available for the current filters.",
            viewModel.RootFacetStatusAnnouncement);

        await viewModel.SortDriveFacetsByNameCommand.ExecuteAsync(null);
        Assert.AreEqual(1, viewModel.DriveFacetErrorAnnouncementVersion);
        Assert.IsTrue(viewModel.HasDriveFacetError);
        StringAssert.Contains(viewModel.DriveFacetErrorMessage, "Worker drive facet query failed");
        Assert.AreEqual(2, viewModel.DriveFacetStatusAnnouncementVersion);
    }

    [TestMethod]
    public async Task RootFacetGenerationRejectsLateResponseAndCacheStaysBounded()
    {
        var oldResponse = new TaskCompletionSource<WorkerDuplicateFileSelectedRootFacetPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var oldRequestObserved = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage(
                    [Group(1, query.RunId, "item.bin")],
                    1,
                    null,
                    null)),
            RootFacetPageHandler = (query, _) =>
            {
                if (query.SortField == DuplicateFileSelectedRootFacetSortField.Value
                    && query.Filter.Search.Length == 0)
                {
                    oldRequestObserved.TrySetResult();
                    return oldResponse.Task;
                }
                var page = query.Cursor is null ? 0 : int.Parse(query.Cursor);
                return Task.FromResult(new WorkerDuplicateFileSelectedRootFacetPage(
                    [new WorkerDuplicateFileSelectedRootFacet($"new-root-{page}", 10 - page)],
                    10,
                    page < 9 ? (page + 1).ToString() : null,
                    page > 0 ? (page - 1).ToString() : null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(12, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        var oldSort = viewModel.SortRootFacetsByNameCommand.ExecuteAsync(null);
        await oldRequestObserved.Task;

        viewModel.SearchText = "new";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        oldResponse.SetResult(new WorkerDuplicateFileSelectedRootFacetPage(
            [new WorkerDuplicateFileSelectedRootFacet("stale-root", 99)],
            1,
            null,
            null));
        await oldSort;

        Assert.AreEqual("new-root-0", viewModel.SelectedRootFacetOptions[1].Value);
        Assert.AreEqual(0, viewModel.RootFacetStatusAnnouncementVersion);
        for (var page = 1; page < 9; page++)
        {
            await viewModel.NextRootFacetPageCommand.ExecuteAsync(null);
            Assert.IsTrue(viewModel.CachedRootFacetPageCount <= DuplicateFilesViewModel.CacheCapacity);
            Assert.AreEqual($"new-root-{page}", viewModel.SelectedRootFacetOptions[1].Value);
            Assert.AreEqual(page, viewModel.RootFacetStatusAnnouncementVersion);
        }
    }

    [TestMethod]
    public async Task DriveFacetGenerationRejectsLateResponseAndCacheStaysBounded()
    {
        var oldResponse = new TaskCompletionSource<WorkerDuplicateFileDriveFacetPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var oldRequestObserved = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage(
                    [Group(1, query.RunId, "item.bin")],
                    1,
                    null,
                    null)),
            DriveFacetPageHandler = (query, _) =>
            {
                if (query.SortField == DuplicateFileDriveFacetSortField.Value
                    && query.Filter.Search.Length == 0)
                {
                    oldRequestObserved.TrySetResult();
                    return oldResponse.Task;
                }
                var page = query.Cursor is null ? 0 : int.Parse(query.Cursor);
                return Task.FromResult(new WorkerDuplicateFileDriveFacetPage(
                    [new WorkerDuplicateFileDriveFacet($"drive-{page}", 10 - page)],
                    10,
                    page < 9 ? (page + 1).ToString() : null,
                    page > 0 ? (page - 1).ToString() : null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(13, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        var oldSort = viewModel.SortDriveFacetsByNameCommand.ExecuteAsync(null);
        await oldRequestObserved.Task;

        viewModel.SearchText = "new";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        oldResponse.SetResult(new WorkerDuplicateFileDriveFacetPage(
            [new WorkerDuplicateFileDriveFacet("stale-drive", 99)],
            1,
            null,
            null));
        await oldSort;

        Assert.AreEqual("drive-0", viewModel.DriveFacetOptions[1].Value);
        Assert.AreEqual(0, viewModel.DriveFacetStatusAnnouncementVersion);
        for (var page = 1; page < 9; page++)
        {
            await viewModel.NextDriveFacetPageCommand.ExecuteAsync(null);
            Assert.IsTrue(viewModel.CachedDriveFacetPageCount <= DuplicateFilesViewModel.CacheCapacity);
            Assert.AreEqual($"drive-{page}", viewModel.DriveFacetOptions[1].Value);
            Assert.AreEqual(page, viewModel.DriveFacetStatusAnnouncementVersion);
        }
    }

    [TestMethod]
    public async Task ResortKeepsDisplayedResultsUntilReplacementPageArrives()
    {
        var replacement = new TaskCompletionSource<WorkerDuplicateFileGroupPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var resortObserved = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                if (query.SortDirection == WorkerSortDirection.Ascending)
                {
                    resortObserved.TrySetResult();
                    return replacement.Task;
                }
                return Task.FromResult(new WorkerDuplicateFileGroupPage(
                    [Group(1, query.RunId, "before-sort.bin")], 1, null, null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(11, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        var resort = viewModel.ApplySortAsync(
            DuplicateFileGroupSortField.RecoverableBytes,
            WorkerSortDirection.Ascending);
        await resortObserved.Task;

        Assert.IsTrue(viewModel.IsLoading);
        Assert.IsFalse(viewModel.IsEmpty);
        Assert.IsFalse(viewModel.IsLoadingOverlayVisible);
        Assert.AreEqual("before-sort.bin", viewModel.Groups.Single().RepresentativeName);

        replacement.SetResult(new WorkerDuplicateFileGroupPage(
            [Group(2, 11, "after-sort.bin")], 1, null, null));
        await resort;

        Assert.IsFalse(viewModel.IsLoading);
        Assert.AreEqual("after-sort.bin", viewModel.Groups.Single().RepresentativeName);
    }

    [TestMethod]
    public async Task PageCacheNeverExceedsTwoPagesOnEitherSide()
    {
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) =>
            {
                var page = query.Cursor is null ? 0 : int.Parse(query.Cursor);
                return Task.FromResult(new WorkerDuplicateFileGroupPage(
                    [Group(page + 1, query.RunId, $"page-{page}.bin")],
                    10,
                    page < 9 ? (page + 1).ToString() : null,
                    page > 0 ? (page - 1).ToString() : null));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(9, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        Assert.AreEqual(1, viewModel.GroupStatusAnnouncementVersion);

        for (var page = 1; page < 9; page++)
        {
            await viewModel.NextPageCommand.ExecuteAsync(null);
            Assert.IsTrue(viewModel.CachedGroupPageCount <= DuplicateFilesViewModel.CacheCapacity);
            Assert.AreEqual($"page-{page}.bin", viewModel.Groups[0].RepresentativeName);
            Assert.AreEqual(page + 1, viewModel.GroupStatusAnnouncementVersion);
        }
    }

    [TestMethod]
    public async Task InvalidMinimumSizeAndExplorerFailureBecomeActionableStates()
    {
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([Group(1, query.RunId, "item.bin")], 1, null, null)),
            MemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileMemberPage([Member(1, query.GroupId, @"C:\Data\item.bin")], 1, null, null)),
        };
        var explorer = new TestExplorer { Error = new IOException("Explorer could not open the item.") };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), explorer);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(10, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        viewModel.MinimumSizeText = "-1";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.IsTrue(viewModel.HasError);
        StringAssert.Contains(viewModel.ErrorMessage, "non-negative");

        await viewModel.RevealInExplorerCommand.ExecuteAsync(viewModel.Members[0]);
        Assert.IsTrue(viewModel.HasDetailError);
        StringAssert.Contains(viewModel.DetailErrorMessage, "Explorer could not open");
    }

    [TestMethod]
    public async Task ManualReviewDecisionRefreshesPersistedMemberAndBoundedSummaries()
    {
        var revision = 0L;
        var decision = "undecided";
        long? observedExpectedRevision = null;
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([Group(1, query.RunId, "item.bin")], 1, null, null)),
            ReviewPlanHandler = (runId, _) => Task.FromResult(new WorkerReviewPlanView(
                new WorkerReviewPlan(revision == 0 ? null : 7, runId, revision == 0 ? "notCreated" : "active", revision, null, null),
                new WorkerReviewPlanSummary(
                    revision == 0 ? 0 : 1,
                    0,
                    decision == "remove" ? 1 : 0,
                    decision == "remove" ? 1 : 2,
                    decision == "remove" ? "1024" : "0",
                    decision == "remove" ? 1 : 2))),
            MemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileMemberPage(
                    [Member(1, query.GroupId, @"C:\Data\item.bin") with { Decision = decision },
                     Member(2, query.GroupId, @"D:\Backup\item.bin")],
                    2,
                    null,
                    null)
                {
                    ReviewPlanId = revision == 0 ? null : 7,
                    ReviewRevision = revision,
                    ReviewSummary = new WorkerReviewGroupSummary(
                        query.GroupId,
                        0,
                        decision == "remove" ? 1 : 0,
                        decision == "remove" ? 1 : 2,
                        decision == "remove" ? 1 : 2),
                }),
            ReviewDecisionHandler = (_, _, _, _, value, expectedRevision, _) =>
            {
                observedExpectedRevision = expectedRevision;
                decision = value;
                revision++;
                return Task.FromResult(new WorkerReviewDecisionMutation(7, revision, false, value));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        (long RunId, long Revision)? publishedRevision = null;
        viewModel.ReviewRevisionChanged += (runId, appliedRevision) =>
            publishedRevision = (runId, appliedRevision);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(12, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        await viewModel.RemoveMemberCommand.ExecuteAsync(viewModel.Members[0]);

        Assert.AreEqual(0, observedExpectedRevision);
        Assert.AreEqual("Remove", viewModel.Members[0].Decision);
        Assert.AreEqual(1, viewModel.ReviewPlan.Plan.Revision);
        Assert.AreEqual("Review: 0 keep, 1 remove, 1 undecided · 1 KB planned", viewModel.ReviewPlanSummaryText);
        Assert.AreEqual(
            "Set review: 0 keep, 1 remove, 1 undecided · 1 physical copy remains",
            viewModel.SelectedReviewSummaryText);
        StringAssert.Contains(viewModel.SelectedSetStatusAnnouncement, "Review decision saved: Remove");
        Assert.AreEqual((12L, 1L), publishedRevision);
        Assert.IsTrue(viewModel.RemoveMemberCommand.CanExecute(viewModel.Members[0]));
        Assert.IsTrue(viewModel.CachedMemberPageCount <= DuplicateFilesViewModel.CacheCapacity);
    }

    [TestMethod]
    public async Task ExternalFolderRevisionRefreshesVisibleFileReviewState()
    {
        var revision = 0L;
        var planQueries = 0;
        var memberQueries = 0;
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([Group(1, query.RunId, "item.bin")], 1, null, null)),
            ReviewPlanHandler = (runId, _) =>
            {
                planQueries++;
                return Task.FromResult(new WorkerReviewPlanView(
                    new WorkerReviewPlan(revision == 0 ? null : 7, runId, revision == 0 ? "notCreated" : "active", revision, null, null),
                    new WorkerReviewPlanSummary(0, 0, 0, 2, "0", 2)
                    {
                        FolderKeepCount = revision,
                    }));
            },
            MemberPageHandler = (query, _) =>
            {
                memberQueries++;
                return Task.FromResult(new WorkerDuplicateFileMemberPage(
                    [Member(1, query.GroupId, @"C:\Data\item.bin"), Member(2, query.GroupId, @"D:\Backup\item.bin")],
                    2,
                    null,
                    null)
                {
                    ReviewPlanId = revision == 0 ? null : 7,
                    ReviewRevision = revision,
                    ReviewSummary = new WorkerReviewGroupSummary(query.GroupId, 0, 0, 2, 2),
                });
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(30, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        revision = 1;
        await viewModel.RefreshReviewRevisionAsync(30, revision);

        Assert.AreEqual(1, viewModel.ReviewPlan.Plan.Revision);
        Assert.AreEqual(2, planQueries);
        Assert.AreEqual(2, memberQueries);
        Assert.IsTrue(viewModel.CachedMemberPageCount <= DuplicateFilesViewModel.CacheCapacity);
    }

    [TestMethod]
    public async Task LateReviewMutationCannotReplaceANewerRun()
    {
        var mutation = new TaskCompletionSource<WorkerReviewDecisionMutation>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([Group(query.RunId, query.RunId, $"run-{query.RunId}.bin")], 1, null, null)),
            MemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileMemberPage(
                    [Member(query.RunId, query.GroupId, $@"C:\Data\run-{query.RunId}.bin")],
                    1,
                    null,
                    null)),
            ReviewDecisionHandler = (_, _, _, _, _, _, _) => mutation.Task,
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(20, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        var pending = viewModel.KeepMemberCommand.ExecuteAsync(viewModel.Members[0]);

        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(21, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        mutation.SetResult(new WorkerReviewDecisionMutation(9, 1, false, "keep"));
        await pending;

        Assert.AreEqual(21, viewModel.Run!.Id);
        Assert.AreEqual("run-21.bin", viewModel.Groups[0].RepresentativeName);
        Assert.AreEqual(0, viewModel.ReviewPlan.Plan.Revision);
        Assert.AreEqual("Undecided", viewModel.Members[0].Decision);
    }

    [TestMethod]
    public async Task UnsafeReviewFailureIsActionableAndLeavesDisplayedDecisionUnchanged()
    {
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([Group(1, query.RunId, "item.bin")], 1, null, null)),
            MemberPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileMemberPage(
                    [Member(1, query.GroupId, @"C:\Data\item.bin")],
                    1,
                    null,
                    null)),
            ReviewDecisionHandler = (_, _, _, _, _, _, _) =>
                Task.FromException<WorkerReviewDecisionMutation>(
                    new InvalidOperationException(
                        "unsafe_physical_survivor: one independently accessible copy must remain")),
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(22, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        await viewModel.RemoveMemberCommand.ExecuteAsync(viewModel.Members[0]);

        Assert.AreEqual("Undecided", viewModel.Members[0].Decision);
        Assert.IsTrue(viewModel.HasDetailError);
        StringAssert.Contains(viewModel.DetailErrorMessage, "one independently accessible copy must remain");
        Assert.AreEqual(1, viewModel.SelectedSetErrorAnnouncementVersion);
        Assert.IsFalse(viewModel.IsReviewUpdating);
    }

    [TestMethod]
    public async Task VisiblePageValidationBindsOnlyCurrentPageAndInvalidatesWorkingChoices()
    {
        ReviewLiveValidationRequest? captured = null;
        var memberQueries = 0;
        var members = Enumerable.Range(1, DuplicateFilesViewModel.PageSize)
            .Select(id => Member(id, 1, $@"C:\Data\copy-{id:D3}.bin") with
            {
                Decision = id == 1 ? "keep" : id == 2 ? "remove" : "undecided",
            })
            .ToArray();
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([Group(1, query.RunId, "item.bin")], 1, null, null)),
            MemberPageHandler = (_, _) =>
            {
                memberQueries++;
                return Task.FromResult(new WorkerDuplicateFileMemberPage(members, 350, null, null)
                {
                    ReviewPlanId = 7,
                    ReviewRevision = 2,
                    ReviewSummary = new WorkerReviewGroupSummary(1, 1, 1, 198, 199),
                });
            },
            ReviewPlanHandler = (runId, _) => Task.FromResult(new WorkerReviewPlanView(
                new WorkerReviewPlan(7, runId, "active", 2, null, null),
                captured is null
                    ? new WorkerReviewPlanSummary(1, 1, 1, 198, "1024", 199)
                    : new WorkerReviewPlanSummary(0, 0, 0, 200, "0", 200))),
            ReviewLiveValidationHandler = (request, _) =>
            {
                captured = request;
                return Task.FromResult(new WorkerReviewLiveValidationResult(
                    9,
                    request.RunId,
                    request.GroupId,
                    request.ExpectedReviewRevision,
                    request.Scope,
                    false,
                    new WorkerReviewLiveValidationSummary(200, 198, 1, 1, 0, 2),
                    request.FileIds.Select(id => id switch
                    {
                        1 => new WorkerReviewLiveValidationItem(id, "missing", "path_missing", true, "keep", "2026-08-24T00:00:00Z"),
                        2 => new WorkerReviewLiveValidationItem(id, "changed", "size_changed", true, "remove", "2026-08-24T00:00:00Z"),
                        _ => new WorkerReviewLiveValidationItem(id, "present", "matched_snapshot", false, null, "2026-08-24T00:00:00Z"),
                    }).ToArray()));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        var validationAvailabilityNotifications = new List<bool>();
        viewModel.PropertyChanged += (_, args) =>
        {
            if (args.PropertyName == nameof(DuplicateFilesViewModel.CanValidateVisiblePage))
            {
                validationAvailabilityNotifications.Add(viewModel.CanValidateVisiblePage);
            }
        };
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(40, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        var memberQueriesBeforeValidation = memberQueries;
        Assert.IsTrue(validationAvailabilityNotifications.Last(), "The binding was not notified when the member page became validatable.");
        var removeCommandNotifications = 0;
        viewModel.RemoveMemberCommand.CanExecuteChanged += (_, _) => removeCommandNotifications++;

        await viewModel.ValidateVisiblePageCommand.ExecuteAsync(null);

        Assert.IsNotNull(captured);
        Assert.AreEqual("visible_page", captured.Scope);
        Assert.AreEqual(DuplicateFilesViewModel.PageSize, captured.FileIds.Count);
        CollectionAssert.AreEqual(Enumerable.Range(1, 200).Select(value => (long)value).ToArray(), captured.FileIds.ToArray());
        Assert.AreEqual(memberQueriesBeforeValidation, memberQueries, "Validation followed a page cursor or rebound the result set.");
        Assert.AreEqual("Undecided", viewModel.Members[0].Decision);
        Assert.AreEqual("Missing; prior Keep decision invalidated", viewModel.Members[0].LiveState);
        Assert.AreEqual("Changed since scan; prior Remove decision invalidated", viewModel.Members[1].LiveState);
        Assert.IsFalse(viewModel.Members[0].CanRecordCurrentDecision);
        Assert.IsTrue(viewModel.Members[0].CanClearDecision);
        Assert.IsTrue(removeCommandNotifications > 0, "Review commands were not re-queried after validation completed.");
        StringAssert.Contains(viewModel.LiveValidationStatusMessage, "2 review choices invalidated");
        StringAssert.Contains(viewModel.LiveValidationStatusMessage, "Original scan history was not changed");
        StringAssert.Contains(viewModel.LiveValidationErrorMessage, "Restore or reconnect");
        Assert.AreEqual("Review: 0 keep, 0 remove, 200 undecided · 0 B planned", viewModel.ReviewPlanSummaryText);
        Assert.IsFalse(viewModel.IsLiveValidationRunning);
    }

    [TestMethod]
    public async Task ValidationCancellationAndLateResponseCannotReplaceNewerContext()
    {
        var late = new TaskCompletionSource<WorkerReviewLiveValidationResult>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        CancellationToken observedToken = default;
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([Group(query.RunId, query.RunId, $"run-{query.RunId}.bin")], 1, null, null)),
            MemberPageHandler = (query, _) => Task.FromResult(new WorkerDuplicateFileMemberPage(
                [Member(query.RunId, query.GroupId, $@"C:\Data\run-{query.RunId}.bin")], 1, null, null)),
            ReviewLiveValidationHandler = (request, token) =>
            {
                observedToken = token;
                return late.Task;
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(41, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        var pending = viewModel.ValidateVisiblePageCommand.ExecuteAsync(null);
        Assert.IsTrue(viewModel.IsLiveValidationRunning);

        viewModel.CancelLiveValidationCommand.Execute(null);
        Assert.IsTrue(observedToken.IsCancellationRequested);
        Assert.IsFalse(viewModel.IsLiveValidationRunning);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(42, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        late.SetResult(new WorkerReviewLiveValidationResult(
            10, 41, 41, 0, "selection", false,
            new WorkerReviewLiveValidationSummary(1, 0, 0, 1, 0, 1),
            [new WorkerReviewLiveValidationItem(41, "missing", "path_missing", true, "keep", "2026-08-24T00:00:00Z")]));
        await pending;

        Assert.AreEqual(42, viewModel.Run!.Id);
        Assert.AreEqual("run-42.bin", viewModel.Groups.Single().RepresentativeName);
        Assert.AreEqual("Not validated in this working view", viewModel.Members.Single().LiveState);
        Assert.IsFalse(viewModel.HasLiveValidationStatus);
        Assert.IsFalse(viewModel.HasLiveValidationError);
    }

    [TestMethod]
    public async Task DirtyRootReconstructionReconcilesOneBoundedBatchAndPreservesMemberCursor()
    {
        const string rootPath = @"C:\Data";
        var dirty = new WorkerReviewLiveRootState(
            50, rootPath, "dirty", 3, "watcher_overflow", "2026-08-24T00:00:00Z",
            null, 0, "2026-08-24T00:00:00Z", true);
        ReviewLiveRootReconciliationRequest? captured = null;
        var reconciled = false;
        var memberCursors = new List<string?>();
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([Group(1, query.RunId, "item.bin")], 1, null, null)),
            MemberPageHandler = (query, _) =>
            {
                memberCursors.Add(query.Cursor);
                var id = query.Cursor is null ? 1 : 2;
                var member = Member(id, query.GroupId, $@"C:\Data\copy-{id}.bin") with
                {
                    ValidationState = reconciled && id == 2 ? "missing" : null,
                    ValidationReasonCode = reconciled && id == 2 ? "path_missing" : null,
                    ValidationObservedAt = reconciled && id == 2 ? "2026-08-24T00:01:00Z" : null,
                };
                return Task.FromResult(new WorkerDuplicateFileMemberPage(
                    [member],
                    2,
                    query.Cursor is null ? "next-page" : null,
                    query.Cursor is null ? null : "previous-page"));
            },
            DirtyReviewRootsHandler = (runId, _) => Task.FromResult(
                new WorkerReviewLiveRootPage(runId, [dirty with { RunId = runId }], 1, false)),
            DirtyRootReconciliationHandler = (request, _) =>
            {
                captured = request;
                reconciled = true;
                var clean = dirty with
                {
                    RunId = request.RunId,
                    State = "clean",
                    ReconciledItemCount = 2,
                    UpdatedAt = "2026-08-24T00:01:00Z",
                    ReconciliationRequired = false,
                };
                return Task.FromResult(new WorkerReviewLiveRootReconciliationResult(
                    7,
                    request.RunId,
                    request.RootPath,
                    request.ExpectedDirtyRevision,
                    request.ExpectedReviewRevision,
                    false,
                    new WorkerReviewLiveValidationSummary(2, 1, 0, 1, 0, 0),
                    [new WorkerReviewLiveValidationItem(
                        2, "missing", "path_missing", false, null, "2026-08-24T00:01:00Z")],
                    clean,
                    false));
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(50, 3, "completed", "finalizing", DateTimeOffset.UtcNow));

        Assert.IsTrue(viewModel.HasDirtyRoots);
        StringAssert.Contains(viewModel.DirtyRootWarningMessage, "dirty and reconciliation is required");
        StringAssert.Contains(viewModel.DirtyRootWarningMessage, "at most 200");
        var dirtyRootDispatcherUpdates = 0;
        viewModel.PropertyChanged += (_, args) =>
        {
            if (args.PropertyName == nameof(DuplicateFilesViewModel.DirtyRoots))
            {
                dirtyRootDispatcherUpdates++;
            }
        };
        await viewModel.NextMemberPageCommand.ExecuteAsync(null);
        Assert.AreEqual(2, viewModel.Members.Single().Id);
        var queryCountBefore = memberCursors.Count;

        await viewModel.ReconcileDirtyRootCommand.ExecuteAsync(null);

        Assert.IsNotNull(captured);
        Assert.AreEqual(DuplicateFilesViewModel.PageSize, captured.PageSize);
        Assert.AreEqual(rootPath, captured.RootPath);
        Assert.AreEqual(3, captured.ExpectedDirtyRevision);
        var reconciliationQueries = memberCursors.Skip(queryCountBefore).ToArray();
        Assert.AreEqual(1, reconciliationQueries.Count(cursor => cursor == "next-page"),
            "The explicit reconciliation did not refresh exactly the committed member page.");
        Assert.IsTrue(reconciliationQueries.Length <= 2,
            "Reconciliation escaped the accepted current-plus-neighbor bounded member cache.");
        Assert.AreEqual(2, viewModel.Members.Single().Id);
        Assert.AreEqual("Missing", viewModel.Members.Single().LiveState);
        Assert.IsFalse(viewModel.HasDirtyRoots);
        Assert.AreEqual(1, dirtyRootDispatcherUpdates,
            "One explicit reconciliation response should produce one root-state binding update, not watcher-event fan-out.");
        StringAssert.Contains(viewModel.DirtyRootStatusMessage, "dirty marker is cleared");
        StringAssert.Contains(viewModel.DirtyRootStatusMessage, "Original scan history was not changed");
        Assert.IsFalse(viewModel.IsDirtyRootReconciliationRunning);
    }

    [TestMethod]
    public async Task DirtyRootCancellationAndLateResponseCannotReplaceNewerRunContext()
    {
        var late = new TaskCompletionSource<WorkerReviewLiveRootReconciliationResult>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        CancellationToken observedToken = default;
        var dirty = new WorkerReviewLiveRootState(
            51, @"C:\Data", "dirty", 1, "watcher_overflow", "2026-08-24T00:00:00Z",
            null, 0, "2026-08-24T00:00:00Z", true);
        var client = new TestWorkerClient
        {
            GroupPageHandler = (query, _) => Task.FromResult(
                new WorkerDuplicateFileGroupPage([Group(query.RunId, query.RunId, $"run-{query.RunId}.bin")], 1, null, null)),
            MemberPageHandler = (query, _) => Task.FromResult(new WorkerDuplicateFileMemberPage(
                [Member(query.RunId, query.GroupId, $@"C:\Data\run-{query.RunId}.bin")], 1, null, null)),
            DirtyReviewRootsHandler = (runId, _) => Task.FromResult(
                runId == 51
                    ? new WorkerReviewLiveRootPage(runId, [dirty], 1, false)
                    : new WorkerReviewLiveRootPage(runId, [], 0, false)),
            DirtyRootReconciliationHandler = (request, token) =>
            {
                observedToken = token;
                return late.Task;
            },
        };
        using var viewModel = new DuplicateFilesViewModel(client, new TestClipboard(), new TestExplorer());
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(51, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        var pending = viewModel.ReconcileDirtyRootCommand.ExecuteAsync(null);
        Assert.IsTrue(viewModel.IsDirtyRootReconciliationRunning);

        viewModel.CancelDirtyRootReconciliationCommand.Execute(null);
        Assert.IsTrue(observedToken.IsCancellationRequested);
        Assert.IsFalse(viewModel.IsDirtyRootReconciliationRunning);
        Assert.IsTrue(viewModel.HasDirtyRoots);
        await viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(52, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        late.SetResult(new WorkerReviewLiveRootReconciliationResult(
            8, 51, dirty.RootPath, 1, 0, false,
            new WorkerReviewLiveValidationSummary(1, 1, 0, 0, 0, 0),
            [],
            dirty with { State = "clean", ReconciliationRequired = false },
            false));
        await pending;

        Assert.AreEqual(52, viewModel.Run!.Id);
        Assert.IsFalse(viewModel.HasDirtyRoots);
        Assert.IsFalse(viewModel.HasDirtyRootStatus);
        Assert.IsFalse(viewModel.HasDirtyRootError);
        Assert.AreEqual("run-52.bin", viewModel.Groups.Single().RepresentativeName);
    }

    private static WorkerDuplicateFileGroup Group(long id, long runId, string name) =>
        new(id, runId, "1024", 2, "1024", name, ".bin")
        {
            DistinctSelectedRootCount = 2,
            DistinctDriveCount = 2,
        };

    private static WorkerDuplicateFileMember Member(long id, long groupId, string path) =>
        new(id, groupId, path, Path.GetFileName(path), Path.GetDirectoryName(path)!, "1024", "1700000000000000000")
        {
            RootPath = Path.GetDirectoryName(path)!,
            RelativePath = Path.GetFileName(path),
            DriveLetter = Path.GetPathRoot(path)!.TrimEnd(Path.DirectorySeparatorChar),
        };
}
