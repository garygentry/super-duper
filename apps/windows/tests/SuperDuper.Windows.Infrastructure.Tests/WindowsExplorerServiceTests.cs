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
}
