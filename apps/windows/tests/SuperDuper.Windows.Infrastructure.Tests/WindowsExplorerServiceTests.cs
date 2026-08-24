using SuperDuper.Windows.Core.Services;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WindowsExplorerServiceTests
{
    [TestMethod]
    public async Task RevealAsync_RunsNativeWorkAwayFromTheCallerAndNormalizesThePath()
    {
        var callerThread = Environment.CurrentManagedThreadId;
        var nativeStarted = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var releaseNative = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var nativeThread = 0;
        string? capturedPath = null;
        var service = new WindowsExplorerService(path =>
        {
            nativeThread = Environment.CurrentManagedThreadId;
            capturedPath = path;
            nativeStarted.SetResult();
            releaseNative.Task.GetAwaiter().GetResult();
        });

        var reveal = service.RevealAsync(@"C:\fixture\folder");
        await nativeStarted.Task;

        Assert.IsFalse(reveal.IsCompleted);
        Assert.AreNotEqual(callerThread, nativeThread);
        Assert.AreEqual(@"C:\fixture\folder", capturedPath);

        releaseNative.SetResult();
        await reveal;
    }

    [TestMethod]
    public async Task RevealAsync_ReportsTheRequestedPathAndUnderlyingFailure()
    {
        var service = new WindowsExplorerService(
            _ => throw new IOException("The location is unavailable."));

        var exception = await Assert.ThrowsExceptionAsync<InvalidOperationException>(
            () => service.RevealAsync(@"C:\missing\folder"));

        StringAssert.Contains(exception.Message, @"File Explorer could not reveal 'C:\missing\folder'.");
        StringAssert.Contains(exception.Message, "The location is unavailable.");
    }

    [TestMethod]
    public async Task RevealAsync_CancellationBeforeDispatchDoesNotRunNativeWork()
    {
        var called = false;
        var service = new WindowsExplorerService(_ => called = true);
        using var cancellation = new CancellationTokenSource();
        cancellation.Cancel();

        await Assert.ThrowsExceptionAsync<TaskCanceledException>(
            () => service.RevealAsync(@"C:\fixture\folder", cancellation.Token));

        Assert.IsFalse(called);
    }

    [TestMethod]
    public async Task SelectByParentAsync_GroupsDeterministicallyAndCallsEachParentOnceOffCallerThread()
    {
        var callerThread = Environment.CurrentManagedThreadId;
        var firstCallStarted = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var releaseFirstCall = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var calls = new List<(int ThreadId, string Parent, IReadOnlyList<string> Items)>();
        var service = new WindowsExplorerService(
            _ => { },
            (parent, items) =>
            {
                calls.Add((Environment.CurrentManagedThreadId, parent, items));
                if (calls.Count == 1)
                {
                    firstCallStarted.SetResult();
                    releaseFirstCall.Task.GetAwaiter().GetResult();
                }
            });

        var selection = service.SelectByParentAsync(
            [@"D:\Other\Three", @"C:\Shared\Two", @"c:\shared\One"]);
        await firstCallStarted.Task;

        Assert.IsFalse(selection.IsCompleted);
        Assert.AreNotEqual(callerThread, calls[0].ThreadId);

        releaseFirstCall.SetResult();
        var result = await selection;

        Assert.AreEqual(3, result.RequestedItemCount);
        Assert.AreEqual(2, result.ParentCount);
        Assert.AreEqual(3, result.SelectedItemCount);
        Assert.AreEqual(0, result.Failures.Count);
        Assert.AreEqual(2, calls.Count);
        Assert.IsTrue(calls.All(call => call.ThreadId != callerThread));
        Assert.AreEqual(@"C:\Shared", calls[0].Parent, ignoreCase: true);
        CollectionAssert.AreEqual(
            new[] { @"c:\shared\One", @"C:\Shared\Two" },
            calls[0].Items.ToArray(),
            StringComparer.OrdinalIgnoreCase);
        Assert.AreEqual(@"D:\Other", calls[1].Parent);
        CollectionAssert.AreEqual(new[] { @"D:\Other\Three" }, calls[1].Items.ToArray());
    }

    [TestMethod]
    public async Task SelectByParentAsync_ContinuesAfterOneParentFailsAndReturnsAggregateFailure()
    {
        var calls = new List<string>();
        var service = new WindowsExplorerService(
            _ => { },
            (parent, _) =>
            {
                calls.Add(parent);
                if (parent.StartsWith(@"D:\", StringComparison.OrdinalIgnoreCase))
                {
                    throw new IOException("The parent is offline.");
                }
            });

        var result = await service.SelectByParentAsync(
            [@"C:\Shared\One", @"C:\Shared\Two", @"D:\Offline\Three"]);

        Assert.AreEqual(2, calls.Count);
        Assert.AreEqual(2, result.ParentCount);
        Assert.AreEqual(2, result.SelectedItemCount);
        Assert.AreEqual(1, result.FailedItemCount);
        Assert.AreEqual(1, result.SuccessfulParentCount);
        Assert.AreEqual(1, result.Failures.Count);
        Assert.AreEqual(@"D:\Offline", result.Failures[0].ParentPath);
        Assert.AreEqual(1, result.Failures[0].ItemCount);
        StringAssert.Contains(result.Failures[0].ErrorMessage, "offline");
    }

    [TestMethod]
    public async Task SelectByParentAsync_CancellationStopsBeforeTheNextParent()
    {
        using var cancellation = new CancellationTokenSource();
        var calls = new List<string>();
        var service = new WindowsExplorerService(
            _ => { },
            (parent, _) =>
            {
                calls.Add(parent);
                cancellation.Cancel();
            });

        await Assert.ThrowsExceptionAsync<OperationCanceledException>(
            () => service.SelectByParentAsync(
                [@"C:\First\One", @"D:\Second\Two"],
                cancellation.Token));

        Assert.AreEqual(1, calls.Count);
        Assert.AreEqual(@"C:\First", calls[0]);
    }

    [TestMethod]
    public void SelectByParentAsync_RejectsMoreThanTheBoundedCurrentPage()
    {
        var called = false;
        var service = new WindowsExplorerService(
            _ => { },
            (_, _) => called = true);
        var paths = Enumerable.Range(0, IExplorerService.MaximumSelectionItems + 1)
            .Select(index => $@"C:\Bounded\Folder-{index}")
            .ToArray();

        Assert.ThrowsException<ArgumentOutOfRangeException>(
            () => service.SelectByParentAsync(paths));
        Assert.IsFalse(called);
    }
}
