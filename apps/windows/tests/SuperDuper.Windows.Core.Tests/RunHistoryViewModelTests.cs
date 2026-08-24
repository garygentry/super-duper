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
        worker.Runs.Add(TestWorkerClient.CreateRun(7, 3, "completed", "finalizing", DateTimeOffset.UtcNow));
        var page = 0;
        worker.RunWarningsHandler = (runId, pageSize, cursor, _) =>
        {
            Assert.AreEqual(7, runId);
            Assert.AreEqual(RunHistoryViewModel.WarningPageSize, pageSize);
            var index = page++;
            return Task.FromResult(new WorkerRunWarningPage(
                [new WorkerRunWarningAggregate(index + 1, runId, "discovering", "scan",
                    $"warning-{index}", "warning", $"Warning {index}", 1, [$"Example {index}"])],
                7, 1, 1, index < 6 ? $"cursor-{index + 1}" : null, false));
        };
        using var viewModel = new RunHistoryViewModel(worker);
        await viewModel.LoadAsync(3);

        await viewModel.OpenWarningsCommand.ExecuteAsync(null);
        for (var index = 0; index < 6; index++)
        {
            await viewModel.NextWarningPageCommand.ExecuteAsync(null);
        }

        Assert.IsTrue(viewModel.IsWarningDrilldownOpen);
        Assert.AreEqual("warning-6", viewModel.Warnings.Single().Code);
        Assert.AreEqual("warnings", viewModel.FocusTarget);
        var cache = (System.Collections.IDictionary)typeof(RunHistoryViewModel)
            .GetField("_warningCache", BindingFlags.Instance | BindingFlags.NonPublic)!
            .GetValue(viewModel)!;
        Assert.AreEqual(RunHistoryViewModel.WarningCachePageLimit, cache.Count);

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
        worker.RunWarningsHandler = (_, _, _, token) =>
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
        worker.RunWarningsHandler = (_, _, _, _) => Task.FromResult(
            new WorkerRunWarningPage([], 0, 1, 0, null, true));
        using var viewModel = new RunHistoryViewModel(worker);
        await viewModel.LoadAsync(3);

        await viewModel.OpenWarningsCommand.ExecuteAsync(null);

        Assert.IsTrue(viewModel.IsWarningDrilldownOpen);
        Assert.IsTrue(viewModel.HasWarningError);
        StringAssert.Contains(viewModel.WarningErrorMessage, "unsafe or incomplete");
    }
}
