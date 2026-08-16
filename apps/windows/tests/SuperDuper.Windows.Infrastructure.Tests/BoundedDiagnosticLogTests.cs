namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class BoundedDiagnosticLogTests
{
    [TestMethod]
    public async Task ActiveLog_RotatesWhileTheSameWriterRemainsInUse()
    {
        var directory = Path.Combine(
            Path.GetTempPath(),
            $"super-duper-diagnostic-log-{Guid.NewGuid():N}");
        var path = Path.Combine(directory, "worker.log");

        try
        {
            await using var log = BoundedDiagnosticLog.TryOpen(path, maximumBytes: 256);
            Assert.IsNotNull(log);
            for (var index = 0; index < 40; index++)
            {
                Assert.IsTrue(await log.TryWriteLineAsync(
                    $"record-{index:D2} {new string('x', 40)}",
                    CancellationToken.None));
            }

            Assert.IsTrue(File.Exists(path));
            Assert.IsTrue(File.Exists(path + ".previous"));
            Assert.IsTrue(new FileInfo(path).Length < 256);
            Assert.IsTrue(new FileInfo(path + ".previous").Length < 320);
        }
        finally
        {
            if (Directory.Exists(directory))
            {
                Directory.Delete(directory, recursive: true);
            }
        }
    }
}
