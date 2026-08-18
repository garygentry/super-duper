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

        oldResponse.SetResult(new WorkerDuplicateFileMemberPage(
            [Member(1, 1, @"C:\Data\stale-first.bin")],
            1,
            null,
            null));
        await Task.Yield();
        await Task.Yield();

        Assert.AreEqual(2, viewModel.SelectedGroup!.Id);
        Assert.AreEqual(@"C:\Data\second.bin", viewModel.Members.Single().Path);
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
                if (query.Filter.Search.Length == 0)
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
        var initialLoad = viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(12, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        await oldRequestObserved.Task;

        viewModel.SearchText = "new";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        oldResponse.SetResult(new WorkerDuplicateFileSelectedRootFacetPage(
            [new WorkerDuplicateFileSelectedRootFacet("stale-root", 99)],
            1,
            null,
            null));
        await initialLoad;

        Assert.AreEqual("new-root-0", viewModel.SelectedRootFacetOptions[1].Value);
        for (var page = 1; page < 9; page++)
        {
            await viewModel.NextRootFacetPageCommand.ExecuteAsync(null);
            Assert.IsTrue(viewModel.CachedRootFacetPageCount <= DuplicateFilesViewModel.CacheCapacity);
            Assert.AreEqual($"new-root-{page}", viewModel.SelectedRootFacetOptions[1].Value);
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
                if (query.Filter.Search.Length == 0)
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
        var initialLoad = viewModel.ShowRunAsync(
            TestWorkerClient.CreateRun(13, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        await oldRequestObserved.Task;

        viewModel.SearchText = "new";
        await viewModel.ApplyFiltersCommand.ExecuteAsync(null);
        oldResponse.SetResult(new WorkerDuplicateFileDriveFacetPage(
            [new WorkerDuplicateFileDriveFacet("stale-drive", 99)],
            1,
            null,
            null));
        await initialLoad;

        Assert.AreEqual("drive-0", viewModel.DriveFacetOptions[1].Value);
        for (var page = 1; page < 9; page++)
        {
            await viewModel.NextDriveFacetPageCommand.ExecuteAsync(null);
            Assert.IsTrue(viewModel.CachedDriveFacetPageCount <= DuplicateFilesViewModel.CacheCapacity);
            Assert.AreEqual($"drive-{page}", viewModel.DriveFacetOptions[1].Value);
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

        for (var page = 1; page < 9; page++)
        {
            await viewModel.NextPageCommand.ExecuteAsync(null);
            Assert.IsTrue(viewModel.CachedGroupPageCount <= DuplicateFilesViewModel.CacheCapacity);
            Assert.AreEqual($"page-{page}.bin", viewModel.Groups[0].RepresentativeName);
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
