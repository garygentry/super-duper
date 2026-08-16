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
}
