using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class RunHistoryViewModelTests
{
    [TestMethod]
    public async Task HistoryUsesOneBoundedFiveHundredRunPageAndRetainsAcceptedPageOnFailure()
    {
        var worker = new TestWorkerClient();
        var session = worker.AddSession("Archive", @"C:\Data");
        for (var index = 1; index <= RunHistoryViewModel.HistoryPageSize + 1; index++)
        {
            worker.Runs.Add(TestWorkerClient.CreateRun(
                index,
                session.Id,
                "completed",
                "finalizing",
                DateTimeOffset.UtcNow.AddMinutes(-index)));
        }
        using var viewModel = new RunHistoryViewModel(worker);

        await viewModel.LoadAsync(session.Id);

        Assert.AreEqual(RunHistoryViewModel.HistoryPageSize, viewModel.Runs.Count);
        Assert.AreEqual(RunHistoryViewModel.HistoryPageSize + 1, viewModel.HistoryTotal);
        Assert.IsTrue(viewModel.CanLoadNextHistoryPage);
        Assert.IsFalse(viewModel.CanLoadPreviousHistoryPage);
        StringAssert.Contains(viewModel.HistoryPageStatus, "1–500 of 501");

        await viewModel.NextHistoryPageCommand.ExecuteAsync(null);

        Assert.AreEqual(1, viewModel.Runs.Count);
        Assert.AreEqual(1, viewModel.SelectedRun?.Id);
        Assert.IsTrue(viewModel.CanLoadPreviousHistoryPage);
        Assert.IsFalse(viewModel.CanLoadNextHistoryPage);
        StringAssert.Contains(viewModel.HistoryPageStatus, "501–501 of 501");
        Assert.AreEqual("history", viewModel.FocusTarget);

        worker.RunsHandler = (_, _) => Task.FromException<WorkerRunPage>(
            new InvalidOperationException("next history unavailable"));
        await viewModel.PreviousHistoryPageCommand.ExecuteAsync(null);

        Assert.IsTrue(viewModel.HasError);
        StringAssert.Contains(viewModel.ErrorMessage, "Retained the accepted page");
        Assert.AreEqual(1, viewModel.Runs.Count);
        Assert.AreEqual(1, viewModel.SelectedRun?.Id);
    }

    [TestMethod]
    public async Task ContextNamesHighlightedOpenedAndActiveRunsAndReturnsToExactOrigin()
    {
        var worker = new TestWorkerClient();
        var session = worker.AddSession("Photo archive", @"C:\Photos");
        var opened = worker.AddRun(session.Id, "completed");
        var active = worker.AddRun(session.Id, "running", "hashing") with { WarningCount = 1 };
        worker.Runs[1] = active;
        worker.RunWarningsHandler = (_, _) => Task.FromResult(new WorkerRunWarningPage(
            [new WorkerRunWarningAggregate(1, active.Id, "hashing", "scan", "read-warning", "warning",
                "One candidate could not be read.", 1, [@"C:\Photos\one.bin"])],
            1, 1, 1, 9, "active", "running", TestWorkerClient.DiagnosticLog, null, false));
        var workspaceRun = opened;
        var activeRun = active;
        WarningReturnDestination? returnedTo = null;
        using var viewModel = new RunHistoryViewModel(
            worker,
            workspaceRun: () => workspaceRun,
            activeRun: () => activeRun,
            sessionName: _ => session.Name,
            returnFromWarnings: destination => returnedTo = destination);
        await viewModel.LoadAsync(session.Id);

        Assert.AreEqual(active.Id, viewModel.SelectedRun?.Id);
        StringAssert.Contains(viewModel.SelectedRunIdentity, $"Scan {active.Id}");
        StringAssert.Contains(viewModel.SelectedRunRelationship, $"workspace remains on Photo archive · Scan {opened.Id}");
        StringAssert.Contains(viewModel.SelectedRunParameters, "immutable run");
        CollectionAssert.AreEqual(active.Parameters.Roots.ToArray(), viewModel.SelectedRunRoots.ToArray());
        StringAssert.Contains(viewModel.ActiveRunContext, $"Scan {active.Id}");

        await viewModel.OpenWarningsForRunAsync(active, WarningReturnDestination.ScanProgress);

        Assert.AreEqual("Warnings · active scan", viewModel.WarningContextHeading);
        StringAssert.Contains(viewModel.WarningContextIdentity, $"Scan {active.Id}");
        StringAssert.Contains(viewModel.WarningSnapshotBoundary, "Current warning revision 9");
        Assert.AreEqual("_Return to progress", viewModel.WarningReturnLabel);
        viewModel.CloseWarningsCommand.Execute(null);
        Assert.AreEqual(WarningReturnDestination.ScanProgress, returnedTo);
        Assert.AreNotEqual("history", viewModel.FocusTarget);
    }

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
                100_000, 100_000, 100_000, 10, "terminal", "completed",
                TestWorkerClient.DiagnosticLog, $"cursor-{index + 1}", false));
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
        Assert.AreEqual(RunHistoryViewModel.WarningCachePageLimit, viewModel.WarningDrilldown.CachedPageCount);
        Assert.IsTrue(queries.All(query => query.SortField == RunWarningSortField.OccurrenceCount));
        Assert.IsTrue(queries.All(query => query.SortDirection == WorkerSortDirection.Descending));
        StringAssert.Contains(viewModel.WarningStatusMessage, "25 of 100,000");

        await viewModel.ApplyWarningSortAsync(RunWarningSortField.Phase, WorkerSortDirection.Ascending);
        Assert.AreEqual(RunWarningSortField.Phase, queries[^1].SortField);
        Assert.AreEqual(WorkerSortDirection.Ascending, queries[^1].SortDirection);
        Assert.IsNull(queries[^1].Cursor);
        Assert.AreEqual(RunHistoryViewModel.WarningPageSize, viewModel.Warnings.Count);
        Assert.AreEqual(1, viewModel.WarningDrilldown.CachedPageCount);

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
            1, 1, 1, 10, "terminal", "completed", TestWorkerClient.DiagnosticLog, null, false));
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
            new WorkerRunWarningPage(
                [], 0, 1, 0, 10, "terminal", "completed", TestWorkerClient.DiagnosticLog, null, true));
        using var viewModel = new RunHistoryViewModel(worker);
        await viewModel.LoadAsync(3);

        await viewModel.OpenWarningsCommand.ExecuteAsync(null);

        Assert.IsTrue(viewModel.IsWarningDrilldownOpen);
        Assert.IsTrue(viewModel.HasWarningError);
        StringAssert.Contains(viewModel.WarningErrorMessage, "unsafe");
    }

    [TestMethod]
    public async Task HashWarningNavigatesByStableRunIdAndReportsOneMissingTargetActionably()
    {
        var run = TestWorkerClient.CreateRun(7, 3, "completed", "finalizing", DateTimeOffset.UtcNow) with
        {
            WarningCount = 1,
        };
        var warning = new WorkerRunWarningAggregate(
            41,
            run.Id,
            "hashing",
            "scan",
            RunHistoryViewModel.HashWarningCode,
            "warning",
            "Some candidate files could not be hashed.",
            1,
            ["bounded example"]);
        var worker = new TestWorkerClient();
        worker.Runs.Add(run);
        worker.RunWarningsHandler = (_, _) => Task.FromResult(
            new WorkerRunWarningPage(
                [warning], 1, 1, 1, 10, "terminal", "completed", TestWorkerClient.DiagnosticLog, null, false));
        WorkerRun? navigatedTarget = null;
        using var viewModel = new RunHistoryViewModel(
            worker,
            (target, _) =>
            {
                navigatedTarget = target;
                return Task.CompletedTask;
            });
        await viewModel.LoadAsync(3);
        await viewModel.OpenWarningsCommand.ExecuteAsync(null);

        await viewModel.NavigateWarningCommand.ExecuteAsync(warning);

        Assert.AreEqual(run.Id, navigatedTarget?.Id);
        StringAssert.Contains(viewModel.WarningStatusMessage, "Opened immutable duplicate-file results");
        Assert.IsFalse(viewModel.HasWarningError);
        Assert.AreEqual(1, viewModel.Warnings.Count, "Navigation changed the bounded warning page.");

        worker.GetRunHandler = (_, _) => Task.FromException<WorkerRun>(
            new InvalidOperationException("run_not_found"));
        await viewModel.NavigateWarningCommand.ExecuteAsync(warning);

        Assert.IsTrue(viewModel.HasWarningError);
        StringAssert.Contains(viewModel.WarningErrorMessage, "unavailable");
        StringAssert.Contains(viewModel.WarningErrorMessage, "Refresh run history");
        Assert.AreEqual("warning-action:41", viewModel.FocusTarget);
        Assert.AreEqual(1, viewModel.Warnings.Count, "Missing-target handling changed immutable history.");
    }

    [TestMethod]
    public async Task HashWarningNavigationCancelsAndRejectsLateOldRunContext()
    {
        var current = TestWorkerClient.CreateRun(8, 3, "completed", "finalizing", DateTimeOffset.UtcNow) with
        {
            WarningCount = 1,
        };
        var older = TestWorkerClient.CreateRun(7, 3, "completed", "finalizing", DateTimeOffset.UtcNow.AddMinutes(-1));
        var warning = new WorkerRunWarningAggregate(
            42,
            current.Id,
            "hashing",
            "scan",
            RunHistoryViewModel.HashWarningCode,
            "warning",
            "Some candidate files could not be hashed.",
            1,
            ["bounded example"]);
        var worker = new TestWorkerClient();
        worker.Runs.Add(current);
        worker.Runs.Add(older);
        worker.RunWarningsHandler = (_, _) => Task.FromResult(
            new WorkerRunWarningPage(
                [warning], 1, 1, 1, 10, "terminal", "completed", TestWorkerClient.DiagnosticLog, null, false));
        var firstResolution = new TaskCompletionSource<WorkerRun>(TaskCreationOptions.RunContinuationsAsynchronously);
        CancellationToken firstToken = default;
        worker.GetRunHandler = (_, token) =>
        {
            firstToken = token;
            return firstResolution.Task;
        };
        var navigationCount = 0;
        using var viewModel = new RunHistoryViewModel(
            worker,
            (_, _) =>
            {
                navigationCount++;
                return Task.CompletedTask;
            });
        await viewModel.LoadAsync(3);
        await viewModel.OpenWarningsCommand.ExecuteAsync(null);

        var cancelled = viewModel.NavigateWarningCommand.ExecuteAsync(warning);
        viewModel.CancelWarningNavigationCommand.Execute(null);
        firstResolution.SetResult(current);
        await cancelled;

        Assert.IsTrue(firstToken.IsCancellationRequested);
        Assert.AreEqual(0, navigationCount);
        StringAssert.Contains(viewModel.WarningStatusMessage, "cancelled");
        Assert.AreEqual("warning-action:42", viewModel.FocusTarget);

        var staleResolution = new TaskCompletionSource<WorkerRun>(TaskCreationOptions.RunContinuationsAsynchronously);
        worker.GetRunHandler = (_, _) => staleResolution.Task;
        var stale = viewModel.NavigateWarningCommand.ExecuteAsync(warning);
        viewModel.SelectedRun = viewModel.Runs.Single(item => item.Id == older.Id);
        staleResolution.SetResult(current);
        await stale;

        Assert.AreEqual(0, navigationCount);
        Assert.IsFalse(viewModel.IsWarningDrilldownOpen);
        Assert.IsFalse(viewModel.HasWarningError);
        Assert.AreEqual(0, viewModel.Warnings.Count);
    }
}
