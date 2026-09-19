namespace SuperDuper.Windows.Infrastructure.Tests;

internal static class TestDirectoryCleanup
{
    public static async Task DeleteAsync(string path)
    {
        var deadline = DateTime.UtcNow.AddSeconds(5);
        while (true)
        {
            try
            {
                Directory.Delete(path, recursive: true);
                return;
            }
            catch (IOException) when (DateTime.UtcNow < deadline)
            {
                await Task.Delay(100);
            }
        }
    }

    // Worker-backed tests write the worker's stderr to <path>/logs; a failure is exactly when that
    // log is worth keeping, so preserve it as a test result attachment before it is deleted with
    // the rest of the disposable fixture.
    public static async Task DeleteAsync(string path, TestContext testContext)
    {
        if (testContext.CurrentTestOutcome != UnitTestOutcome.Passed)
        {
            PreserveWorkerLogs(path, testContext);
        }
        await DeleteAsync(path);
    }

    private static void PreserveWorkerLogs(string path, TestContext testContext)
    {
        var logsDirectory = Path.Combine(path, "logs");
        if (!Directory.Exists(logsDirectory))
        {
            return;
        }
        var preserved = Path.Combine(
            Path.GetTempPath(),
            $"super-duper-worker-log-{testContext.TestName}-{Guid.NewGuid():N}");
        Directory.CreateDirectory(preserved);
        foreach (var file in Directory.GetFiles(logsDirectory))
        {
            var destination = Path.Combine(preserved, Path.GetFileName(file));
            try
            {
                File.Copy(file, destination, overwrite: true);
                testContext.AddResultFile(destination);
            }
            catch (IOException)
            {
            }
        }
        testContext.WriteLine($"Preserved worker log(s) for the failed test at {preserved}.");
    }
}
