using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class ScanProgressViewModelTests
{
    [TestMethod]
    public void ApplyProgress_IgnoresOutOfOrderSequence()
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "discovering");
        using var viewModel = new ScanProgressViewModel(client, new ImmediateDispatcher());
        viewModel.ShowRun(run);

        viewModel.ApplyProgress(Progress(run.Id, sequence: 2, files: 20));
        viewModel.ApplyProgress(Progress(run.Id, sequence: 1, files: 10));

        Assert.AreEqual("20", viewModel.FilesDiscovered.Replace(",", ""));
    }

    [TestMethod]
    public async Task CancelCommand_ShowsCancellingBeforeWorkerConfirms()
    {
        var client = new TestWorkerClient();
        var run = client.AddRun(1, "running", "hashing");
        var completion = new TaskCompletionSource<WorkerRun>(TaskCreationOptions.RunContinuationsAsynchronously);
        client.CancelHandler = (_, _) => completion.Task;
        using var viewModel = new ScanProgressViewModel(client, new ImmediateDispatcher());
        viewModel.ShowRun(run);

        var cancel = viewModel.CancelCommand.ExecuteAsync(null);

        Assert.IsTrue(viewModel.IsCancelling);
        completion.SetResult(run with { Status = "cancelling" });
        await cancel;
        Assert.AreEqual("Cancelling", viewModel.Status);
    }

    private static WorkerRunProgressEventArgs Progress(long runId, ulong sequence, long files) => new()
    {
        RunId = runId,
        Sequence = sequence,
        Status = "running",
        Phase = "discovering",
        FilesDiscovered = files,
        BytesDiscovered = "100",
        FilesHashed = 0,
        WarningCount = 0,
    };
}
