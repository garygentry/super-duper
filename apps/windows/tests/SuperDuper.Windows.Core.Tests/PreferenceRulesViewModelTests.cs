using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class PreferenceRulesViewModelTests
{
    [TestMethod]
    public async Task Saves_reorders_and_pages_read_only_preview_with_a_five_page_cache()
    {
        var worker = new TestWorkerClient();
        var run = CompletedRun(7, [@"C:\Photos", @"D:\Backup"]);
        var reviewRevision = 0L;
        var requestedCursors = new List<string?>();
        worker.PreferencePreviewHandler = (query, _) =>
        {
            requestedCursors.Add(query.Cursor);
            var pageNumber = query.Cursor is null ? 0 : int.Parse(query.Cursor[1..]);
            var next = pageNumber < 5 ? $"p{pageNumber + 1}" : null;
            return Task.FromResult(new WorkerPreferencePreviewPage(
                [new WorkerPreferencePreviewGroup(
                    pageNumber + 1,
                    "applicable",
                    0,
                    @"C:\Photos",
                    1,
                    1,
                    1,
                    1,
                    "100",
                    0,
                    0,
                    "preferred_root_rank",
                    null,
                    null)],
                6,
                next,
                query.RuleId,
                query.RuleRevision,
                null,
                query.ReviewRevision,
                Summary(6)));
        };
        using var viewModel = new PreferenceRulesViewModel(
            worker,
            () => new DuplicateFileGroupFilter(string.Empty, "0"),
            () => 41,
            () => reviewRevision);

        await viewModel.ShowRunAsync(run);
        Assert.AreEqual(2, viewModel.OrderedRoots.Count);
        viewModel.RuleName = "Primary libraries";
        await viewModel.SaveCommand.ExecuteAsync(null);
        Assert.AreEqual(1, worker.PreferenceRules.Count);

        viewModel.SelectedRoot = @"D:\Backup";
        viewModel.MoveRootUpCommand.Execute(null);
        Assert.AreEqual(@"D:\Backup", viewModel.OrderedRoots[0]);
        Assert.IsFalse(viewModel.PreviewCommand.CanExecute(null), "edited ordering must be saved before preview");
        await viewModel.SaveCommand.ExecuteAsync(null);
        Assert.AreEqual(2, worker.PreferenceRules[0].Revision);

        await viewModel.PreviewCommand.ExecuteAsync(null);
        for (var page = 0; page < 5; page++)
        {
            await viewModel.NextPageCommand.ExecuteAsync(null);
        }
        Assert.AreEqual(6, requestedCursors.Count);
        Assert.AreEqual(5, viewModel.CachedPageCount);
        Assert.AreEqual(6, viewModel.PreviewGroups[0].GroupId);
        StringAssert.Contains(viewModel.StatusMessage, "Nothing was applied or deleted");
        StringAssert.Contains(viewModel.SummaryText, "6 applicable sets");

        reviewRevision = 1;
        viewModel.InvalidateReviewRevision(reviewRevision);
        Assert.AreEqual(0, viewModel.PreviewGroups.Count);
        StringAssert.Contains(viewModel.StatusMessage, "Manual review decisions changed");
    }

    [TestMethod]
    public async Task Cancellation_and_late_preview_response_cannot_replace_a_new_generation()
    {
        var worker = new TestWorkerClient();
        var run = CompletedRun(9, [@"C:\Preferred", @"D:\Other"]);
        var pending = new TaskCompletionSource<WorkerPreferencePreviewPage>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        worker.PreferencePreviewHandler = (_, token) =>
        {
            token.Register(() => pending.TrySetCanceled(token));
            return pending.Task;
        };
        using var viewModel = new PreferenceRulesViewModel(
            worker,
            () => new DuplicateFileGroupFilter(string.Empty, "0"),
            () => 1,
            () => 0);

        await viewModel.ShowRunAsync(run);
        viewModel.RuleName = "Preferred";
        await viewModel.SaveCommand.ExecuteAsync(null);
        var preview = viewModel.PreviewCommand.ExecuteAsync(null);
        viewModel.InvalidateFilter();
        await preview;

        Assert.AreEqual(0, viewModel.PreviewGroups.Count);
        Assert.IsNull(viewModel.ErrorMessage);
        StringAssert.Contains(viewModel.StatusMessage, "filter changed");
    }

    private static WorkerRun CompletedRun(long id, IReadOnlyList<string> roots) => new(
        id,
        1,
        new WorkerRunParameters(
            roots,
            [],
            500,
            CloudPolicyNames.ExcludeRegisteredRoots,
            [],
            [],
            CloudDetectionStatusNames.Complete),
        "completed",
        "finalizing",
        DateTimeOffset.UtcNow,
        DateTimeOffset.UtcNow,
        DateTimeOffset.UtcNow,
        4,
        "400",
        4,
        2,
        0,
        "200",
        0,
        0,
        null,
        "test");

    private static WorkerPreferencePreviewSummary Summary(long affected) => new(
        affected,
        affected * 2,
        affected * 2,
        (affected * 200).ToString(),
        affected,
        0,
        affected,
        affected,
        affected,
        (affected * 100).ToString(),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0);
}
