using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class DisabledRecycleOperationCapabilityExecutorTests
{
    [TestMethod]
    public async Task InspectAsync_IsBoundedToInputAndNeverEnablesExecution()
    {
        var executor = new DisabledRecycleOperationCapabilityExecutor();
        var item = new WorkerRecycleOperationItem(
            7, 3, 2, 0, 5, null, "file", @"C:\fixture.bin", 1, null, null, 9, null,
            "128", "pending", null, "pending", null, null, null, null);

        var observations = await executor.InspectAsync([item]);

        Assert.IsFalse(executor.IsEnabled);
        Assert.AreEqual(1, observations.Count);
        Assert.AreEqual(7, observations[0].ItemId);
        Assert.AreEqual("non_recyclable", observations[0].Status);
        Assert.AreEqual("executor_disabled", observations[0].ReasonCode);
    }

    [TestMethod]
    public async Task InspectAsync_HonorsCancellationBeforeInspection()
    {
        var executor = new DisabledRecycleOperationCapabilityExecutor();
        using var cancellation = new CancellationTokenSource();
        cancellation.Cancel();

        await Assert.ThrowsExceptionAsync<OperationCanceledException>(
            () => executor.InspectAsync([], cancellation.Token));
    }
}
