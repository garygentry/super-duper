namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WorkerClientStartupTests
{
    [TestMethod]
    public async Task ConnectAsync_WhenExecutableIsMissing_ReturnsActionableFailure()
    {
        var missingPath = Path.Combine(
            Path.GetTempPath(),
            $"missing-super-duper-worker-{Guid.NewGuid():N}.exe");
        await using var client = new WorkerClient(missingPath, TimeSpan.FromSeconds(1));

        var exception = await Assert.ThrowsExceptionAsync<WorkerConnectionException>(
            () => client.ConnectAsync());

        Assert.AreEqual(Path.GetFullPath(missingPath), exception.ExecutablePath);
        StringAssert.Contains(exception.Message, missingPath);
        StringAssert.Contains(exception.Message, "could not be started");
    }
}
