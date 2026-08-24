using System.Reflection;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class RunHistoryViewModelTests
{
    [TestMethod]
    public async Task WarningDrilldownPagesBoundsCacheAndRestoresFocus()
    {
        var worker = new TestWorkerClient();
        worker.Runs.Add(TestWorkerClient.CreateRun(7, 3, "completed", "finalizing", DateTimeOffset.UtcNow) with
        {
            WarningCount = 100_000,
        });
        var queries = new List<RunWarningQuery>();
        worker.RunWarningsHandler = (query, _) =>
        {
            queries.Add(query);
            Assert.AreEqual(7, query.RunId);
            Assert.AreEqual(RunHistoryViewModel.WarningPageSize, query.PageSize);
            var index = query.Cursor is null ? 0 : int.Parse(query.Cursor[7..]);
            var firstId = index * RunHistoryViewModel.WarningPageSize + 1;
            return Task.FromResult(new WorkerRunWarningPage(
                Enumerable.Range(firstId, RunHistoryViewModel.WarningPageSize)
                    .Select(id => new WorkerRunWarningAggregate(id, query.RunId, "discovering", "scan",
                        $"warning-{id}", "warning", $"Warning {id}", 1, [$"Example {id}"]))
                    .ToArray(),
                100_000, 100_000, 100_000, $"cursor-{index + 1}", false));
        };
        using var viewModel = new RunHistoryViewModel(worker);
        await viewModel.LoadAsync(3);

        await viewModel.OpenWarningsCommand.ExecuteAsync(null);
        for (var index = 0; index < 6; index++)
        {
            await viewModel.NextWarningPageCommand.ExecuteAsync(null);
        }

        Assert.IsTrue(viewModel.IsWarningDrilldownOpen);
        Assert.AreEqual(RunHistoryViewModel.WarningPageSize, viewModel.Warnings.Count);
        Assert.AreEqual("warning-151", viewModel.Warnings[0].Code);
        Assert.AreEqual("warnings", viewModel.FocusTarget);
        var cache = (System.Collections.IDictionary)typeof(RunHistoryViewModel)
            .GetField("_warningCache", BindingFlags.Instance | BindingFlags.NonPublic)!
            .GetValue(viewModel)!;
        Assert.AreEqual(RunHistoryViewModel.WarningCachePageLimit, cache.Count);
        Assert.IsTrue(queries.All(query => query.SortField == RunWarningSortField.OccurrenceCount));
        Assert.IsTrue(queries.All(query => query.SortDirection == WorkerSortDirection.Descending));
        StringAssert.Contains(viewModel.WarningStatusMessage, "25 of 100,000");

        await viewModel.ApplyWarningSortAsync(RunWarningSortField.Phase, WorkerSortDirection.Ascending);
        Assert.AreEqual(RunWarningSortField.Phase, queries[^1].SortField);
        Assert.AreEqual(WorkerSortDirection.Ascending, queries[^1].SortDirection);
        Assert.IsNull(queries[^1].Cursor);
        Assert.AreEqual(RunHistoryViewModel.WarningPageSize, viewModel.Warnings.Count);
        Assert.AreEqual(1, cache.Count);

        viewModel.CloseWarningsCommand.Execute(null);
        Assert.IsFalse(viewModel.IsWarningDrilldownOpen);
        Assert.AreEqual("history", viewModel.FocusTarget);
    }

    [TestMethod]
    public async Task WarningDrilldownCancelsAndRejectsLateOldRunResponse()
    {
        var worker = new TestWorkerClient();
        worker.Runs.Add(TestWorkerClient.CreateRun(8, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        worker.Runs.Add(TestWorkerClient.CreateRun(7, 3, "completed", "finalizing", DateTimeOffset.UtcNow.AddMinutes(-1)));
        var completion = new TaskCompletionSource<WorkerRunWarningPage>(TaskCreationOptions.RunContinuationsAsynchronously);
        CancellationToken observedToken = default;
        worker.RunWarningsHandler = (_, token) =>
        {
            observedToken = token;
            return completion.Task;
        };
        using var viewModel = new RunHistoryViewModel(worker);
        await viewModel.LoadAsync(3);
        var load = viewModel.OpenWarningsCommand.ExecuteAsync(null);

        viewModel.SelectedRun = viewModel.Runs.Single(run => run.Id == 7);
        completion.SetResult(new WorkerRunWarningPage(
            [new WorkerRunWarningAggregate(1, 8, "hashing", "scan", "stale", "warning",
                "Stale warning", 1, ["stale example"])],
            1, 1, 1, null, false));
        await load;

        Assert.IsTrue(observedToken.IsCancellationRequested);
        Assert.IsFalse(viewModel.IsWarningDrilldownOpen);
        Assert.AreEqual(0, viewModel.Warnings.Count);
        Assert.IsNull(viewModel.WarningStatusMessage);
    }

    [TestMethod]
    public async Task WarningDrilldownRejectsUnaccountedOrExecutorEnabledPage()
    {
        var worker = new TestWorkerClient();
        worker.Runs.Add(TestWorkerClient.CreateRun(7, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        worker.RunWarningsHandler = (_, _) => Task.FromResult(
            new WorkerRunWarningPage([], 0, 1, 0, null, true));
        using var viewModel = new RunHistoryViewModel(worker);
        await viewModel.LoadAsync(3);

        await viewModel.OpenWarningsCommand.ExecuteAsync(null);

        Assert.IsTrue(viewModel.IsWarningDrilldownOpen);
        Assert.IsTrue(viewModel.HasWarningError);
        StringAssert.Contains(viewModel.WarningErrorMessage, "unsafe");
    }
}
