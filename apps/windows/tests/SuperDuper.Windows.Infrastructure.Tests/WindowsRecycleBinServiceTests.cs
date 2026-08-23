using System.Diagnostics;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WindowsRecycleBinServiceTests
{
    [TestMethod]
    public async Task OpenAsync_NavigatesToTheRecycleBinWithoutInspectingIt()
    {
        ProcessStartInfo? captured = null;
        var service = new WindowsRecycleBinService(startInfo => captured = startInfo);

        await service.OpenAsync();

        Assert.IsNotNull(captured);
        Assert.AreEqual("explorer.exe", captured.FileName);
        Assert.AreEqual("shell:RecycleBinFolder", captured.Arguments);
        Assert.IsTrue(captured.UseShellExecute);
    }

    [TestMethod]
    public async Task OpenAsync_DoesNotStartAnythingAfterCancellation()
    {
        var started = false;
        var service = new WindowsRecycleBinService(_ => started = true);
        using var cancellation = new CancellationTokenSource();
        cancellation.Cancel();

        await Assert.ThrowsExceptionAsync<OperationCanceledException>(
            () => service.OpenAsync(cancellation.Token));

        Assert.IsFalse(started);
    }
}
