using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class FileQueryEditorTests
{
    [TestMethod]
    public async Task DelayedApplyKeepsAcceptedRowsTotalsChipsAndDecisionsUntilReplacement()
    {
        var pending = new TaskCompletionSource<WorkerDuplicateFileGroupPage>();
        var client = new TestWorkerClient
        {
            GroupPageHandler = (q, _) => q.Filter.Search == "new" ? pending.Task : Task.FromResult(Page(q.RunId, 1)),
        };
        using var model = Model(client);
        await model.ShowRunAsync(Run(1));
        var groups = model.Groups;
        var selected = model.SelectedGroup;
        var plan = model.ReviewPlan;
        var chips = model.AppliedFilters;
        model.SearchText = "new";
        StringAssert.Contains(model.QueryEditorStatus, "Unapplied");
        var apply = model.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.IsTrue(model.IsLoading);
        Assert.AreSame(groups, model.Groups);
        Assert.AreSame(selected, model.SelectedGroup);
        Assert.AreSame(plan, model.ReviewPlan);
        Assert.AreSame(chips, model.AppliedFilters);
        Assert.AreEqual("1", model.FilteredSetsText);
        Assert.AreEqual("2", model.FilteredCopiesText);
        Assert.AreEqual("1024 bytes", model.FilteredSavingsExactText);
        model.SearchText = "later draft";
        pending.SetResult(Page(1, 2));
        await apply;
        Assert.AreEqual(2, model.Groups.Single().Id);
        Assert.AreEqual("Path contains: new", model.AppliedFilters.Single().Text);
        Assert.AreEqual("later draft", model.SearchText);
        Assert.AreSame(plan, model.ReviewPlan);
        StringAssert.Contains(model.QueryEditorStatus, "Unapplied");
    }

    [TestMethod]
    public async Task InvalidAndFailedQueriesPreserveAcceptedFilterAndUnknownIsNotZero()
    {
        var calls = 0;
        var client = new TestWorkerClient { GroupPageHandler = (q, _) =>
        {
            calls++;
            return q.Filter.Search == "fail" ? Task.FromException<WorkerDuplicateFileGroupPage>(new IOException("fixture query failed")) : Task.FromResult(Page(q.RunId, 1));
        }};
        using var model = Model(client);
        Assert.AreEqual("—", model.FilteredSetsText);
        await model.ShowRunAsync(Run(1));
        var groups = model.Groups;
        model.MinimumSizeText = "0.1";
        await model.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.AreEqual(1, calls);
        Assert.AreSame(groups, model.Groups);
        model.MinimumSizeText = "";
        model.SearchText = "fail";
        await model.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.AreSame(groups, model.Groups);
        Assert.AreEqual(0, model.AppliedFilters.Count);
        Assert.AreEqual("1", model.FilteredSetsText);
        StringAssert.Contains(model.QueryEditorStatus, "Previous applied results");
        await model.ShowRunAsync(Run(2) with { Status = "cancelled" });
        Assert.AreEqual("—", model.FilteredSetsText);
        Assert.AreEqual(0, model.Groups.Count);
    }

    [TestMethod]
    public async Task DraftDoesNotLeakIntoPagingSortingFacetsOrRuleScopeAndCeilingsRemain()
    {
        var queries = new List<DuplicateFileGroupQuery>();
        var roots = new List<DuplicateFileSelectedRootFacetQuery>();
        var drives = new List<DuplicateFileDriveFacetQuery>();
        var client = new TestWorkerClient
        {
            GroupPageHandler = (q, _) =>
            {
                queries.Add(q);
                var index = q.Cursor is null ? 0 : int.Parse(q.Cursor);
                return Task.FromResult(Page(q.RunId, index + 1) with { Total = 20,
                    NextCursor = index < 19 ? (index + 1).ToString() : null,
                    PreviousCursor = index > 0 ? (index - 1).ToString() : null });
            },
            RootFacetPageHandler = (q, _) => { roots.Add(q); return Task.FromResult(new WorkerDuplicateFileSelectedRootFacetPage([], 0, null, null)); },
            DriveFacetPageHandler = (q, _) => { drives.Add(q); return Task.FromResult(new WorkerDuplicateFileDriveFacetPage([], 0, null, null)); },
        };
        using var model = Model(client);
        await model.ShowRunAsync(Run(1));
        model.SearchText = "draft"; model.MinimumSizeText = "invalid"; model.ThreeOrMoreCopies = true;
        for (var i = 0; i < 8; i++) await model.NextPageCommand.ExecuteAsync(null);
        await model.SortRootFacetsByNameCommand.ExecuteAsync(null);
        await model.SortDriveFacetsByNameCommand.ExecuteAsync(null);
        await model.ApplySortAsync(DuplicateFileGroupSortField.CopyCount, WorkerSortDirection.Ascending);
        Assert.IsTrue(queries.All(q => q.Filter.Search == "" && q.Filter.MinimumSize == "0" && q.Filter.MinimumCopyCount == 2));
        Assert.IsTrue(roots.All(q => q.Filter.Search == "" && q.PageSize == 25));
        Assert.IsTrue(drives.All(q => q.Filter.Search == "" && q.PageSize == 25));
        Assert.IsTrue(queries.All(q => q.PageSize == 200 && q.RunId == 1));
        Assert.IsTrue(model.CachedGroupPageCount <= 5);
        Assert.IsTrue(model.Groups.Count <= 200);
        Assert.IsFalse(model.HasError);
        model.PreferenceRules.RuleName = "Fictional preference";
        model.PreferenceRules.NewRoot = @"C:\fictional";
        model.PreferenceRules.AddRootCommand.Execute(null);
        await model.PreferenceRules.SaveCommand.ExecuteAsync(null);
        PreferencePreviewQuery? preview = null;
        client.PreferencePreviewHandler = (q, _) =>
        {
            preview = q;
            return Task.FromException<WorkerPreferencePreviewPage>(new IOException("Stop after recording fixture scope"));
        };
        await model.PreferenceRules.PreviewCommand.ExecuteAsync(null);
        Assert.IsNotNull(preview);
        Assert.AreEqual(new DuplicateFileGroupFilter("", "0"), preview.Scope.Filter);
    }

    [TestMethod]
    public async Task FailedSortRestoresCursorSortAndCancelledFacetsDoNotRemainLoading()
    {
        var delayedFacet = new TaskCompletionSource<WorkerDuplicateFileSelectedRootFacetPage>();
        var queries = new List<DuplicateFileGroupQuery>();
        var client = new TestWorkerClient
        {
            GroupPageHandler = (q, _) =>
            {
                queries.Add(q);
                return q.SortField == DuplicateFileGroupSortField.CopyCount
                    ? Task.FromException<WorkerDuplicateFileGroupPage>(new IOException("fixture sort failed"))
                    : Task.FromResult(Page(q.RunId, 1) with { NextCursor = q.Cursor is null ? "next" : null });
            },
            RootFacetPageHandler = (_, _) => delayedFacet.Task,
        };
        using var model = Model(client);
        var initial = model.ShowRunAsync(Run(1));
        Assert.IsTrue(model.IsRootFacetLoading);
        await model.ApplySortAsync(DuplicateFileGroupSortField.CopyCount, WorkerSortDirection.Ascending);
        Assert.IsFalse(model.IsRootFacetLoading);
        Assert.AreEqual(DuplicateFileGroupSortField.RecoverableBytes, model.SortField);
        await model.NextPageCommand.ExecuteAsync(null);
        Assert.AreEqual("next", queries.Last().Cursor);
        Assert.AreEqual(DuplicateFileGroupSortField.RecoverableBytes, queries.Last().SortField);
        Assert.AreEqual(WorkerSortDirection.Descending, queries.Last().SortDirection);
        delayedFacet.SetResult(new([], 0, null, null));
        await initial;
        Assert.IsFalse(model.IsRootFacetLoading);
    }

    [TestMethod]
    public async Task ChipsRemoveOnlyAppliedSemanticAndClearRestoresDefaultQuery()
    {
        DuplicateFileGroupQuery? last = null;
        var client = new TestWorkerClient { GroupPageHandler = (q, _) => { last = q; return Task.FromResult(Page(q.RunId, 1)); } };
        using var model = Model(client);
        await model.ShowRunAsync(Run(1));
        model.SearchText = @"\\?\UNC\fictional\archive\copy.bin"; model.ExactPathMatch = true;
        model.MinimumSizeText = "1.5"; model.MinimumSizeUnit = "GiB"; model.OneGigabyteOrLarger = true;
        model.ThreeOrMoreCopies = true; model.AcrossDrives = true;
        model.WithoutExtension = true; model.AllMembersMustMatchExtension = true;
        model.SelectedRootFacet = new(selectedValue: @"C:\fictional");
        model.SelectedDriveFacet = new(selectedValue: "C:");
        await model.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.AreEqual(7, model.AppliedFilters.Count);
        Assert.AreEqual("1610612736", last!.Filter.MinimumSize);
        model.ExtensionText = "unapplied";
        foreach (var key in new[] { "path", "size", "copies", "across", "root", "drive", "extension" })
        {
            await model.RemoveFilterCommand.ExecuteAsync(model.AppliedFilters.Single(c => c.Key == key));
            Assert.IsFalse(model.AppliedFilters.Any(c => c.Key == key));
            if (key != "extension") Assert.AreEqual("", last.Filter.Extension);
        }
        Assert.AreEqual(new DuplicateFileGroupFilter("", "0"), last.Filter);
        model.SearchText = "draft"; model.MinimumSizeUnit = "TiB";
        await model.ApplySortAsync(DuplicateFileGroupSortField.CopyCount, WorkerSortDirection.Ascending);
        await model.ClearFiltersCommand.ExecuteAsync(null);
        Assert.AreEqual(new DuplicateFileGroupFilter("", "0"), last.Filter);
        Assert.AreEqual(DuplicateFileGroupSortField.RecoverableBytes, last.SortField);
        Assert.AreEqual(WorkerSortDirection.Descending, last.SortDirection);
        Assert.AreEqual("B", model.MinimumSizeUnit);
        Assert.AreEqual("", model.SearchText);
    }

    [TestMethod]
    public async Task SizeUnitsConvertExactlyRejectFractionalBytesOverflowAndKeepPresetMaximum()
    {
        DuplicateFileGroupQuery? last = null;
        var client = new TestWorkerClient { GroupPageHandler = (q, _) => { last = q; return Task.FromResult(Page(q.RunId, 1)); } };
        using var model = Model(client);
        await model.ShowRunAsync(Run(1));
        foreach (var (unit, input, bytes) in new[] { ("B", "9223372036854775807", "9223372036854775807"),
            ("KiB", "1.5", "1536"), ("MiB", "0.5", "524288"), ("GiB", "1", "1073741824"),
            ("TiB", "0.0000000000009094947017729282379150390625", "1") })
        {
            model.MinimumSizeUnit = unit; model.MinimumSizeText = input;
            await model.ApplyFiltersCommand.ExecuteAsync(null);
            Assert.IsFalse(model.HasError, model.ErrorMessage);
            Assert.AreEqual(bytes, last!.Filter.MinimumSize);
        }
        foreach (var (unit, input) in new[] { ("B", "9223372036854775808"), ("GiB", "8589934592"),
            ("B", "0.1"), ("KiB", "-1"), ("B", "1e3"), ("B", "1,024"), ("GiB", "0.0000000000000000000000000000000000000001") })
        {
            var accepted = last;
            model.MinimumSizeUnit = unit; model.MinimumSizeText = input;
            await model.ApplyFiltersCommand.ExecuteAsync(null);
            Assert.IsTrue(model.HasError, $"{input} {unit}");
            Assert.AreSame(accepted, last);
        }
        model.MinimumSizeUnit = "MiB"; model.MinimumSizeText = "1"; model.OneGigabyteOrLarger = true;
        await model.ApplyFiltersCommand.ExecuteAsync(null);
        Assert.AreEqual("1073741824", last!.Filter.MinimumSize);
    }

    [TestMethod]
    public async Task ClearAndRunChangeRejectLateReplacementAndItsChips()
    {
        var pending = new TaskCompletionSource<WorkerDuplicateFileGroupPage>();
        var client = new TestWorkerClient { GroupPageHandler = (q, _) => q.Filter.Search == "delay" ? pending.Task : Task.FromResult(Page(q.RunId, q.RunId)) };
        using var model = Model(client);
        await model.ShowRunAsync(Run(1));
        model.SearchText = "delay";
        var apply = model.ApplyFiltersCommand.ExecuteAsync(null);
        await model.ClearFiltersCommand.ExecuteAsync(null);
        await model.ShowRunAsync(Run(2));
        pending.SetResult(Page(1, 99));
        await apply;
        Assert.AreEqual(2, model.Run!.Id);
        Assert.AreEqual(2, model.Groups.Single().Id);
        Assert.AreEqual(0, model.AppliedFilters.Count);
        Assert.AreEqual("1", model.FilteredSetsText);
    }

    private static DuplicateFilesViewModel Model(TestWorkerClient client) => new(client, new TestClipboard(), new TestExplorer());
    private static WorkerRun Run(long id) => TestWorkerClient.CreateRun(id, 1, "completed", "finalizing", DateTimeOffset.UtcNow);
    private static WorkerDuplicateFileGroupPage Page(long run, long id) => new([new(id, run, "1024", 2, "1024", "fictional.bin", ".bin")], 1, null, null)
    { Summary = new(1, 2, "1024", "1024") };
}
