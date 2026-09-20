namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WorkerClientStartupTests
{
    [TestMethod]
    public void WorkerClient_SupportsSynchronousContainerDisposal()
    {
        using var client = new WorkerClient(
            Path.Combine(Path.GetTempPath(), $"unused-worker-{Guid.NewGuid():N}.exe"),
            TimeSpan.FromSeconds(1));

        Assert.IsInstanceOfType<IDisposable>(client);
    }

    [TestMethod]
    public void WorkerClient_DerivesDiagnosticLogPathFromStateDirectory()
    {
        // A disposable database override must not leave the worker log behind in the real
        // %LOCALAPPDATA%\SuperDuper\logs folder (issue #42).
        var database = Path.Combine(Path.GetTempPath(), $"sd-diag-log-{Guid.NewGuid():N}", "super_duper.db");
        var state = WorkerStateLocations.Resolve(
            name => name == "SUPER_DUPER_DB_PATH" ? database : null,
            Path.Combine(Path.GetTempPath(), "unused-local-app-data"));

        using var client = new WorkerClient(
            Path.Combine(Path.GetTempPath(), $"unused-worker-{Guid.NewGuid():N}.exe"),
            state);

        Assert.AreEqual(
            Path.Combine(state.StateDirectory, "logs", "worker.log"),
            client.DiagnosticLogPath);
    }

    [TestMethod]
    public async Task ConnectAsync_WhenExecutableIsMissing_ReturnsActionableFailure()
    {
        var missingPath = Path.Combine(
            Path.GetTempPath(),
            $"missing-super-duper-worker-{Guid.NewGuid():N}.exe");
        await using var client = new WorkerClient(missingPath, TimeSpan.FromSeconds(1));

        var exception = await Assert.ThrowsExactlyAsync<WorkerConnectionException>(
            () => client.ConnectAsync());

        Assert.AreEqual(Path.GetFullPath(missingPath), exception.ExecutablePath);
        StringAssert.Contains(exception.Message, missingPath);
        StringAssert.Contains(exception.Message, "could not be started");
    }
}
